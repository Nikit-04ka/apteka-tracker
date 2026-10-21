//! Экспорт и импорт CSV: формат программы (разделитель «;», BOM, экранирование кавычек).
use crate::model::{days_left, dispose_reason_text, fmt_date, fmt_date_display, parse_date_input, status_for, status_text, Database, Medicine, Package, STATUS_DISPOSED};
use crate::storage::{data_file_path, exe_dir};
use chrono::Local;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Путь к файлу экспорта CSV.
pub fn csv_file_path() -> PathBuf {
    exe_dir().join("apteka_export.csv")
}

/// Каталог резервных копий (рядом с exe).
pub fn backup_dir() -> PathBuf {
    exe_dir().join("backups")
}

/// Резервная копия файла данных при запуске; хранятся последние 5 копий.
pub fn backup() -> io::Result<Option<PathBuf>> {
    backup_from_to(&data_file_path(), &backup_dir(), 5)
}

/// Копирует `src` в каталог `dir` под именем с меткой времени и оставляет
/// только `keep` самых свежих копий.
pub fn backup_from_to(src: &Path, dir: &Path, keep: usize) -> io::Result<Option<PathBuf>> {
    if !src.exists() {
        return Ok(None);
    }
    fs::create_dir_all(dir)?;
    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let dst = dir.join(format!("apteka_{stamp}.json"));
    fs::copy(src, &dst)?;
    rotate_backups(dir, keep);
    Ok(Some(dst))
}

/// Удаляет самые старые резервные копии, оставляя `keep` последних.
/// Имена вида `apteka_YYYYMMDD-HHMMSS.json` сортируются хронологически.
pub fn rotate_backups(dir: &Path, keep: usize) {
    let mut files: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok().map(|e| e.path())).collect(),
        Err(_) => return,
    };
    files.retain(|p| p.is_file() && p.extension().is_some_and(|ext| ext == "json"));
    files.sort();
    while files.len() > keep {
        match files.first() {
            Some(oldest) => {
                let _ = fs::remove_file(oldest);
                files.remove(0);
            }
            None => break,
        }
    }
}

/// Список резервных копий, самые свежие первыми.
pub fn list_backups() -> Vec<PathBuf> {
    let dir = backup_dir();
    let mut files: Vec<PathBuf> = match fs::read_dir(&dir) {
        Ok(rd) => rd.filter_map(|e| e.ok().map(|e| e.path())).collect(),
        Err(_) => return Vec::new(),
    };
    files.retain(|p| p.is_file() && p.extension().is_some_and(|ext| ext == "json"));
    files.sort();
    files.reverse();
    files
}

/// Загрузка базы со стороны старта приложения: с полной диагностикой.
pub fn load() -> LoadOutcome {
    load_outcome_from(&data_file_path())
}

/// Результат загрузки базы с диагностикой.
pub enum LoadOutcome {
    /// Успешно загружена
    Loaded(Database),
    /// Файла нет — первый запуск
    Missing,
    /// Файл повреждён (переименован в *.corrupt-*) — работаем с пустой базой
    Corrupt { database: Database, backup: PathBuf, msg: String },
    /// Файл не читается (занят/права) — вызывающий должен завершиться,
    /// не перезаписывая данные
    Io(String),
}

/// Загрузка с полной диагностикой (для старта приложения и тестов).
pub fn load_outcome_from(path: &Path) -> LoadOutcome {
    if !path.exists() {
        return LoadOutcome::Missing;
    }
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => return LoadOutcome::Io(format!("{}: {e}", path.display())),
    };
    match serde_json::from_str::<Database>(&text) {
        Ok(db) => LoadOutcome::Loaded(db),
        Err(e) => {
            let stamp = Local::now().format("%Y%m%d-%H%M%S");
            let backup = PathBuf::from(format!(
                "{}.corrupt-{stamp}",
                path.to_string_lossy()
            ));
            let msg = format!("{e}");
            let _ = fs::rename(path, &backup);
            LoadOutcome::Corrupt { database: Database::default(), backup, msg }
        }
    }
}

/// Загрузка базы из файла (для восстановления из бэкапа) — без побочных эффектов.
pub fn load_database_from(path: &Path) -> Result<Database, String> {
    match load_outcome_from(path) {
        LoadOutcome::Loaded(db) => Ok(db),
        LoadOutcome::Missing => Err(format!("Файл не найден: {}", path.display())),
        LoadOutcome::Corrupt { msg, .. } => Err(format!("Повреждённый JSON: {msg}")),
        LoadOutcome::Io(e) => Err(e),
    }
}

