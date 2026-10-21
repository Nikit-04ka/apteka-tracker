//! Доменная модель: лекарства, упаковки, статусы сроков годности.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Категории лекарственной формы (единый источник для UI и CSV).
pub const FORMS: &[&str] = &[
    "Таблетки",
    "Капсулы",
    "Мазь",
    "Гель",
    "Сироп",
    "Капли",
    "Спрей",
    "Инъекции",
    "Свечи",
    "Порошок",
    "Прочее",
];

/// Статусы упаковки/лекарства. Значения передаются в UI как int.
pub const STATUS_OK: i32 = 0;
pub const STATUS_SOON: i32 = 1;
pub const STATUS_EXPIRED: i32 = 2;
pub const STATUS_EMPTY: i32 = 3;
pub const STATUS_DISPOSED: i32 = 4;

/// Причина списания упаковки.
pub const REASON_USED: i32 = 0;
pub const REASON_EXPIRED: i32 = 1;

/// Факт списания упаковки (израсходована или выброшена).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisposedInfo {
    pub date: NaiveDate,
    pub reason: i32,
}

pub fn dispose_reason_text(reason: i32) -> &'static str {
    match reason {
        REASON_USED => "израсходовано",
        REASON_EXPIRED => "просрочено, выброшено",
        _ => "списано",
    }
}

/// Одна упаковка лекарства со своим сроком годности и жизненным циклом:
/// запечатана → открыта → списана.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub id: i32,
    /// Заводской срок годности. Для `month_only` — последний день указанного месяца.
    pub expiry: NaiveDate,
    /// Производитель указал только месяц и год («Годен до 01 2028»):
    /// препарат годен до конца этого месяца.
    #[serde(default)]
    pub month_only: bool,
    #[serde(default)]
    pub quantity: String,
    #[serde(default)]
    pub note: String,
    /// Цена упаковки при покупке, руб. (None — не указана).
    #[serde(default)]
    pub price: Option<f64>,
    /// Когда куплена (None — не указана; для аналитики затрат).
    #[serde(default)]
    pub purchase_date: Option<NaiveDate>,
    /// Дата вскрытия (None — запечатана).
    #[serde(default)]
    pub opened: Option<NaiveDate>,
    /// Период после вскрытия, дней (напр. 30 для глазных капель, 180 для сиропа).
    #[serde(default)]
    pub open_period_days: Option<i32>,
    /// Списание — упаковка скрыта из активного учёта, но остаётся в базе.
    #[serde(default)]
    pub disposed: Option<DisposedInfo>,
    /// Имя файла фото в каталоге `photos/` рядом с exe (пусто — нет фото).
    #[serde(default)]
    pub photo: String,
}

impl Package {
    pub fn is_disposed(&self) -> bool {
        self.disposed.is_some()
    }

    /// Реальный срок годности: заводской, а при вскрытии с периодом —
    /// более ранний из «заводского» и «вскрытие + период».
    pub fn effective_expiry(&self) -> NaiveDate {
        match (self.opened, self.open_period_days) {
            (Some(opened), Some(days)) => {
                let after_open = opened + chrono::Duration::days(days as i64);
                if after_open < self.expiry {
                    after_open
                } else {
                    self.expiry
                }
            }
            _ => self.expiry,
        }
    }
}

/// Лекарство. Может содержать несколько упаковок с разными сроками.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Medicine {
    pub id: i32,
    pub name: String,
    #[serde(default)]
    pub form: String,
    #[serde(default)]
    pub note: String,
    /// Категории/теги (напр. «жаропонижающее») — свободные, без дублей.
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub packages: Vec<Package>,
}

/// Событие журнала: что и когда произошло с лекарством/упаковкой.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEvent {
    pub at: chrono::NaiveDateTime,
    pub kind: String,
    pub medicine: String,
    pub package: String,
}

