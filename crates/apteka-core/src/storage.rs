//! Сохранение и загрузка данных: JSON рядом с exe (портативный режим) и экспорт CSV.

use crate::model::{days_left, fmt_date, fmt_date_display, status_for, status_text, Database};
use chrono::Local;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Папка, где лежит exe (портативный режим).
pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Путь к файлу данных.
pub fn data_file_path() -> PathBuf {
    exe_dir().join("apteka_data.json")
}

/// Путь к файлу экспорта CSV.
pub fn csv_file_path() -> PathBuf {
    exe_dir().join("apteka_export.csv")
}

/// Загружает базу; если файла нет — возвращает пустую базу.
pub fn load() -> Database {
    load_from(&data_file_path())
}

/// Загружает базу из конкретного файла. Повреждённый файл не удаляется,
/// а переименовывается в `*.corrupt-<дата>`, чтобы данные можно было спасти.
pub fn load_from(path: &Path) -> Database {
    match fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<Database>(&text) {
            Ok(db) => db,
            Err(e) => {
                eprintln!("Не удалось разобрать {}: {e}. Файл сохранён как повреждённый.", path.display());
                backup_corrupt(path);
                Database::default()
            }
        },
        Err(_) => Database::default(),
    }
}

fn backup_corrupt(path: &Path) {
    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let mut backup = path.as_os_str().to_owned();
    backup.push(format!(".corrupt-{stamp}"));
    let _ = fs::rename(path, PathBuf::from(backup));
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
    out.push_str("Лекарство;Форма;Заметки;Срок годности;Дней до конца;Статус;Количество;Комментарий\r\n");
    for m in &db.medicines {
        if m.packages.is_empty() {
            out.push_str(&format!(
                "{};{};{};;;;;\r\n",
                esc(&m.name),
                esc(&m.form),
                esc(&m.note)
            ));
            continue;
        }
        for p in &m.packages {
            let days = days_left(p.expiry, today);
            let status = status_for(days, db.warn_days as i64);
            let date_cell = if p.month_only {
                format!("{} (до конца мес.)", fmt_date_display(p.expiry, true))
            } else {
                fmt_date(p.expiry)
            };
            out.push_str(&format!(
                "{};{};{};{};{};{};{};{}\r\n",
                esc(&m.name),
                esc(&m.form),
                esc(&m.note),
                date_cell,
                days,
                esc(&status_text(status, days)),
                esc(&p.quantity),
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
    format!("\"{}\"", s.replace('"', "\"\""))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Medicine, Package};
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// Уникальная временная папка для каждого теста.
    fn temp_dir(tag: &str) -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "apteka-test-{}-{}-{tag}{n}",
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
                },
                // «Годен до 01 2028» → 31.01.2028, месяц без дня
                Package {
                    id: 2,
                    expiry: chrono::NaiveDate::from_ymd_opt(2028, 1, 31).unwrap(),
                    month_only: true,
                    quantity: "1 фл.".to_string(),
                    note: String::new(),
                },
            ],
        });
        db
    }

    #[test]
    fn roundtrip_saves_and_loads() {
        let dir = temp_dir("roundtrip");
        let path = dir.join("data.json");
        let db = sample_db();
        save_to(&db, &path).unwrap();
        let loaded = load_from(&path);
        assert_eq!(loaded.warn_days, 45);
        assert_eq!(loaded.medicines.len(), 1);
        assert_eq!(loaded.medicines[0].name, "Парацетамол \"Тева\"");
        assert_eq!(loaded.medicines[0].packages[0].quantity, "2 уп.");
        let mp = &loaded.medicines[0].packages[1];
        assert!(mp.month_only, "флаг month_only должен пережить сохранение");
        assert_eq!(mp.expiry.to_string(), "2028-01-31");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_missing_file_returns_default() {
        let dir = temp_dir("missing");
        let db = load_from(&dir.join("nope.json"));
        assert_eq!(db.warn_days, 60);
        assert!(db.medicines.is_empty());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn corrupt_file_is_backed_up_not_deleted() {
        let dir = temp_dir("corrupt");
        let path = dir.join("data.json");
        fs::write(&path, "{ это не json").unwrap();
        let db = load_from(&path);
        assert!(db.medicines.is_empty());
        let leftovers: Vec<_> = fs::read_dir(&dir).unwrap().collect();
        assert_eq!(leftovers.len(), 1, "повреждённый файл должен остаться как .corrupt-*");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn csv_export_contains_all_rows() {
        let dir = temp_dir("csv");
        let path = dir.join("export.csv");
        let db = sample_db();
        export_csv_to(&db, &path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with('\u{FEFF}'), "нужен BOM для Excel");
        assert!(text.contains("\"Парацетамол \"\"Тева\"\"\""), "кавычки экранируются");
        assert!(text.contains("01.01.2030"));
        assert!(text.contains("01.2028 (до конца мес.)"), "месячный срок помечается в CSV");
        assert!(text.contains("в ящике"));
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn backup_copies_data_file() {
        let dir = temp_dir("backup");
        let src = dir.join("apteka_data.json");
        save_to(&sample_db(), &src).unwrap();
        let bdir = dir.join("backups");
        // нет исходника → нет копии
        assert!(backup_from_to(&dir.join("missing.json"), &bdir, 5).unwrap().is_none());
        // есть исходник → копия создана и читается
        let copied = backup_from_to(&src, &bdir, 5).unwrap();
        assert!(copied.is_some(), "бэкап должен создаться");
        let loaded = load_from(&copied.unwrap());
        assert_eq!(loaded.medicines.len(), 1);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rotation_keeps_only_newest_backups() {
        let dir = temp_dir("rotate");
        fs::create_dir_all(&dir).unwrap();
        for name in [
            "apteka_20200101-000001.json",
            "apteka_20210101-000001.json",
            "apteka_20220101-000001.json",
            "apteka_20230101-000001.json",
        ] {
            fs::write(dir.join(name), "{}").unwrap();
        }
        rotate_backups(&dir, 2);
        let mut left: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(
            left,
            vec!["apteka_20220101-000001.json", "apteka_20230101-000001.json"],
            "должны остаться две самые свежие копии"
        );
        fs::remove_dir_all(&dir).ok();
    }
}