/// Загрузка «как раньше» — для тестов и внутренних нужд: пустая база при проблемах.
pub fn load_from(path: &Path) -> Database {
    match load_outcome_from(path) {
        LoadOutcome::Loaded(db) => db,
        _ => Database::default(),
    }
}

/// Сохраняет базу рядом с exe.
pub fn save(db: &Database) -> io::Result<PathBuf> {
    let path = data_file_path();
    save_to(db, &path)?;
    Ok(path)
}

/// Атомарная запись: сначала во временный файл, затем переименование.
pub fn save_to(db: &Database, path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    {
        let text = serde_json::to_string_pretty(db).map_err(io::Error::other)?;
        let mut f = fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, path)
}

/// Экспортирует всё в CSV (разделитель `;`, BOM — чтобы Excel открыл кириллицу).
pub fn export_csv(db: &Database) -> io::Result<PathBuf> {
    let path = csv_file_path();
    export_csv_to(db, &path)?;
    Ok(path)
}

pub fn export_csv_to(db: &Database, path: &Path) -> io::Result<()> {
    let today = Local::now().date_naive();
    let mut out = String::new();
    out.push('\u{FEFF}');
    out.push_str("Лекарство;Форма;Заметки;Срок годности;Дней до конца;Статус;Количество;Цена;Открыта;Списано;Комментарий\r\n");
    for m in &db.medicines {
        if m.packages.is_empty() {
            out.push_str(&format!(
                "{};{};{};;;;;;;;\r\n",
                esc(&m.name),
                esc(&m.form),
                esc(&m.note)
            ));
            continue;
        }
        for p in &m.packages {
            let eff = p.effective_expiry();
            let days = days_left(eff, today);
            let (_status, status_cell) = if let Some(d) = &p.disposed {
                (
                    STATUS_DISPOSED,
                    format!("списано ({})", dispose_reason_text(d.reason)),
                )
            } else {
                let s = status_for(days, db.warn_days as i64);
                (s, status_text(s, days))
            };
            let date_cell = if p.month_only {
                format!("{} (до конца мес.)", fmt_date_display(p.expiry, true))
            } else {
                fmt_date(p.expiry)
            };
            let price_cell = p.price.map(|v| format!("{v:.2}")).unwrap_or_default();
            let opened_cell = p
                .opened
                .map(|o| format!("открыта {}", fmt_date(o)))
                .unwrap_or_default();
            let disposed_cell = p
                .disposed
                .as_ref()
                .map(|d| format!("{} {}", dispose_reason_text(d.reason), fmt_date(d.date)))
                .unwrap_or_default();
            out.push_str(&format!(
                "{};{};{};{};{};{};{};{};{};{};{}\r\n",
                esc(&m.name),
                esc(&m.form),
                esc(&m.note),
                date_cell,
                days,
                esc(&status_cell),
                esc(&p.quantity),
                price_cell,
                opened_cell,
                disposed_cell,
                esc(&p.note)
            ));
        }
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, out.as_bytes())
}

fn esc(s: &str) -> String {
    let mut v = s.replace('"', "\"\"");
    // защита от CSV formula injection: Excel исполняет ячейки, начинающиеся с = + - @
    if v.starts_with(['=', '+', '-', '@']) {
        v.insert(0, '\'');
    }
    format!("\"{}\"", v)
}

