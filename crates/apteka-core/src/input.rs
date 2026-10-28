//! Чистая логика валидации и расчётов, переносимая из UI: форма упаковки,
//! календарная сетка, навигация по списку. Не содержит UI-типов.

use crate::model::{
    medicine_info, parse_date_input, Database, MedInfo, Medicine, Package, STATUS_EXPIRED,
    STATUS_SOON,
};
use chrono::{Datelike, NaiveDate};

/// Результат валидации формы упаковки.
#[derive(Debug, Clone, PartialEq)]
pub struct PackageInput {
    pub expiry: NaiveDate,
    pub month_only: bool,
    pub price: Option<f64>,
    pub open_period_days: Option<i32>,
}

const HINT_FORMAT: &str = "Формат: ГГГГ-ММ-ДД, ДД.ММ.ГГГГ или ММ.ГГГГ (годен до конца месяца)";

/// Разбор и валидация полей формы упаковки.
/// `Ok` — все поля корректны; `Err(сообщение)` — текст для показа пользователю
/// (сообщение, начинающееся с «Формат:», считается нейтральной подсказкой, не ошибкой).
pub fn parse_package_input(
    expiry: &str,
    period: &str,
    price: &str,
) -> Result<PackageInput, String> {
    let expiry_trimmed = expiry.trim();
    if expiry_trimmed.is_empty() {
        return Err(HINT_FORMAT.to_string());
    }
    let Some((expiry, month_only)) = parse_date_input(expiry_trimmed) else {
        return Err("Неверная дата".to_string());
    };
    let open_period_days = match period.trim() {
        "" => None,
        t => match t.parse::<i32>() {
            Ok(v) if (1..=3650).contains(&v) => Some(v),
            _ => return Err("Период после вскрытия: целое число дней от 1 до 3650".to_string()),
        },
    };
    let price = match price.trim() {
        "" => None,
        t => match t.replace(',', ".").parse::<f64>() {
            Ok(v) if (0.0..=1_000_000.0).contains(&v) => Some(v),
            _ => return Err("Цена: число, например 350 или 249.99".to_string()),
        },
    };
    Ok(PackageInput {
        expiry,
        month_only,
        price,
        open_period_days,
    })
}

/// Разобранная форма упаковки — единая точка «форма ↔ Package»: создание
/// новой и правка существующей упаковки идут через неё, а не через
/// поштучное присвоение полей в UI-колбэке.
#[derive(Debug, Clone, PartialEq)]
pub struct PackageForm {
    pub expiry: NaiveDate,
    pub month_only: bool,
    pub quantity: String,
    pub note: String,
    pub price: Option<f64>,
    pub open_period_days: Option<i32>,
    pub purchase_date: Option<NaiveDate>,
}

impl PackageForm {
    /// Разбор всех строк формы. `quantity`/`note` — текст как есть;
    /// срок, период, цена и дата покупки — через парсеры (`Err` — готовое
    /// сообщение для показа пользователю).
    pub fn parse(
        expiry: &str,
        quantity: &str,
        note: &str,
        price: &str,
        period: &str,
        purchased: &str,
    ) -> Result<Self, String> {
        let input = parse_package_input(expiry, period, price)?;
        let purchase_date = match purchased.trim() {
            "" => None,
            t => match parse_date_input(t) {
                Some((d, _)) => Some(d),
                None => {
                    return Err(
                        "Дата покупки: ГГГГ-ММ-ДД или ДД.ММ.ГГГГ (можно оставить пустым)"
                            .to_string(),
                    )
                }
            },
        };
        Ok(Self {
            expiry: input.expiry,
            month_only: input.month_only,
            quantity: quantity.to_string(),
            note: note.to_string(),
            price: input.price,
            open_period_days: input.open_period_days,
            purchase_date,
        })
    }

    /// Новая упаковка: жизненный цикл пуст (запечатана, не списана, без фото).
    pub fn into_package(self, id: i32) -> Package {
        Package {
            id,
            expiry: self.expiry,
            month_only: self.month_only,
            quantity: self.quantity,
            note: self.note,
            price: self.price,
            purchase_date: self.purchase_date,
            opened: None,
            open_period_days: self.open_period_days,
            disposed: None,
            photo: String::new(),
        }
    }