/// Разбор строки категорий: разделители — запятая или точка с запятой,
/// пустые и повторяющиеся (без регистра) отбрасываются.
pub fn parse_tags(text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in text.split([',', ';']) {
        let t = part.trim();
        if t.is_empty() {
            continue;
        }
        let lower = t.to_lowercase();
        if !out.iter().any(|e| e.to_lowercase() == lower) {
            out.push(t.to_string());
        }
    }
    out
}

/// Всё, что сохраняется на диск (JSON рядом с exe).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Database {
    /// Версия схемы файла; `#[serde(default)]` — старые файлы без поля
    /// читаются как версия 0 и нормализуются при загрузке (см. storage).
    #[serde(default)]
    pub version: u32,
    /// Порог «истекает скоро» в днях.
    pub warn_days: i32,
    pub next_med_id: i32,
    pub next_pkg_id: i32,
    #[serde(default)]
    pub medicines: Vec<Medicine>,
    /// Журнал событий (хранятся последние 400).
    #[serde(default)]
    pub events: Vec<JournalEvent>,
}

/// Текущая версия схемы файла данных. Меняя структуру `Database`,
/// увеличить и добавить миграцию в `storage::migrate`.
pub const DATA_VERSION: u32 = 1;

impl Database {
    /// Миграции схемы: приводит базу любой прочитанной версии к текущей.
    /// Вызывается после каждой загрузки.
    pub fn migrate(&mut self) {
        if self.version == 0 {
            // файлы до появления явной версии — считаем первой версией
            self.version = DATA_VERSION;
        }
        // будущее: match self.version { 0 => ..., 1 => {} , v => *self.version = DATA_VERSION }
        self.version = DATA_VERSION;
    }
}

impl Default for Database {
    fn default() -> Self {
        Self {
            version: DATA_VERSION,
            warn_days: 60,
            next_med_id: 1,
            next_pkg_id: 1,
            medicines: Vec::new(),
            events: Vec::new(),
        }
    }
}

impl Database {
    pub fn new_med_id(&mut self) -> i32 {
        let id = self.next_med_id;
        self.next_med_id += 1;
        id
    }

    pub fn new_pkg_id(&mut self) -> i32 {
        let id = self.next_pkg_id;
        self.next_pkg_id += 1;
        id
    }

    pub fn medicine(&self, id: i32) -> Option<&Medicine> {
        self.medicines.iter().find(|m| m.id == id)
    }

    pub fn medicine_mut(&mut self, id: i32) -> Option<&mut Medicine> {
        self.medicines.iter_mut().find(|m| m.id == id)
    }

    /// Добавляет событие в журнал (хранятся последние 400).
    pub fn journal_push(&mut self, kind: &str, medicine: &str, package: &str) {
        self.events.push(JournalEvent {
            at: chrono::Local::now().naive_local(),
            kind: kind.to_string(),
            medicine: medicine.to_string(),
            package: package.to_string(),
        });
        if self.events.len() > 400 {
            self.events.remove(0);
        }
    }
}

/// Агрегированная информация о лекарстве по его упаковкам.
#[derive(Debug, Clone, Copy)]
pub struct MedInfo {
    pub status: i32,
    pub nearest: Option<NaiveDate>,
    pub nearest_days: i64,
    pub nearest_month_only: bool,
}

