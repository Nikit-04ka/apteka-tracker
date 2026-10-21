//! Чистая логика валидации и расчётов, переносимая из UI: форма упаковки,
//! календарная сетка, навигация по списку. Не содержит UI-типов.

use crate::model::parse_date_input;
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
pub fn parse_package_input(expiry: &str, period: &str, price: &str) -> Result<PackageInput, String> {
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
            _ => {
                return Err(
                    "Период после вскрытия: целое число дней от 1 до 3650".to_string(),
                )
            }
        },
    };
    let price = match price.trim() {
        "" => None,
        t => match t.replace(',', ".").parse::<f64>() {
            Ok(v) if (0.0..=1_000_000.0).contains(&v) => Some(v),
            _ => return Err("Цена: число, например 350 или 249.99".to_string()),
        },
    };
    Ok(PackageInput { expiry, month_only, price, open_period_days })
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
pub fn cal_grid(
    year: i32,
    month: u32,
    today: NaiveDate,
) -> Option<Vec<Vec<CalCellTuple>>> {
    let first = NaiveDate::from_ymd_opt(year, month, 1)?;
    let week_start = first - chrono::Duration::days(first.weekday().num_days_from_monday() as i64);
    let mut rows = Vec::with_capacity(6);
    for w in 0..6u32 {
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
        let r = parse_package_input("01.2028 (до конца мес.)", "", "").unwrap();
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
        assert!(err.starts_with("Формат:"), "пустая дата — подсказка, не ошибка");
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
        assert_eq!(grid.len(), 6);
        assert_eq!(grid.len() * grid[0].len(), 42);
        // 1 декабря 2026 — вторник; первая ячейка — понедельник 30 ноября (чужой месяц)
        assert_eq!(grid[0][0].0, "30");
        assert!(!grid[0][0].2);
        // 31 декабря — свой месяц, «сегодня»
        let last = &grid[5][6];
        assert_eq!(last.0, "31");
        assert!(last.2);
        assert!(last.3);
        // переход года в следующем месяце не валиден только по границам года
        assert!(cal_grid(2026, 12, d("2026-12-31")).is_some());
        assert!(cal_grid(2027, 1, d("2027-01-01")).is_some());
    }

    #[test]
    fn cal_grid_first_cell_is_monday() {
        let grid = cal_grid(2026, 10, d("2026-10-20")).unwrap();
        // 1 октября 2026 — четверг; понедельник перед ним — 28 сентября
        assert_eq!(grid[0][0].0, "28");
        assert!(!grid[0][0].2, "дни соседнего месяца приглушены");
    }
}