    /// Совпадают ли поля формы с упаковкой (те же, что трогает `apply_to`).
    /// Правка без изменений не должна создавать шаг истории и событие журнала.
    pub fn matches(&self, p: &Package) -> bool {
        self.expiry == p.expiry
            && self.month_only == p.month_only
            && self.quantity == p.quantity
            && self.note == p.note
            && self.price == p.price
            && self.open_period_days == p.open_period_days
            && self.purchase_date == p.purchase_date
    }

    /// Правка существующей упаковки: обновляются только поля формы —
    /// id, вскрытие, списание и фото не трогаются.
    pub fn apply_to(&self, p: &mut Package) {
        p.expiry = self.expiry;
        p.month_only = self.month_only;
        p.quantity = self.quantity.clone();
        p.note = self.note.clone();
        p.price = self.price;
        p.open_period_days = self.open_period_days;
        p.purchase_date = self.purchase_date;
    }
}

/// Строка haystack для поиска: поля через пробел. Собирается в переиспользуемый
/// буфер без аллокаций; приведение к нижнему регистру (1 аллокация) — у
/// вызывающего. Старый вариант аллоцировал три строки на лекарство на каждый
/// символ поиска.
fn fill_hay(hay: &mut String, m: &Medicine) {
    hay.clear();
    hay.push_str(&m.name);
    hay.push(' ');
    hay.push_str(&m.form);
    hay.push(' ');
    hay.push_str(&m.note);
    hay.push(' ');
    for (i, t) in m.tags.iter().enumerate() {
        if i > 0 {
            hay.push(' ');
        }
        hay.push_str(t);
    }
}

/// Все уникальные категории базы (уникальность без регистра, первая форма
/// написания), отсортированные по алфавиту.
pub fn all_tags(db: &Database) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for m in &db.medicines {
        for t in &m.tags {
            let lower = t.to_lowercase();
            if !out.iter().any(|e| e.to_lowercase() == lower) {
                out.push(t.clone());
            }
        }
    }
    out.sort_by_cached_key(|a| a.to_lowercase());
    out
}

/// Отфильтрованный и отсортированный список лекарств для отображения.
///
/// Фильтр: `filter_idx` 0 — все, 1 — только «истекает», 2 — только
/// просроченные. Поиск — без регистра по названию, форме, заметке и
/// категориям. `tag` — категория-фильтр (None — все). Сортировка:
/// `sort_idx` 0 — по ближайшему сроку (без упаковок — в конец), иначе —
/// по названию.
pub fn visible_medicines<'a>(
    db: &'a Database,
    search: &str,
    tag: Option<&str>,
    filter_idx: usize,
    sort_idx: usize,
    today: NaiveDate,
) -> Vec<(&'a Medicine, MedInfo)> {
    let warn = db.warn_days as i64;
    let needle = search.trim().to_lowercase();
    let mut rows: Vec<(&Medicine, MedInfo)> = Vec::new();
    // haystack собирается в переиспользуемый буфер; регистр приводится один раз
    // на лекарство (1 аллокация вместо трёх на каждый символ поиска)
    let mut hay = String::new();
    for m in &db.medicines {
        if !needle.is_empty() {
            fill_hay(&mut hay, m);
            if !hay.to_lowercase().contains(&needle) {
                continue;
            }
        }
        if let Some(tag) = tag {
            let tag_l = tag.to_lowercase();
            if !m.tags.iter().any(|t| t.to_lowercase() == tag_l) {
                continue;
            }
        }
        let info = medicine_info(m, today, warn);
        if filter_idx == 1 && info.status != STATUS_SOON {
            continue;
        }
        if filter_idx == 2 && info.status != STATUS_EXPIRED {
            continue;
        }
        rows.push((m, info));
    }
    match sort_idx {
        // sort_by_cached_key: ключ (с аллокацией to_lowercase) вычисляется
        // один раз на элемент, а не на каждом сравнении (sort_by_key)
        0 => rows.sort_by_cached_key(|(m, info)| {
            let key = info.nearest.map(|_| info.nearest_days).unwrap_or(i64::MAX);
            (key, m.name.to_lowercase())
        }),
        _ => rows.sort_by_cached_key(|a| a.0.name.to_lowercase()),
    }
    rows
}

