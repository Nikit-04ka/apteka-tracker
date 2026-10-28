//! Экспорт и импорт CSV: формат программы (разделитель «;», BOM, экранирование кавычек).
use crate::model::{
    days_left, dispose_reason_text, fmt_date, fmt_date_display, parse_date_input, parse_tags,
    status_for, status_text, Database, DisposedInfo, Medicine, Package, REASON_DISCARDED,
    REASON_USED, STATUS_DISPOSED,
};
use crate::storage::exe_dir;
use chrono::Local;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Путь к файлу экспорта CSV.
pub fn csv_file_path() -> PathBuf {
    exe_dir().join("apteka_export.csv")
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
    out.push_str("Лекарство;Форма;Заметки;Срок годности;Дней до конца;Статус;Количество;Цена;Открыта;Списано;Комментарий;Категории;Куплена;Период после вскрытия\r\n");
    for m in &db.medicines {
        let tags_cell = esc(&m.tags.join(", "));
        if m.packages.is_empty() {
            out.push_str(&format!(
                "{};{};{};;;;;;;;;{};;\r\n",
                esc(&m.name),
                esc(&m.form),
                esc(&m.note),
                tags_cell
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
            let purchased_cell = p.purchase_date.map(fmt_date).unwrap_or_default();
            let period_cell = p
                .open_period_days
                .map(|v| v.to_string())
                .unwrap_or_default();
            out.push_str(&format!(
                "{};{};{};{};{};{};{};{};{};{};{};{};{};{}\r\n",
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
                esc(&p.note),
                tags_cell,
                purchased_cell,
                period_cell
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
    // Защита от CSV formula injection: Excel исполняет ячейки, начинающиеся
    // с = + - @. Апостроф добавляется и перед собственным апострофом значения,
    // чтобы unesc снимал ровно одну защиту и round-trip был без потерь.
    if v.starts_with(['\'', '=', '+', '-', '@']) {
        v.insert(0, '\'');
    }
    format!("\"{}\"", v)
}

/// Обратная операция к `esc`: снимает ровно одну добавленную защиту (апостроф)
/// перед значением, начинающимся с `'=+-@`. Иначе импорт возвращал бы имена
/// с лишним ведущим апострофом (formula-injection защита «прилипала»).
fn unesc(s: &str) -> String {
    // проверка и срез по байту безопасны: ведущий '\'' — однобайтовый ASCII
    if s.len() >= 2 && s.starts_with('\'') {
        let rest = &s[1..];
        if rest.starts_with(['\'', '=', '+', '-', '@']) {
            return rest.to_string();
        }
    }
    s.to_string()
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

/// Разбор ячейки «Открыта» экспортного формата: «открыта 01.10.2026».
fn parse_opened_cell(s: &str) -> Option<chrono::NaiveDate> {
    let rest = s.trim().strip_prefix("открыта ")?;
    parse_date_input(rest.trim()).map(|(d, _)| d)
}

/// Разбор ячейки «Списано» экспортного формата: «израсходовано 01.10.2026»,
/// «выброшено 01.10.2026». Старый текст прежних версий («просрочено,
/// выброшено …») и любая неизвестная причина трактуется как «выброшено» —
/// обе штатные причины переживают round-trip точно.
fn parse_disposed_cell(s: &str) -> Option<DisposedInfo> {
    let (reason_text, date_text) = s.trim().rsplit_once(' ')?;
    let (date, _) = parse_date_input(date_text.trim())?;
    let reason = if reason_text.trim() == dispose_reason_text(REASON_USED) {
        REASON_USED
    } else {
        REASON_DISCARDED
    };
    Some(DisposedInfo { date, reason })
}

/// Склеивает строки одного лекарства в его упаковки: пара (имя, форма) без
/// регистра — как и дедупликация при импорте; строки не обязаны быть соседними.
fn merge_medicine(
    meds: &mut Vec<Medicine>,
    name: &str,
    form: String,
    note: String,
    tags: Vec<String>,
    package: Option<Package>,
) {
    let key = |n: &str, f: &str| (n.trim().to_lowercase(), f.trim().to_lowercase());
    let k = key(name, &form);
    if let Some(m) = meds.iter_mut().find(|m| key(&m.name, &m.form) == k) {
        if let Some(p) = package {
            m.packages.push(p);
        }
        if m.note.is_empty() && !note.is_empty() {
            m.note = note;
        }
        for t in tags {
            if !m.tags.iter().any(|e| e.to_lowercase() == t.to_lowercase()) {
                m.tags.push(t);
            }
        }
    } else {
        meds.push(Medicine {
            id: 0,
            name: name.to_string(),
            form,
            note,
            tags,
            packages: package.into_iter().collect(),
        });
    }
}

/// Дедупликация импортированных лекарств против существующей базы и внутри
/// самого файла: лекарство с уже существующей парой (имя, форма) без регистра
/// пропускается целиком — повторный импорт того же CSV не плодит дубли.
/// Возвращает (оставленные лекарства, сколько отброшено как дубли).
pub fn dedup_imported(
    existing: &[impl std::borrow::Borrow<Medicine>],
    mut imported: Vec<Medicine>,
) -> (Vec<Medicine>, usize) {
    let key = |m: &Medicine| (m.name.trim().to_lowercase(), m.form.trim().to_lowercase());
    let mut seen: std::collections::HashSet<(String, String)> =
        existing.iter().map(|m| key(m.borrow())).collect();
    let mut duplicates = 0usize;
    imported.retain(|m| {
        let k = key(m);
        if seen.contains(&k) {
            duplicates += 1;
            false
        } else {
            seen.insert(k);
            true
        }
    });
    (imported, duplicates)
}

/// Импорт CSV-файла экспорта: возвращает лекарства (id проставит вызывающий)
/// и количество пропущенных строк. Жизненный цикл упаковки (вскрытие, период
/// после вскрытия, списание) переживает экспорт/импорт.
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
        let name_raw = unesc(&row[0]);
        let name = name_raw.trim();
        if name.is_empty() {
            skipped += 1;
            continue;
        }
        let form = unesc(&row[1]).trim().to_string();
        let note = unesc(&row[2]).trim().to_string();
        let tags_src = if row.len() > 11 {
            unesc(&row[11])
        } else {
            String::new()
        };
        let tags = parse_tags(tags_src.trim());
        // экспортный формат: «01.2028 (до конца мес.)» — отрезаем пометку
        let date_raw = row[3].trim();
        let date_clean = date_raw.split(" (").next().unwrap_or(date_raw);
        let Some((expiry, month_only)) = parse_date_input(date_clean) else {
            // строка без даты: либо мусор, либо лекарство без упаковок — экспорт
            // пишет его одной строкой с пустыми полями упаковки
            let empty_pkg_cells = [3, 6, 7, 8, 9, 10, 12, 13]
                .iter()
                .all(|&c| row.get(c).map(|v| v.trim().is_empty()).unwrap_or(true));
            if empty_pkg_cells {
                merge_medicine(&mut meds, name, form, note, tags, None);
            } else {
                skipped += 1;
            }
            continue;
        };
        let quantity = unesc(&row[6]).trim().to_string();
        let price = if row.len() > 7 && !row[7].trim().is_empty() {
            row[7].trim().replace(',', ".").parse::<f64>().ok()
        } else {
            None
        };
        let opened = row.get(8).and_then(|c| parse_opened_cell(c));
        let disposed = row.get(9).and_then(|c| parse_disposed_cell(c));
        let pnote = if row.len() > 10 {
            unesc(&row[10]).trim().to_string()
        } else {
            String::new()
        };
        // «Куплена» (необязательная колонка): нечитаемая дата — игнорируем
        let purchase_date = if row.len() > 12 && !row[12].trim().is_empty() {
            parse_date_input(row[12].trim()).map(|(d, _)| d)
        } else {
            None
        };
        // «Период после вскрытия» (необязательная колонка старых файлов)
        let open_period_days = row.get(13).and_then(|c| {
            let t = c.trim();
            if t.is_empty() {
                None
            } else {
                t.parse::<i32>().ok().filter(|v| (1..=3650).contains(v))
            }
        });
        let package = Package {
            id: 0,
            expiry,
            month_only,
            quantity,
            note: pnote,
            price,
            purchase_date,
            opened,
            open_period_days,
            disposed,
            photo: String::new(),
        };
        merge_medicine(&mut meds, name, form, note, tags, Some(package));
    }
    Ok((meds, skipped))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc;

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
        let mut db = Database {
            warn_days: 45,
            ..Default::default()
        };
        db.medicines.push(Arc::new(Medicine {
            id: 1,
            name: "Парацетамол \"Тева\"".to_string(),
            form: "Таблетки".to_string(),
            note: "от головы".to_string(),
            tags: vec!["жаропонижающее".to_string()],
            packages: vec![
                Package {
                    id: 1,
                    expiry: chrono::NaiveDate::from_ymd_opt(2030, 1, 1).unwrap(),
                    month_only: false,
                    quantity: "2 уп.".to_string(),
                    note: "в ящике".to_string(),
                    price: Some(350.0),
                    purchase_date: chrono::NaiveDate::from_ymd_opt(2025, 8, 15),
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
                    purchase_date: None,
                    opened: None,
                    open_period_days: None,
                    disposed: None,
                    photo: String::new(),
                },
            ],
        }));
        db
    }

    #[test]
    fn csv_export_contains_all_rows() {
        let dir = temp_dir("csv");
        let path = dir.join("export.csv");
        export_csv_to(&sample_db(), &path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.starts_with('\u{FEFF}'), "нужен BOM для Excel");
        assert!(
            text.contains("\"Парацетамол \"\"Тева\"\"\""),
            "кавычки экранируются"
        );
        assert!(text.contains("01.01.2030"));
        assert!(text.contains("350.00"), "цена попадает в CSV");
        assert!(
            text.contains("01.2028 (до конца мес.)"),
            "месячный срок помечается в CSV"
        );
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
    fn csv_tags_and_purchases_roundtrip() {
        let dir = temp_dir("tags");
        let path = dir.join("export.csv");
        let db = sample_db();
        export_csv_to(&db, &path).unwrap();
        let (meds, skipped) = import_csv(&path).unwrap();
        assert_eq!(skipped, 0);
        assert_eq!(
            meds[0].tags,
            vec!["жаропонижающее".to_string()],
            "категории переживают экспорт/импорт"
        );
        assert_eq!(
            meds[0].packages[0].purchase_date,
            chrono::NaiveDate::from_ymd_opt(2025, 8, 15),
            "дата покупки переживает экспорт/импорт"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn import_csv_tolerates_garbage() {
        let dir = temp_dir("import-bad");
        let csv = dir.join("bad.csv");
        fs::write(
            &csv,
            "\u{FEFF}Лекарство;Форма\r\n;;\r\nХлам;без;даты;не дата\r\n",
        )
        .unwrap();
        let (meds, skipped) = import_csv(&csv).unwrap();
        assert_eq!(meds.len(), 0);
        assert!(skipped >= 1);
        fs::remove_dir_all(&dir).ok();
    }

    fn lifecycle_pkg(
        id: i32,
        expiry: (i32, u32, u32),
        opened: Option<(i32, u32, u32)>,
        period: Option<i32>,
        disposed: Option<(i32, u32, u32, i32)>,
    ) -> Package {
        let d = |t: (i32, u32, u32)| chrono::NaiveDate::from_ymd_opt(t.0, t.1, t.2).unwrap();
        Package {
            id,
            expiry: d(expiry),
            month_only: false,
            quantity: String::new(),
            note: String::new(),
            price: None,
            purchase_date: None,
            opened: opened.map(d),
            open_period_days: period,
            disposed: disposed.map(|(y, m, day, reason)| DisposedInfo {
                date: d((y, m, day)),
                reason,
            }),
            photo: String::new(),
        }
    }

    #[test]
    fn csv_roundtrip_preserves_package_lifecycle() {
        let dir = temp_dir("lifecycle");
        let path = dir.join("export.csv");
        let mut db = sample_db();
        db.medicine_mut(1).unwrap().packages.push(lifecycle_pkg(
            3,
            (2030, 6, 1),
            Some((2026, 10, 1)),
            Some(30),
            None,
        ));
        db.medicine_mut(1).unwrap().packages.push(lifecycle_pkg(
            4,
            (2026, 1, 1),
            None,
            None,
            Some((2026, 10, 5, REASON_USED)),
        ));
        export_csv_to(&db, &path).unwrap();

        let (meds, skipped) = import_csv(&path).unwrap();
        assert_eq!(skipped, 0);
        assert_eq!(meds.len(), 1);
        let ps = &meds[0].packages;
        assert_eq!(ps.len(), 4);
        assert_eq!(
            ps[2].opened,
            Some(chrono::NaiveDate::from_ymd_opt(2026, 10, 1).unwrap())
        );
        assert_eq!(ps[2].open_period_days, Some(30));
        assert!(ps[3].is_disposed(), "списанная упаковка не «воскресает»");
        assert_eq!(ps[3].disposed.as_ref().unwrap().reason, REASON_USED);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn disposed_cell_parses_both_reasons() {
        let used = parse_disposed_cell("израсходовано 01.10.2026").unwrap();
        assert_eq!(used.reason, REASON_USED);
        let tossed = parse_disposed_cell("выброшено 01.10.2026").unwrap();
        assert_eq!(tossed.reason, REASON_DISCARDED);
        // текст прежних версий («просрочено, выброшено …») читается как «выброшено»
        let legacy = parse_disposed_cell("просрочено, выброшено 01.10.2026").unwrap();
        assert_eq!(legacy.reason, REASON_DISCARDED);
        assert_eq!(parse_disposed_cell("мусор"), None);
    }

    #[test]
    fn disposed_reason_text_roundtrips_through_export_import() {
        // «выброшено» (без утверждения просрочки) переживает экспорт/импорт
        let dir = temp_dir("reason");
        let path = dir.join("export.csv");
        let mut db = sample_db();
        db.medicine_mut(1).unwrap().packages.push(lifecycle_pkg(
            3,
            (2026, 1, 1),
            None,
            None,
            Some((2026, 10, 5, REASON_DISCARDED)),
        ));
        export_csv_to(&db, &path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("выброшено 05.10.2026"),
            "причина без ложного «просрочено»"
        );
        let (meds, _) = import_csv(&path).unwrap();
        assert_eq!(
            meds[0].packages[2].disposed.as_ref().unwrap().reason,
            REASON_DISCARDED,
            "причина переживает round-trip"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn import_merges_non_consecutive_rows_of_same_medicine() {
        let dir = temp_dir("merge");
        let csv = dir.join("in.csv");
        fs::write(
            &csv,
            "\u{FEFF}Лекарство;Форма;Заметки;Срок годности;Дней до конца;Статус;Количество\r\n\
             Аспирин;Таблетки;;2027-01-01;;;1 уп\r\n\
             Ибупрофен;Капсулы;;2027-02-01;;;1 уп\r\n\
             аспирин;таблетки;;2028-01-01;;;2 уп\r\n",
        )
        .unwrap();
        let (meds, skipped) = import_csv(&csv).unwrap();
        assert_eq!(skipped, 0);
        assert_eq!(
            meds.len(),
            2,
            "строки одного лекарства склеиваются и через другие, и без регистра"
        );
        let aspirin = meds.iter().find(|m| m.name == "Аспирин").unwrap();
        assert_eq!(aspirin.packages.len(), 2, "упаковки обеих строк сохранены");
        assert_eq!(aspirin.packages[0].quantity, "1 уп");
        assert_eq!(aspirin.packages[1].quantity, "2 уп");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn medicine_without_packages_survives_roundtrip() {
        let dir = temp_dir("nopkg");
        let path = dir.join("export.csv");
        let mut db = Database::default();
        db.medicines.push(Arc::new(Medicine {
            id: 1,
            name: "Пустышка".to_string(),
            form: "Прочее".to_string(),
            note: "без упаковок".to_string(),
            tags: vec!["запас".to_string()],
            packages: Vec::new(),
        }));
        export_csv_to(&db, &path).unwrap();
        let (meds, skipped) = import_csv(&path).unwrap();
        assert_eq!(skipped, 0, "строка без упаковок — не мусор");
        assert_eq!(meds.len(), 1);
        assert!(meds[0].packages.is_empty());
        assert_eq!(meds[0].note, "без упаковок");
        assert_eq!(meds[0].tags, vec!["запас".to_string()]);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn formula_injection_guard_survives_roundtrip() {
        // F12: esc добавляет апостроф перед =+-@, unesc снимает его при импорте —
        // имена с «опасным» первым символом не растут и не теряют защиту
        let dir = temp_dir("inject");
        let path = dir.join("export.csv");
        let mut db = Database::default();
        for (i, name) in ["=SUM(A1)", "'=1+1", "+7", "@cmd", "'просто", "обычное"]
            .iter()
            .enumerate()
        {
            db.medicines.push(Arc::new(Medicine {
                id: i as i32 + 1,
                name: (*name).to_string(),
                form: "Таблетки".to_string(),
                note: String::new(),
                tags: Vec::new(),
                packages: Vec::new(),
            }));
        }
        export_csv_to(&db, &path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("\"'=SUM(A1)\""),
            "добавлена защита от формулы"
        );
        let (meds, skipped) = import_csv(&path).unwrap();
        assert_eq!(skipped, 0);
        let got: Vec<String> = meds.iter().map(|m| m.name.clone()).collect();
        assert_eq!(
            got,
            vec!["=SUM(A1)", "'=1+1", "+7", "@cmd", "'просто", "обычное"],
            "round-trip без прилипшего апострофа"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn csv_export_import_export_is_idempotent() {
        // Тест №4: экспорт → импорт → экспорт даёт тот же текст
        let dir = temp_dir("idempotent");
        let p1 = dir.join("a.csv");
        let p2 = dir.join("b.csv");
        let db = sample_db();
        export_csv_to(&db, &p1).unwrap();
        let (meds, _) = import_csv(&p1).unwrap();
        let mut db2 = Database {
            warn_days: db.warn_days,
            ..Default::default()
        };
        db2.medicines = meds.into_iter().map(Arc::new).collect();
        export_csv_to(&db2, &p2).unwrap();
        assert_eq!(
            fs::read_to_string(&p1).unwrap(),
            fs::read_to_string(&p2).unwrap(),
            "повторный экспорт после импорта идентичен"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn dedup_skips_existing_medicine_by_name_and_form() {
        // Тест №1: повторный импорт того же файла → 0 новых; семантика —
        // лекарство с существующей парой (имя, форма) пропускается целиком
        let existing = sample_db().medicines; // «Парацетамол "Тева"», форма «Таблетки»
        let dup = Medicine {
            id: 0,
            name: "парацетамол \"тева\"".to_string(), // без регистра
            form: "таблетки".to_string(),
            note: String::new(),
            tags: Vec::new(),
            packages: vec![],
        };
        let fresh = Medicine {
            id: 0,
            name: "Новый".to_string(),
            form: "Капсулы".to_string(),
            note: String::new(),
            tags: Vec::new(),
            packages: vec![],
        };
        let (kept, dup_count) = dedup_imported(&existing, vec![dup.clone(), fresh, dup]);
        assert_eq!(kept.len(), 1, "осталось только новое лекарство");
        assert_eq!(kept[0].name, "Новый");
        assert_eq!(dup_count, 2, "дубль против базы + дубль внутри файла");
        // повторный импорт того же — пусто
        let (kept2, dup2) = dedup_imported(&existing, vec![]);
        assert!(kept2.is_empty());
        assert_eq!(dup2, 0);
    }

    #[test]
    fn reimport_of_same_file_adds_no_duplicates() {
        // контракт README: повторный импорт того же CSV не плодит дубли.
        // Лекарство с известной парой (имя, форма) отбрасывается ЦЕЛИКОМ —
        // вместе со своими упаковками, даже новыми.
        let dir = temp_dir("reimport");
        let path = dir.join("exp.csv");
        export_csv_to(&sample_db(), &path).unwrap();
        let (imported, skipped) = import_csv(&path).unwrap();
        assert_eq!(skipped, 0);
        assert_eq!(imported.len(), 1, "в файле одно лекарство");
        let (kept, dups) = dedup_imported(&sample_db().medicines, imported);
        assert!(kept.is_empty(), "повторный импорт не добавляет лекарств");
        assert_eq!(dups, 1);
        // новые упаковки у уже известного лекарства тоже не проходят
        let mut more = sample_db();
        let extra = more.medicines[0].packages[0].clone();
        more.medicine_mut(1).unwrap().packages.push(extra);
        let (kept2, dups2) = dedup_imported(
            &sample_db().medicines,
            more.medicines
                .into_iter()
                .map(Arc::unwrap_or_clone)
                .collect(),
        );
        assert!(
            kept2.is_empty(),
            "лекарство отбрасывается вместе со всеми упаковками"
        );
        assert_eq!(dups2, 1);
        fs::remove_dir_all(&dir).ok();
    }
}