/// Парсит ввод срока годности.
///
/// Полные даты: `ГГГГ-ММ-ДД`, `ДД.ММ.ГГГГ` → `(дата, false)`.
/// Только месяц: `ММ.ГГГГ`, `ГГГГ-ММ`, `ММ/ГГГГ`, `ММ ГГГГ` → `(последний день месяца, true)`.
///
/// Аптечная конвенция: «Годен до 01 2028» означает «годен до конца
/// января 2028», поэтому эффективная дата — 31.01.2028.
pub fn parse_date_input(s: &str) -> Option<(NaiveDate, bool)> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    for fmt in ["%Y-%m-%d", "%d.%m.%Y"] {
        if let Ok(d) = NaiveDate::parse_from_str(s, fmt) {
            return Some((d, false));
        }
    }
    // Только месяц и год: нормализуем разделители -, /, пробел в точку.
    let norm: String = s
        .chars()
        .map(|c| {
            if matches!(c, '-' | '/' | ' ' | ',') {
                '.'
            } else {
                c
            }
        })
        .collect();
    let parts: Vec<&str> = norm.split('.').filter(|p| !p.is_empty()).collect();
    if parts.len() != 2 {
        return None;
    }
    // Год — более длинная часть: «01.2028» и «2028.01» равнозначны.
    let (ys, ms) = if parts[0].len() >= parts[1].len() {
        (parts[0], parts[1])
    } else {
        (parts[1], parts[0])
    };
    let Ok(year) = ys.parse::<i32>() else {
        return None;
    };
    let Ok(month) = ms.parse::<u32>() else {
        return None;
    };
    if !(1900..=2200).contains(&year) || !(1..=12).contains(&month) {
        return None;
    }
    Some((end_of_month(year, month), true))
}

/// Последний день месяца.
pub fn end_of_month(year: i32, month: u32) -> NaiveDate {
    let (ny, nm) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    NaiveDate::from_ymd_opt(ny, nm, 1)
        .and_then(|d| d.pred_opt())
        .expect("end_of_month: некорректный год/месяц")
}

/// Отображение срока: для month_only — `ММ.ГГГГ`, как указал производитель.
pub fn fmt_date_display(d: NaiveDate, month_only: bool) -> String {
    if month_only {
        d.format("%m.%Y").to_string()
    } else {
        fmt_date(d)
    }
}

/// Дата в пользовательском формате `ДД.ММ.ГГГГ`.
pub fn fmt_date(d: NaiveDate) -> String {
    d.format("%d.%m.%Y").to_string()
}

/// Сколько дней осталось до конца срока (отрицательное — уже истёк).
pub fn days_left(expiry: NaiveDate, today: NaiveDate) -> i64 {
    (expiry - today).num_days()
}

/// Классификация: просрочено / истекает / годен.
pub fn status_for(days: i64, warn_days: i64) -> i32 {
    if days < 0 {
        STATUS_EXPIRED
    } else if days <= warn_days {
        STATUS_SOON
    } else {
        STATUS_OK
    }
}

/// Развёрнутый текст статуса для деталей и CSV.
pub fn status_text(status: i32, days: i64) -> String {
    match status {
        STATUS_EXPIRED => format!("просрочено {} дн. назад", -days),
        STATUS_SOON if days == 0 => "истекает сегодня".to_string(),
        STATUS_SOON => format!("истекает через {} дн.", days),
        STATUS_OK => format!("годен ещё {} дн.", days),
        STATUS_DISPOSED => "списано".to_string(),
        _ => "нет упаковок".to_string(),
    }
}

/// Короткий текст статуса для строки списка.
pub fn days_short(status: i32, days: i64) -> String {
    match status {
        STATUS_EXPIRED => format!("-{} дн.", -days),
        STATUS_SOON if days == 0 => "сегодня".to_string(),
        STATUS_SOON => format!("{} дн.", days),
        STATUS_OK => format!("{} дн.", days),
        _ => "—".to_string(),
    }
}

