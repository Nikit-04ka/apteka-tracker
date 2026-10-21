//! Чистая логика валидации и расчётов, переносимая из UI: форма упаковки,
//! календарная сетка, навигация по списку. Не содержит UI-типов.

use crate::model::{parse_date_input, Package};
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
}

impl PackageForm {
    /// Разбор всех пяти строк формы. `quantity`/`note` — текст как есть;
    /// срок, период и цена — через `parse_package_input` (Err — готовое
    /// сообщение для показа пользователю).
    pub fn parse(
        expiry: &str,
        quantity: &str,
        note: &str,
        price: &str,
        period: &str,
    ) -> Result<Self, String> {
        let input = parse_package_input(expiry, period, price)?;
        Ok(Self {
            expiry: input.expiry,
            month_only: input.month_only,
            quantity: quantity.to_string(),
            note: note.to_string(),
            price: input.price,
            open_period_days: input.open_period_days,
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
            opened: None,
            open_period_days: self.open_period_days,
            disposed: None,
            photo: String::new(),
        }
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
    }
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
    let rows_count = (first.weekday().num_days_from_monday() as u32 + days_in_month + 6) / 7;
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
        PackageForm::parse(expiry, "2 уп.", "аптечка", price, period).unwrap()
    }

    fn pkg() -> Package {
        Package {
            id: 7,
            expiry: d("2027-01-01"),
            month_only: false,
            quantity: "старое".to_string(),
            note: "старая заметка".to_string(),
            price: Some(100.0),
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
        assert!(PackageForm::parse("мусор", "", "", "", "").is_err());
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
        // жизненный цикл не тронут
        assert_eq!(p.id, 7);
        assert_eq!(p.opened, Some(d("2026-10-01")));
        assert!(p.disposed.is_some());
        assert_eq!(p.photo, "pkg_7.png");
    }

    #[test]
    fn package_form_into_package_starts_fresh() {
        let p = form("01.2028", "12", "30").into_package(42);
        assert_eq!(p.id, 42);
        assert_eq!(p.expiry, d("2028-01-31"));
        assert!(p.month_only);
        assert_eq!(p.price, Some(12.0));
        assert_eq!(p.open_period_days, Some(30));
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
}