/// Разбор CSV-текста (наш экспорт: разделитель «;», кавычки с удвоением).
pub fn parse_csv_text(text: &str) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(c);
            }
        } else {
            match c {
                '"' => in_quotes = true,
                ';' => row.push(std::mem::take(&mut field)),
                '\r' => {}
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    if !(row.len() == 1 && row[0].is_empty()) {
                        rows.push(std::mem::take(&mut row));
                    }
                }
                _ => field.push(c),
            }
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// Импорт CSV-файла экспорта: возвращает лекарства (id проставит вызывающий)
/// и количество пропущенных строк.
pub fn import_csv(path: &Path) -> io::Result<(Vec<Medicine>, usize)> {
    let text = fs::read_to_string(path)?;
    let text = text.strip_prefix('\u{FEFF}').unwrap_or(&text);
    let rows = parse_csv_text(text);
    let mut meds: Vec<Medicine> = Vec::new();
    let mut skipped = 0usize;
    for (i, row) in rows.iter().enumerate() {
        if i == 0 {
            continue; // заголовок
        }
        if row.len() < 7 {
            skipped += 1;
            continue;
        }
        let name = row[0].trim();
        if name.is_empty() {
            skipped += 1;
            continue;
        }
        // экспортный формат: «01.2028 (до конца мес.)» — отрезаем пометку
        let date_raw = row[3].trim();
        let date_clean = date_raw.split(" (").next().unwrap_or(date_raw);
        let Some((expiry, month_only)) = parse_date_input(date_clean) else {
            skipped += 1;
            continue;
        };
        let form = row[1].trim().to_string();
        let note = row[2].trim().to_string();
        let quantity = row[6].trim().to_string();
        let price = if row.len() > 7 && !row[7].trim().is_empty() {
            row[7].trim().replace(',', ".").parse::<f64>().ok()
        } else {
            None
        };
        let pnote = if row.len() > 10 {
            row[10].trim().to_string()
        } else {
            String::new()
        };
        let package = Package {
            id: 0,
            expiry,
            month_only,
            quantity,
            note: pnote,
            price,
            opened: None,
            open_period_days: None,
            disposed: None,
            photo: String::new(),
        };
        // подряд идущие строки одного лекарства склеиваются в его упаковки
        if let Some(last) = meds.last_mut() {
            if last.name == name && last.form == form {
                last.packages.push(package);
                continue;
            }
        }
        meds.push(Medicine {
            id: 0,
            name: name.to_string(),
            form,
            note,
            packages: vec![package],
        });
    }
    Ok((meds, skipped))
}



#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn temp_dir(tag: &str) -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "apteka-csv-{}-{}-{tag}{n}",
            std::process::id(),
            Local::now().timestamp_millis()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_db() -> Database {
        let mut db = Database { warn_days: 45, ..Default::default() };
        db.medicines.push(Medicine {
            id: 1,
            name: "Парацетамол \"Тева\"".to_string(),
            form: "Таблетки".to_string(),
            note: "от головы".to_string(),
            packages: vec![
                Package {
                    id: 1,
                    expiry: chrono::NaiveDate::from_ymd_opt(2030, 1, 1).unwrap(),
                    month_only: false,
                    quantity: "2 уп.".to_string(),
                    note: "в ящике".to_string(),
                    price: Some(350.0),
                    opened: None,
                    open_period_days: None,
                    disposed: None,
                    photo: String::new(),
                },
                Package {
                    id: 2,
                    expiry: chrono::NaiveDate::from_ymd_opt(2028, 1, 31).unwrap(),
                    month_only: true,
                    quantity: "1 фл.".to_string(),
                    note: String::new(),
                    price: None,
                    opened: None,
                    open_period_days: None,
                    disposed: None,
                    photo: String::new(),
                },
            ],
        });
        db
    }

    #[test]
    fn csv_export_contains_all_rows() {
        let dir = temp_dir("csv");
        let path = dir.join("export.csv");
        export_csv_to(&sample_db(), &path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with('\u{FEFF}'), "нужен BOM для Excel");
        assert!(text.contains("\"Парацетамол \"\"Тева\"\"\""), "кавычки экранируются");
        assert!(text.contains("01.01.2030"));
        assert!(text.contains("350.00"), "цена попадает в CSV");
        assert!(text.contains("01.2028 (до конца мес.)"), "месячный срок помечается в CSV");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn import_csv_parses_export_format() {
        let dir = temp_dir("import");
        let csv = dir.join("export.csv");
        export_csv_to(&sample_db(), &csv).unwrap();

        let (meds, skipped) = import_csv(&csv).unwrap();
        assert_eq!(skipped, 0);
        assert_eq!(meds.len(), 1);
        assert_eq!(meds[0].name, "Парацетамол \"Тева\"");
        assert_eq!(meds[0].packages.len(), 2, "обе упаковки, включая месячную");
        assert_eq!(meds[0].packages[1].expiry.to_string(), "2028-01-31");
        assert!(meds[0].packages[1].month_only);
        assert_eq!(meds[0].packages[0].price, Some(350.0));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn import_csv_tolerates_garbage() {
        let dir = temp_dir("import-bad");
        let csv = dir.join("bad.csv");
        fs::write(&csv, "\u{FEFF}Лекарство;Форма\r\n;;\r\nХлам;без;даты;не дата\r\n").unwrap();
        let (meds, skipped) = import_csv(&csv).unwrap();
        assert_eq!(meds.len(), 0);
        assert!(skipped >= 1);
        fs::remove_dir_all(&dir).ok();
    }
}