/// Строки «списка покупок»: активные упаковки, которые просрочены
/// или истекают в пределах порога. Одна строка на упаковку.
pub fn shopping_list(db: &Database, today: NaiveDate) -> Vec<String> {
    let mut rows = Vec::new();
    for m in &db.medicines {
        for p in &m.packages {
            if p.is_disposed() {
                continue;
            }
            let days = days_left(p.effective_expiry(), today);
            if days > db.warn_days as i64 {
                continue;
            }
            let when = if days < 0 {
                format!("ПРОСРОЧЕНО на {} дн.", -days)
            } else if days == 0 {
                "истекает сегодня".to_string()
            } else {
                format!("осталось {} дн.", days)
            };
            rows.push(format!(
                "{} ({}): {} — {}, годен до {}",
                m.name,
                if m.form.is_empty() {
                    "без формы"
                } else {
                    &m.form
                },
                if p.quantity.is_empty() {
                    "упаковка"
                } else {
                    &p.quantity
                },
                when,
                fmt_date_display(p.effective_expiry(), p.month_only && p.opened.is_none()),
            ));
        }
    }
    rows
}

/// Худший статус по всем АКТИВНЫМ (не списанным) упаковкам
/// и ближайшая дата окончания срока.
pub fn medicine_info(m: &Medicine, today: NaiveDate, warn_days: i64) -> MedInfo {
    let nearest_pkg = m
        .packages
        .iter()
        .filter(|p| !p.is_disposed())
        .min_by_key(|p| p.effective_expiry());
    match nearest_pkg {
        None => MedInfo {
            status: STATUS_EMPTY,
            nearest: None,
            nearest_days: 0,
            nearest_month_only: false,
        },
        Some(p) => {
            let eff = p.effective_expiry();
            let days = days_left(eff, today);
            MedInfo {
                status: status_for(days, warn_days),
                nearest: Some(eff),
                nearest_days: days,
                // месячный формат показа уместен, пока действует заводской срок
                nearest_month_only: p.month_only && eff == p.expiry,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn pkg(id: i32, expiry: NaiveDate, month_only: bool) -> Package {
        Package {
            id,
            expiry,
            month_only,
            quantity: String::new(),
            note: String::new(),
            photo: String::new(),
            price: None,
            purchase_date: None,
            opened: None,
            open_period_days: None,
            disposed: None,
        }
    }

    #[test]
    fn parses_full_date_formats() {
        assert_eq!(
            parse_date_input("2026-12-31"),
            Some((d("2026-12-31"), false))
        );
        assert_eq!(
            parse_date_input("31.12.2026"),
            Some((d("2026-12-31"), false))
        );
        assert_eq!(parse_date_input("1.1.2028"), Some((d("2028-01-01"), false)));
    }

    #[test]
    fn parses_month_year_variants_to_end_of_month() {
        assert_eq!(parse_date_input("01.2028"), Some((d("2028-01-31"), true)));
        assert_eq!(parse_date_input("2028-01"), Some((d("2028-01-31"), true)));
        assert_eq!(parse_date_input("01 2028"), Some((d("2028-01-31"), true)));
        assert_eq!(parse_date_input("01/2028"), Some((d("2028-01-31"), true)));
        assert_eq!(parse_date_input("1.2028"), Some((d("2028-01-31"), true)));
        assert_eq!(parse_date_input(" 12.2029 "), Some((d("2029-12-31"), true)));
    }

    #[test]
    fn rejects_garbage_and_bad_months() {
        assert_eq!(parse_date_input("31.02.2026"), None); // нет такой даты
        assert_eq!(parse_date_input("13.2028"), None); // месяц 13
        assert_eq!(parse_date_input("2028-13"), None); // месяц 13
        assert_eq!(parse_date_input("0.2028"), None); // месяц 0
        assert_eq!(parse_date_input("2028.01.15"), None); // три части
        assert_eq!(parse_date_input("abc"), None);
        assert_eq!(parse_date_input(""), None);
        assert_eq!(parse_date_input("2026/12/31"), None);
    }

    #[test]
    fn month_display_shows_mm_yyyy() {
        assert_eq!(fmt_date_display(d("2028-01-31"), true), "01.2028");
        assert_eq!(fmt_date_display(d("2028-01-31"), false), "31.01.2028");
    }

    #[test]
    fn end_of_month_handles_december_and_leap_february() {
        assert_eq!(end_of_month(2028, 2), d("2028-02-29")); // 2028 — високосный
        assert_eq!(end_of_month(2027, 2), d("2027-02-28"));
        assert_eq!(end_of_month(2028, 12), d("2028-12-31"));
        assert_eq!(end_of_month(2028, 4), d("2028-04-30"));
    }

    #[test]
    fn status_boundaries() {
        assert_eq!(status_for(-1, 60), STATUS_EXPIRED);
        assert_eq!(status_for(-365, 60), STATUS_EXPIRED);
        assert_eq!(status_for(0, 60), STATUS_SOON);
        assert_eq!(status_for(60, 60), STATUS_SOON);
        assert_eq!(status_for(61, 60), STATUS_OK);
    }

    #[test]
    fn status_texts_are_human_readable() {
        assert_eq!(status_text(STATUS_EXPIRED, -5), "просрочено 5 дн. назад");
        assert_eq!(status_text(STATUS_SOON, 0), "истекает сегодня");
        assert_eq!(status_text(STATUS_SOON, 12), "истекает через 12 дн.");
        assert_eq!(status_text(STATUS_OK, 340), "годен ещё 340 дн.");
        assert_eq!(status_text(STATUS_EMPTY, 0), "нет упаковок");
    }

    #[test]
    fn medicine_info_picks_nearest_expiry_and_worst_status() {
        let m = Medicine {
            id: 1,
            name: "Парацетамол".to_string(),
            form: "Таблетки".to_string(),
            note: String::new(),
            tags: Vec::new(),
            packages: vec![
                pkg(1, d("2030-01-01"), false),
                pkg(2, d("2026-11-01"), false),
            ],
        };
        let info = medicine_info(&m, d("2026-10-20"), 60);
        assert_eq!(info.nearest, Some(d("2026-11-01")));
        assert_eq!(info.nearest_days, 12);
        assert_eq!(info.status, STATUS_SOON);
        assert!(!info.nearest_month_only);
    }

    #[test]
    fn month_only_package_uses_end_of_month_for_status() {
        // «Годен до 01 2028» → 31.01.2028; сегодня 20.10.2026 — годен, не истекает.
        let m = Medicine {
            id: 1,
            name: "Аспирин".to_string(),
            form: "Таблетки".to_string(),
            note: String::new(),
            tags: Vec::new(),
            packages: vec![pkg(1, end_of_month(2028, 1), true)],
        };
        let info = medicine_info(&m, d("2026-10-20"), 60);
        assert_eq!(info.nearest, Some(d("2028-01-31")));
        assert_eq!(info.status, STATUS_OK);
        assert!(info.nearest_month_only);
    }

    #[test]
    fn medicine_without_packages_is_empty_status() {
        let m = Medicine {
            id: 1,
            name: "Йод".to_string(),
            form: "Прочее".to_string(),
            note: String::new(),
            tags: Vec::new(),
            packages: vec![],
        };
        let info = medicine_info(&m, d("2026-10-20"), 60);
        assert_eq!(info.status, STATUS_EMPTY);
        assert_eq!(info.nearest, None);
    }

    #[test]
    fn effective_expiry_sealed_opened_and_period() {
        let mut p = pkg(1, d("2027-06-01"), false);
        // запечатана — действует заводской срок
        assert_eq!(p.effective_expiry(), d("2027-06-01"));
        // открыли 01.10.2026, период 30 дней → годен до 31.10.2026 (раньше заводского)
        p.opened = Some(d("2026-10-01"));
        p.open_period_days = Some(30);
        assert_eq!(p.effective_expiry(), d("2026-10-31"));
        // период длиннее заводского срока — действует заводской
        p.open_period_days = Some(365);
        assert_eq!(p.effective_expiry(), d("2027-06-01"));
        // период без даты вскрытия — не действует
        p.opened = None;
        p.open_period_days = Some(30);
        assert_eq!(p.effective_expiry(), d("2027-06-01"));
    }

    #[test]
    fn medicine_info_ignores_disposed_packages() {
        // единственная упаковка списана → лекарство «без упаковок»
        let mut m = Medicine {
            id: 1,
            name: "Тест".to_string(),
            form: String::new(),
            note: String::new(),
            tags: Vec::new(),
            packages: vec![Package {
                id: 1,
                expiry: d("2026-09-01"),
                month_only: false,
                quantity: String::new(),
                note: String::new(),
                price: None,
                purchase_date: None,
                opened: None,
                open_period_days: None,
                disposed: Some(DisposedInfo {
                    date: d("2026-10-01"),
                    reason: REASON_EXPIRED,
                }),
                photo: String::new(),
            }],
        };
        let info = medicine_info(&m, d("2026-10-20"), 60);
        assert_eq!(info.status, STATUS_EMPTY);
        // добавили активную → статус берётся от неё
        m.packages.push(pkg(2, d("2027-02-01"), false));
        let info = medicine_info(&m, d("2026-10-20"), 60);
        assert_eq!(info.status, STATUS_OK);
        assert_eq!(info.nearest, Some(d("2027-02-01")));
    }

    #[test]
    fn journal_push_caps_at_400() {
        let mut db = Database::default();
        for i in 0..450 {
            db.journal_push(&format!("событие {i}"), "лек", "уп");
        }
        assert_eq!(db.events.len(), 400);
        assert_eq!(db.events[0].kind, "событие 50", "старейшие вытеснены");
        assert_eq!(db.events[399].kind, "событие 449");
    }

    #[test]
    fn shopping_list_includes_only_problem_packages() {
        let mut db = Database {
            warn_days: 60,
            ..Default::default()
        };
        let today = d("2026-10-20");
        let mk = |name: &str, expiry: NaiveDate, disposed: Option<DisposedInfo>| Medicine {
            id: 0,
            name: name.to_string(),
            form: "Таблетки".to_string(),
            note: String::new(),
            tags: Vec::new(),
            packages: vec![Package {
                id: 0,
                expiry,
                month_only: false,
                quantity: "1 уп.".to_string(),
                note: String::new(),
                photo: String::new(),
                price: None,
                purchase_date: None,
                opened: None,
                open_period_days: None,
                disposed,
            }],
        };
        db.medicines.push(mk("Просрочка", d("2026-09-01"), None));
        db.medicines.push(mk("Скоро", d("2026-11-25"), None));
        db.medicines.push(mk("Запас", d("2027-06-01"), None));
        db.medicines.push(mk(
            "Списано",
            d("2026-09-01"),
            Some(DisposedInfo {
                date: today,
                reason: REASON_EXPIRED,
            }),
        ));
        let rows = shopping_list(&db, today);
        assert_eq!(rows.len(), 2, "только просрочка и истекающие");
        assert!(rows[0].starts_with("Просрочка"));
        assert!(rows[0].contains("ПРОСРОЧЕНО"));
        assert!(rows[1].starts_with("Скоро"));
    }

    #[test]
    fn database_ids_increment() {
        let mut db = Database::default();
        assert_eq!(db.new_med_id(), 1);
        assert_eq!(db.new_med_id(), 2);
        assert_eq!(db.new_pkg_id(), 1);
        assert_eq!(db.next_med_id, 3);
    }

    #[test]
    fn parse_tags_splits_and_dedups() {
        assert_eq!(
            parse_tags(" жаропонижающее ,  детское;"),
            vec!["жаропонижающее", "детское"]
        );
        assert_eq!(
            parse_tags("а, б; А ,в"),
            vec!["а", "б", "в"],
            "дубли без регистра"
        );
        assert_eq!(parse_tags(" , ;"), Vec::<String>::new());
        assert_eq!(parse_tags("одно"), vec!["одно"]);
    }
}