/// Индекс в видимой списке после сдвига `delta` от `current`.
/// None — список пуст. Текущее выделение вне списка: вверх — последняя строка, вниз — первая.
pub fn move_selection_index(ids: &[i32], current: i32, delta: i32) -> Option<usize> {
    if ids.is_empty() {
        return None;
    }
    Some(match ids.iter().position(|&id| id == current) {
        Some(i) => ((i as i64) + delta as i64).clamp(0, ids.len() as i64 - 1) as usize,
        None if delta < 0 => ids.len() - 1,
        None => 0,
    })
}

/// Ячейка календарной сетки: (подпись, ymd, день текущего месяца, сегодня).
pub type CalCellTuple = (String, String, bool, bool);

/// Сетка 6×7 (Пн-первый день недели) для месяца; соседние месяцы приглушены,
/// но кликабельны (ymd заполнен всегда). None — невозможный год/месяц.
pub fn cal_grid(year: i32, month: u32, today: NaiveDate) -> Option<Vec<Vec<CalCellTuple>>> {
    let first = NaiveDate::from_ymd_opt(year, month, 1)?;
    let week_start = first - chrono::Duration::days(first.weekday().num_days_from_monday() as i64);
    // строк сетки — ровно столько, сколько нужно месяцу (5 или 6)
    let next_month = first + chrono::Months::new(1);
    let days_in_month = (next_month - first).num_days() as u32;
    let rows_count = (first.weekday().num_days_from_monday() + days_in_month).div_ceil(7);
    let mut rows = Vec::with_capacity(rows_count as usize);
    for w in 0..rows_count {
        let mut row = Vec::with_capacity(7);
        for d in 0..7u32 {
            let day = week_start + chrono::Duration::days((w * 7 + d) as i64);
            row.push((
                format!("{}", day.day()),
                day.format("%Y-%m-%d").to_string(),
                day.month() == month && day.year() == year,
                day == today,
            ));
        }
        rows.push(row);
    }
    Some(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn parse_ok_full_and_price_with_comma() {
        let r = parse_package_input("2027-03-01", "30", "350,50").unwrap();
        assert_eq!(r.expiry, d("2027-03-01"));
        assert!(!r.month_only);
        assert_eq!(r.price, Some(350.5));
        assert_eq!(r.open_period_days, Some(30));
    }

    #[test]
    fn parse_month_only_sets_flag() {
        let r = parse_package_input("01.2028", "", "").unwrap();
        assert_eq!(r.expiry, d("2028-01-31"));
        assert!(r.month_only);
        assert_eq!(r.price, None);
    }

    #[test]
    fn parse_errors_for_bad_period_and_price() {
        assert!(parse_package_input("2027-03-01", "0", "").is_err());
        assert!(parse_package_input("2027-03-01", "3651", "").is_err());
        assert!(parse_package_input("2027-03-01", "abc", "").is_err());
        assert!(parse_package_input("2027-03-01", "", "abc").is_err());
        assert!(parse_package_input("2027-03-01", "", "-1").is_err());
        assert!(parse_package_input("2027-03-01", "", "1000001").is_err());
    }

    #[test]
    fn neutral_hint_when_expiry_empty() {
        let err = parse_package_input("", "", "").unwrap_err();
        assert!(
            err.starts_with("Формат:"),
            "пустая дата — подсказка, не ошибка"
        );
    }

    fn form(expiry: &str, price: &str, period: &str) -> PackageForm {
        PackageForm::parse(expiry, "2 уп.", "аптечка", price, period, "").unwrap()
    }

    fn pkg() -> Package {
        Package {
            id: 7,
            expiry: d("2027-01-01"),
            month_only: false,
            quantity: "старое".to_string(),
            note: "старая заметка".to_string(),
            price: Some(100.0),
            purchase_date: Some(d("2026-09-20")),
            opened: Some(d("2026-10-01")),
            open_period_days: Some(30),
            disposed: Some(crate::model::DisposedInfo {
                date: d("2026-10-05"),
                reason: 1,
            }),
            photo: "pkg_7.png".to_string(),
        }
    }

    #[test]
    fn package_form_parse_carries_all_fields() {
        let f = form("01.2028", "350,50", "90");
        assert_eq!(f.expiry, d("2028-01-31"));
        assert!(f.month_only);
        assert_eq!(f.price, Some(350.5));
        assert_eq!(f.open_period_days, Some(90));
        assert_eq!(f.quantity, "2 уп.");
        assert_eq!(f.note, "аптечка");
        assert!(PackageForm::parse("мусор", "", "", "", "", "").is_err());
    }

    #[test]
    fn package_form_purchase_date_optional_but_validated() {
        let f = PackageForm::parse("2028-01-15", "", "", "", "", "05.03.2026").unwrap();
        assert_eq!(f.purchase_date, Some(d("2026-03-05")));
        let empty = PackageForm::parse("2028-01-15", "", "", "", "", "  ").unwrap();
        assert_eq!(empty.purchase_date, None, "пустое поле — None");
        assert!(PackageForm::parse("2028-01-15", "", "", "", "", "мусор").is_err());
    }

    #[test]
    fn package_form_apply_touches_only_form_fields() {
        let mut p = pkg();
        form("2028-05-10", "", "").apply_to(&mut p);
        assert_eq!(p.expiry, d("2028-05-10"));
        assert!(!p.month_only);
        assert_eq!(p.quantity, "2 уп.");
        assert_eq!(p.note, "аптечка");
        assert_eq!(p.price, None);
        assert_eq!(p.open_period_days, None);
        assert_eq!(
            p.purchase_date, None,
            "дата покупки — поле формы, применяется"
        );
        // жизненный цикл не тронут
        assert_eq!(p.id, 7);
        assert_eq!(p.opened, Some(d("2026-10-01")));
        assert!(p.disposed.is_some());
        assert_eq!(p.photo, "pkg_7.png");
    }

    #[test]
    fn package_form_matches_mirrors_apply_to_fields() {
        let mut p = pkg();
        // форма, повторяющая все поля формы упаковки (включая «Куплена»)
        let same = PackageForm::parse(
            "2027-01-01",
            "старое",
            "старая заметка",
            "100",
            "30",
            "2026-09-20",
        )
        .unwrap();
        assert!(same.matches(&p), "все поля формы совпадают");
        // жизненный цикл (вскрытие, списание, фото) совпадение не определяет
        p.opened = None;
        p.disposed = None;
        p.photo = String::new();
        assert!(same.matches(&p), "жизненный цикл не влияет на совпадение");
        // любое поле формы делает форму «изменённой»
        for f in [
            form("2028-05-10", "", ""),
            form("2027-01-01", "12", ""),
            PackageForm::parse("2027-01-01", "иное", "", "100", "30", "2026-09-20").unwrap(),
            PackageForm::parse("2027-01-01", "старое", "иная", "100", "30", "2026-09-20").unwrap(),
            PackageForm::parse(
                "2027-01-01",
                "старое",
                "старая заметка",
                "100",
                "",
                "2026-09-20",
            )
            .unwrap(),
            PackageForm::parse("2027-01-01", "старое", "старая заметка", "100", "30", "").unwrap(),
        ] {
            assert!(!f.matches(&p), "изменённая форма не должна совпадать");
        }
        // после применения форма совпадает с упаковкой
        let other = form("2028-05-10", "", "");
        other.apply_to(&mut p);
        assert!(other.matches(&p));
    }

    #[test]
    fn package_form_into_package_starts_fresh() {
        let p = form("01.2028", "12", "30").into_package(42);
        assert_eq!(p.id, 42);
        assert_eq!(p.expiry, d("2028-01-31"));
        assert!(p.month_only);
        assert_eq!(p.price, Some(12.0));
        assert_eq!(p.open_period_days, Some(30));
        assert_eq!(p.purchase_date, None);
        assert_eq!(p.opened, None);
        assert_eq!(p.disposed, None);
        assert_eq!(p.photo, "");
    }

    #[test]
    fn move_selection_clamps_and_recovers() {
        let ids = [10, 20, 30];
        assert_eq!(move_selection_index(&ids, 20, 1), Some(2));
        assert_eq!(move_selection_index(&ids, 30, 5), Some(2), "клэмп внизу");
        assert_eq!(move_selection_index(&ids, 10, -5), Some(0), "клэмп вверху");
        // выделение вне списка: вверх — последняя, вниз — первая
        assert_eq!(move_selection_index(&ids, 999, -1), Some(2));
        assert_eq!(move_selection_index(&ids, 999, 1), Some(0));
        assert_eq!(move_selection_index(&[], 1, 1), None);
    }

    #[test]
    fn cal_grid_wraps_year_and_marks_today() {
        let grid = cal_grid(2026, 12, d("2026-12-31")).unwrap();
        // декабрь 2026: вторник-старт, 31 день → 5 недель
        assert_eq!(grid.len(), 5);
        assert_eq!(grid[0].len(), 7);
        // 1 декабря 2026 — вторник; первая ячейка — понедельник 30 ноября (чужой месяц)
        assert_eq!(grid[0][0].0, "30");
        assert!(!grid[0][0].2);
        // 31 декабря — четверг недели 4 (столбец Чт)
        assert_eq!(grid[4][3].0, "31");
        assert!(grid[4][3].2);
        // последняя ячейка сетки — 3 января следующего года, приглушена
        assert_eq!(grid[4][6].0, "3");
        assert!(!grid[4][6].2);
    }

    #[test]
    fn cal_grid_first_cell_is_monday() {
        let grid = cal_grid(2026, 10, d("2026-10-20")).unwrap();
        // октябрь 2026: четверг-старт, 31 день → 5 недель
        assert_eq!(grid.len(), 5);
        // 1 октября 2026 — четверг; понедельник перед ним — 28 сентября
        assert_eq!(grid[0][0].0, "28");
        assert!(!grid[0][0].2, "дни соседнего месяца приглушены");
        // последняя ячейка сетки — 1 ноября, приглушена
        assert_eq!(grid[4][6].0, "1");
        assert!(!grid[4][6].2);
    }

    fn vm_med(id: i32, name: &str, form: &str, note: &str, in_days: Option<i64>) -> Arc<Medicine> {
        let packages = match in_days {
            None => Vec::new(),
            Some(n) => vec![Package {
                id,
                expiry: d("2026-10-20") + chrono::Duration::days(n),
                month_only: false,
                quantity: String::new(),
                note: String::new(),
                price: None,
                purchase_date: None,
                opened: None,
                open_period_days: None,
                disposed: None,
                photo: String::new(),
            }],
        };
        Arc::new(Medicine {
            id,
            name: name.to_string(),
            form: form.to_string(),
            note: note.to_string(),
            tags: Vec::new(),
            packages,
        })
    }

    fn vm_db() -> Database {
        let mut db = Database {
            warn_days: 60,
            ..Default::default()
        };
        // 2026-10-20: ОК (через 100 дн.), истекает (30), просрочен (-5)
        db.medicines
            .push(vm_med(1, "Парацетамол", "Таблетки", "от головы", Some(100)));
        db.medicines
            .push(vm_med(2, "Ибупрофен", "Таблетки", "", Some(30)));
        db.medicines
            .push(vm_med(3, "Аспирин", "Шипучий", "аптечка", Some(-5)));
        db
    }

    #[test]
    fn visible_search_is_case_insensitive_across_fields() {
        let today = d("2026-10-20");
        let db = vm_db();
        let names = |v: Vec<(&Medicine, MedInfo)>| -> Vec<String> {
            v.into_iter().map(|(m, _)| m.name.clone()).collect()
        };
        assert_eq!(
            names(visible_medicines(&db, "", None, 0, 0, today)).len(),
            3
        );
        assert_eq!(
            names(visible_medicines(&db, "пара", None, 0, 0, today)),
            vec!["Парацетамол".to_string()],
            "поиск по названию без регистра"
        );
        assert_eq!(
            names(visible_medicines(&db, "ГОЛОВЫ", None, 0, 0, today)),
            vec!["Парацетамол".to_string()],
            "поиск по заметке"
        );
        assert_eq!(
            names(visible_medicines(&db, "шипуч", None, 0, 0, today)),
            vec!["Аспирин".to_string()],
            "поиск по форме"
        );
        assert!(visible_medicines(&db, "мазь", None, 0, 0, today).is_empty());
    }

    #[test]
    fn visible_filter_by_status() {
        let today = d("2026-10-20");
        let db = vm_db();
        let names = |v: Vec<(&Medicine, MedInfo)>| -> Vec<String> {
            v.into_iter().map(|(m, _)| m.name.clone()).collect()
        };
        assert_eq!(
            names(visible_medicines(&db, "", None, 1, 0, today)),
            vec!["Ибупрофен".to_string()],
            "фильтр «истекает»"
        );
        assert_eq!(
            names(visible_medicines(&db, "", None, 2, 0, today)),
            vec!["Аспирин".to_string()],
            "фильтр «просрочено»"
        );
        assert!(
            visible_medicines(&db, "пара", None, 1, 0, today).is_empty(),
            "фильтр + поиск"
        );
    }

    #[test]
    fn visible_search_matches_tags() {
        let today = d("2026-10-20");
        let mut db = vm_db();
        db.medicine_mut(2).unwrap().tags = vec!["обезболивающее".to_string()];
        let names: Vec<String> = visible_medicines(&db, "обезбол", None, 0, 0, today)
            .into_iter()
            .map(|(m, _)| m.name.clone())
            .collect();
        assert_eq!(names, vec!["Ибупрофен".to_string()], "поиск по категории");
    }

    #[test]
    fn visible_filter_by_tag() {
        let today = d("2026-10-20");
        let mut db = vm_db();
        db.medicine_mut(1).unwrap().tags = vec!["Жаропонижающее".to_string()];
        db.medicine_mut(3).unwrap().tags = vec!["жаропонижающее".to_string()];
        assert_eq!(
            all_tags(&db),
            vec!["Жаропонижающее"],
            "уникальность без регистра, сортировка"
        );
        let names: Vec<String> = visible_medicines(&db, "", Some("жаропонижающее"), 0, 0, today)
            .into_iter()
            .map(|(m, _)| m.name.clone())
            .collect();
        assert_eq!(
            names,
            vec!["Аспирин", "Парацетамол"],
            "фильтр по категории без регистра (сорт. по сроку: -5 дн. раньше +100)"
        );
        // категория + статусный фильтр вместе
        let names: Vec<String> = visible_medicines(&db, "", Some("жаропонижающее"), 2, 0, today)
            .into_iter()
            .map(|(m, _)| m.name.clone())
            .collect();
        assert_eq!(names, vec!["Аспирин"]);
        // несуществующая категория — пусто
        assert!(visible_medicines(&db, "", Some("антибиотик"), 0, 0, today).is_empty());
    }

    #[test]
    fn visible_sort_by_expiry_then_by_name() {
        let today = d("2026-10-20");
        let mut db = Database {
            warn_days: 60,
            ..Default::default()
        };
        db.medicines.push(vm_med(1, "Бэта", "Т", "", Some(10)));
        db.medicines.push(vm_med(2, "Альфа", "Т", "", Some(40)));
        db.medicines.push(vm_med(3, "Вега", "Т", "", None));
        let order = |v: Vec<(&Medicine, MedInfo)>| -> Vec<String> {
            v.into_iter().map(|(m, _)| m.name.clone()).collect()
        };
        assert_eq!(
            order(visible_medicines(&db, "", None, 0, 0, today)),
            vec!["Бэта", "Альфа", "Вега"],
            "по сроку; без упаковок — в конец"
        );
        assert_eq!(
            order(visible_medicines(&db, "", None, 0, 1, today)),
            vec!["Альфа", "Бэта", "Вега"],
            "по названию"
        );
    }
}
