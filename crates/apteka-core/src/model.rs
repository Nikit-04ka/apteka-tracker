//! Доменная модель: лекарства, упаковки, статусы сроков годности.

use chrono::{Datelike, NaiveDate};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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

/// Ёмкость журнала событий: старые записи вытесняются. Окно журнала
/// предупреждает об усечении, опираясь на эту константу.
pub const EVENTS_CAP: usize = 400;

/// Причина списания упаковки. `DISCARDED` — «Выбросить»: кнопка доступна и для
/// просроченной, и для ещё годной упаковки, поэтому текст не утверждает
/// просрочку (значение 1 — прежняя константа «reason expired», JSON хранит число).
pub const REASON_USED: i32 = 0;
pub const REASON_DISCARDED: i32 = 1;

/// Факт списания упаковки (израсходована или выброшена).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DisposedInfo {
    pub date: NaiveDate,
    pub reason: i32,
}

pub fn dispose_reason_text(reason: i32) -> &'static str {
    match reason {
        REASON_USED => "израсходовано",
        REASON_DISCARDED => "выброшено",
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
    ///
    /// Не паникует при переполнении даты: мусорный `open_period_days` из
    /// рукописного JSON отбрасывается, остаётся заводской срок.
    pub fn effective_expiry(&self) -> NaiveDate {
        match (self.opened, self.open_period_days) {
            (Some(opened), Some(days)) => {
                let after_open = opened
                    .checked_add_signed(chrono::Duration::days(days as i64))
                    .unwrap_or(self.expiry);
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
/// В `Database` хранится в `Arc`: снимки истории делят неизменённые лекарства
/// и не копируют их впустую (copy-on-write на уровне лекарства).
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
    /// Порог «истекает скоро» в днях. `#[serde(default)]` — частично
    /// отредактированный вручную JSON без поля не считается повреждённым.
    #[serde(default = "default_warn_days")]
    pub warn_days: i32,
    #[serde(default)]
    pub next_med_id: i32,
    #[serde(default)]
    pub next_pkg_id: i32,
    #[serde(default)]
    pub medicines: Vec<Arc<Medicine>>,
    /// Журнал событий (хранятся последние 400).
    #[serde(default)]
    pub events: Vec<JournalEvent>,
}

/// Текущая версия схемы файла данных. Меняя структуру `Database`,
/// увеличить и добавить миграцию в `Database::migrate`.
pub const DATA_VERSION: u32 = 1;

/// Значение порога по умолчанию для `#[serde(default)]`.
fn default_warn_days() -> i32 {
    60
}

impl Database {
    /// Миграции схемы: приводит базу любой прочитанной версии к текущей.
    /// Вызывается после каждой загрузки. Версия новее текущей не прижимается:
    /// файл из более новой программы не должен молча записаться как текущая.
    ///
    /// Помимо версии нормализует значения, которые рукописный или старый JSON
    /// мог испортить так, что расчёты паникуют или теряют ссылки: счётчики id,
    /// период после вскрытия и имя файла фото.
    pub fn migrate(&mut self) {
        // version == 0 — файлы до появления явной версии, считаем первой
        if self.version < DATA_VERSION {
            self.version = DATA_VERSION;
        }
        // Порог не должен выводить статусы из разумного диапазона (5..=365 —
        // как в SpinBox интерфейса).
        self.warn_days = self.warn_days.clamp(5, 365);
        // Период после вскрытия вне 0..=3650 — мусор: переполнил бы дату.
        for m in &mut self.medicines {
            let m = Arc::make_mut(m);
            for p in &mut m.packages {
                if let Some(days) = p.open_period_days {
                    if !(0..=3650).contains(&days) {
                        p.open_period_days = None;
                    }
                }
                // Инвариант month_only: в expiry хранится последний день месяца.
                // Файл правили руками или данные из старой версии — нормализуем,
                // иначе CSV-roundtrip молча сдвинул бы дату.
                if p.month_only {
                    p.expiry = end_of_month(p.expiry.year(), p.expiry.month());
                }
                // Имя файла фото — только имя: путь с разделителями или «..»
                // позволил бы читать файлы вне каталога photos/.
                if !p.photo.is_empty() {
                    p.photo = std::path::Path::new(&p.photo)
                        .file_name()
                        .map(|s| s.to_string_lossy().into_owned())
                        .unwrap_or_default();
                }
            }
        }
        // Счётчики id — всегда впереди максимального существующего: иначе новая
        // запись получит занятый id и данные переплетутся.
        let max_med = self.medicines.iter().map(|m| m.id).max().unwrap_or(0);
        if self.next_med_id <= max_med {
            self.next_med_id = max_med.saturating_add(1);
        }
        let max_pkg = self
            .medicines
            .iter()
            .flat_map(|m| m.packages.iter())
            .map(|p| p.id)
            .max()
            .unwrap_or(0);
        if self.next_pkg_id <= max_pkg {
            self.next_pkg_id = max_pkg.saturating_add(1);
        }
        // Дубликаты id (файл правили руками) — перенумеровываем: иначе
        // medicine_mut(id) вернёт первую запись, и правки перепутаются.
        let mut seen_med = std::collections::HashSet::new();
        for m in &mut self.medicines {
            let m = Arc::make_mut(m);
            if m.id < 1 || !seen_med.insert(m.id) {
                m.id = self.next_med_id;
                self.next_med_id = self.next_med_id.saturating_add(1);
                seen_med.insert(m.id);
            }
        }
        let mut seen_pkg = std::collections::HashSet::new();
        for m in &mut self.medicines {
            let m = Arc::make_mut(m);
            for p in &mut m.packages {
                if p.id < 1 || !seen_pkg.insert(p.id) {
                    p.id = self.next_pkg_id;
                    self.next_pkg_id = self.next_pkg_id.saturating_add(1);
                    seen_pkg.insert(p.id);
                }
            }
        }
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
        self.medicines.iter().find(|m| m.id == id).map(Arc::as_ref)
    }

    /// Мутация лекарства через `Arc::make_mut`: при общем снимке истории
    /// копируется только правимое лекарство, а не вся база.
    pub fn medicine_mut(&mut self, id: i32) -> Option<&mut Medicine> {
        self.medicines
            .iter_mut()
            .find(|m| m.id == id)
            .map(Arc::make_mut)
    }

    /// Добавляет событие в журнал (хранятся последние `EVENTS_CAP`).
    pub fn journal_push(&mut self, kind: &str, medicine: &str, package: &str) {
        self.events.push(JournalEvent {
            at: chrono::Local::now().naive_local(),
            kind: kind.to_string(),
            medicine: medicine.to_string(),
            package: package.to_string(),
        });
        if self.events.len() > EVENTS_CAP {
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
            // год вне разумного диапазона не принимаем — как и для «ММ.ГГГГ»:
            // «28.02.30» не должно прочитываться как 30 год
            if (1900..=2200).contains(&d.year()) {
                return Some((d, false));
            }
            return None;
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

/// Годовая сводка: количество упаковок и денежная сумма. `unpriced` — сколько
/// упаковок учтено в `count`, но не в `sum` (цена не указана): иначе строка
/// «2 уп. · 0.00 руб.» выглядит как потеря данных.
#[derive(Debug, Clone, PartialEq)]
pub struct YearSum {
    pub year: i32,
    pub count: usize,
    pub sum: f64,
    pub unpriced: usize,
}

fn by_year(items: impl Iterator<Item = (i32, Option<f64>)>) -> Vec<YearSum> {
    let mut acc: std::collections::HashMap<i32, (usize, f64, usize)> =
        std::collections::HashMap::new();
    for (year, price) in items {
        let e = acc.entry(year).or_insert((0, 0.0, 0));
        e.0 += 1;
        match price {
            Some(p) => e.1 += p,
            None => e.2 += 1,
        }
    }
    let mut out: Vec<YearSum> = acc
        .into_iter()
        .map(|(year, (count, sum, unpriced))| YearSum {
            year,
            count,
            sum,
            unpriced,
        })
        .collect();
    out.sort_by_key(|r| std::cmp::Reverse(r.year));
    out
}

/// Покупки по годам (по `purchase_date`), годы по убыванию.
/// Упаковки без даты покупки не учитываются.
pub fn purchases_by_year(db: &Database) -> Vec<YearSum> {
    by_year(
        db.medicines
            .iter()
            .flat_map(|m| m.packages.iter())
            .filter_map(|p| p.purchase_date.map(|d| (d.year(), p.price))),
    )
}

/// Списания по годам (по дате списания), годы по убыванию.
pub fn disposals_by_year(db: &Database) -> Vec<YearSum> {
    by_year(
        db.medicines
            .iter()
            .flat_map(|m| m.packages.iter())
            .filter_map(|p| p.disposed.as_ref().map(|d| (d.date.year(), p.price))),
    )
}

/// Самая ранняя активная (не списанная) упаковка — например, для отчётов,
/// где важен и формат показа срока (`month_only`).
pub fn oldest_active_package(db: &Database) -> Option<&Package> {
    db.medicines
        .iter()
        .flat_map(|m| m.packages.iter())
        .filter(|p| !p.is_disposed())
        .min_by_key(|p| p.effective_expiry())
}

/// Самая ранняя дата годности среди активных (не списанных) упаковок.
pub fn oldest_active_expiry(db: &Database) -> Option<NaiveDate> {
    oldest_active_package(db).map(|p| p.effective_expiry())
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
            let eff = p.effective_expiry();
            let days = days_left(eff, today);
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
                fmt_date_display(eff, p.month_only && eff == p.expiry),
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
    fn display_roundtrip_preserves_month_only() {
        // предзаполнение формы редактирования: parse от отображённой строки
        // обязан вернуть ту же дату и тот же флаг
        let dd = d("2028-01-31");
        assert_eq!(
            parse_date_input(&fmt_date_display(dd, true)),
            Some((dd, true))
        );
        assert_eq!(
            parse_date_input(&fmt_date_display(dd, false)),
            Some((dd, false))
        );
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
    fn dispose_reason_text_does_not_claim_expiry() {
        // «Выбросить» доступен и для годной упаковки — текст причины
        // не должен утверждать просрочку
        assert_eq!(dispose_reason_text(REASON_USED), "израсходовано");
        assert_eq!(dispose_reason_text(REASON_DISCARDED), "выброшено");
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
                    reason: REASON_DISCARDED,
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
        let mk = |name: &str, expiry: NaiveDate, disposed: Option<DisposedInfo>| {
            Arc::new(Medicine {
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
            })
        };
        db.medicines.push(mk("Просрочка", d("2026-09-01"), None));
        db.medicines.push(mk("Скоро", d("2026-11-25"), None));
        db.medicines.push(mk("Запас", d("2027-06-01"), None));
        db.medicines.push(mk(
            "Списано",
            d("2026-09-01"),
            Some(DisposedInfo {
                date: today,
                reason: REASON_DISCARDED,
            }),
        ));
        let rows = shopping_list(&db, today);
        assert_eq!(rows.len(), 2, "только просрочка и истекающие");
        assert!(rows[0].starts_with("Просрочка"));
        assert!(rows[0].contains("ПРОСРОЧЕНО"));
        assert!(rows[1].starts_with("Скоро"));
    }

    #[test]
    fn shopping_list_month_display_matches_medicine_info() {
        // «ММ.ГГГГ» — пока действует заводской месячный срок; после вскрытия
        // с периодом эффективная дата не конец месяца — полная дата
        let mut db = Database {
            warn_days: 60,
            ..Default::default()
        };
        let today = d("2026-10-20");
        // обе упаковки истекают в пределах порога (30..41 дн.), иначе список
        // покупок их не показывает
        let mut sealed = pkg(1, end_of_month(2026, 11), true);
        sealed.quantity = "1 уп.".to_string();
        let mut opened = pkg(2, end_of_month(2026, 11), true);
        opened.quantity = "1 фл.".to_string();
        opened.opened = Some(today);
        opened.open_period_days = Some(30);
        db.medicines.push(Arc::new(Medicine {
            id: 1,
            name: "Тест".to_string(),
            form: String::new(),
            note: String::new(),
            tags: Vec::new(),
            packages: vec![sealed, opened],
        }));
        let rows = shopping_list(&db, today);
        assert_eq!(rows.len(), 2);
        assert!(rows[0].contains("11.2026"), "запечатанная: месяц без дня");
        assert!(
            rows[1].contains("19.11.2026"),
            "вскрытая: полная дата эффективного срока"
        );
    }

    #[test]
    fn database_ids_increment() {
        let mut db = Database::default();
        assert_eq!(db.new_med_id(), 1);
        assert_eq!(db.new_med_id(), 2);
        assert_eq!(db.new_pkg_id(), 1);
        assert_eq!(db.next_med_id, 3);
    }

    fn priced_pkg(
        expiry: NaiveDate,
        price: Option<f64>,
        purchase: Option<NaiveDate>,
        disposed: Option<DisposedInfo>,
    ) -> Package {
        Package {
            id: 0,
            expiry,
            month_only: false,
            quantity: String::new(),
            note: String::new(),
            price,
            purchase_date: purchase,
            opened: None,
            open_period_days: None,
            disposed,
            photo: String::new(),
        }
    }

    #[test]
    fn year_reports_group_and_sort_desc() {
        let mut db = Database::default();
        let dd = |y: i32, m: u32, d: u32| NaiveDate::from_ymd_opt(y, m, d).unwrap();
        let dis2026 = DisposedInfo {
            date: dd(2026, 3, 1),
            reason: 1,
        };
        let mk = |purchase: Option<NaiveDate>, disposed: Option<DisposedInfo>| {
            Arc::new(Medicine {
                id: 0,
                name: String::new(),
                form: String::new(),
                note: String::new(),
                tags: Vec::new(),
                packages: vec![priced_pkg(dd(2030, 1, 1), Some(250.0), purchase, disposed)],
            })
        };
        db.medicines.push(mk(Some(dd(2025, 5, 1)), None));
        db.medicines
            .push(mk(Some(dd(2025, 9, 1)), Some(dis2026.clone())));
        db.medicines.push(mk(Some(dd(2026, 2, 1)), None));
        db.medicines.push(mk(None, Some(dis2026)));

        let purchases = purchases_by_year(&db);
        assert_eq!(purchases.len(), 2, "упаковка без даты покупки не считается");
        assert_eq!(purchases[0].year, 2026);
        assert_eq!(purchases[0].count, 1);
        assert_eq!(purchases[0].sum, 250.0);
        assert_eq!(purchases[1].year, 2025);
        assert_eq!(purchases[1].count, 2);
        assert_eq!(purchases[1].sum, 500.0);

        let disposals = disposals_by_year(&db);
        assert_eq!(disposals.len(), 1);
        assert_eq!(disposals[0].year, 2026);
        assert_eq!(
            disposals[0].count, 2,
            "списаны обе, включая купленную в 2025"
        );
        assert_eq!(disposals[0].sum, 500.0);

        assert_eq!(oldest_active_expiry(&db), Some(dd(2030, 1, 1)));
    }

    #[test]
    fn year_reports_count_unpriced_packages_separately() {
        // упаковка с датой покупки, но без цены: в количестве есть, в сумме —
        // нет, и сводка это честно показывает («без цены»), а не «0.00 руб.»
        let mut db = Database::default();
        let dd = |y: i32, m: u32, d: u32| NaiveDate::from_ymd_opt(y, m, d).unwrap();
        db.medicines.push(Arc::new(Medicine {
            id: 0,
            name: String::new(),
            form: String::new(),
            note: String::new(),
            tags: Vec::new(),
            packages: vec![priced_pkg(dd(2030, 1, 1), None, Some(dd(2025, 5, 1)), None)],
        }));
        let purchases = purchases_by_year(&db);
        assert_eq!(purchases.len(), 1);
        assert_eq!(purchases[0].count, 1);
        assert_eq!(purchases[0].sum, 0.0);
        assert_eq!(purchases[0].unpriced, 1);
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

    #[test]
    fn full_dates_reject_years_out_of_range() {
        // год вне 1900..=2200 не принимается и для полных дат — как и для «ММ.ГГГГ»
        assert_eq!(parse_date_input("28.02.30"), None);
        assert_eq!(parse_date_input("01.01.2500"), None);
        assert_eq!(parse_date_input("01.01.1899"), None);
        assert_eq!(
            parse_date_input("31.12.2026"),
            Some((d("2026-12-31"), false)),
            "обычные даты работают как раньше"
        );
    }

    #[test]
    fn migrate_keeps_future_version_and_normalizes_legacy() {
        let mut legacy = Database {
            version: 0,
            ..Default::default()
        };
        legacy.migrate();
        assert_eq!(legacy.version, DATA_VERSION, "файл без версии — текущая");

        let mut future = Database {
            version: DATA_VERSION + 7,
            ..Default::default()
        };
        future.migrate();
        assert_eq!(
            future.version,
            DATA_VERSION + 7,
            "версия новее текущей не прижимается"
        );
    }

    #[test]
    fn oldest_active_package_skips_disposed() {
        let mut db = Database::default();
        let mut gone = pkg(1, d("2026-01-01"), false);
        gone.disposed = Some(DisposedInfo {
            date: d("2026-02-01"),
            reason: REASON_USED,
        });
        let monthly = pkg(2, end_of_month(2028, 1), true);
        db.medicines.push(Arc::new(Medicine {
            id: 1,
            name: "Тест".to_string(),
            form: String::new(),
            note: String::new(),
            tags: Vec::new(),
            packages: vec![gone, monthly],
        }));
        let oldest = oldest_active_package(&db).unwrap();
        assert_eq!(oldest.id, 2, "списанная упаковка не учитывается");
        assert!(oldest.month_only, "формат показа срока доступен отчёту");
        assert_eq!(oldest_active_expiry(&db), Some(end_of_month(2028, 1)));
    }

    #[test]
    fn effective_expiry_never_panics_on_absurd_period() {
        // open_period_days = i32::MAX из рукописного JSON не должен паниковать:
        // остаётся заводской срок
        let mut p = pkg(1, d("2027-06-01"), false);
        p.opened = Some(d("2026-10-01"));
        p.open_period_days = Some(i32::MAX);
        assert_eq!(p.effective_expiry(), d("2027-06-01"));
    }

    #[test]
    fn migrate_normalizes_counters_period_and_photo() {
        let mut db = Database {
            version: 0,
            warn_days: 5000,
            next_med_id: 1,
            next_pkg_id: 1,
            ..Default::default()
        };
        db.medicines.push(Arc::new(Medicine {
            id: 7,
            name: "Тест".to_string(),
            form: String::new(),
            note: String::new(),
            tags: Vec::new(),
            packages: vec![
                Package {
                    id: 42,
                    expiry: d("2027-06-01"),
                    month_only: false,
                    quantity: String::new(),
                    note: String::new(),
                    photo: "../../evil.png".to_string(),
                    price: None,
                    purchase_date: None,
                    opened: None,
                    open_period_days: Some(i32::MAX),
                    disposed: None,
                },
                Package {
                    id: 43,
                    expiry: d("2027-06-01"),
                    month_only: false,
                    quantity: String::new(),
                    note: String::new(),
                    photo: String::new(),
                    price: None,
                    purchase_date: None,
                    opened: None,
                    open_period_days: Some(30),
                    disposed: None,
                },
            ],
        }));
        db.migrate();
        assert_eq!(db.warn_days, 365, "порог прижат к диапазону SpinBox");
        assert_eq!(db.next_med_id, 8, "счётчик впереди максимального id");
        assert_eq!(db.next_pkg_id, 44, "счётчик упаковок впереди максимального");
        let ps = &db.medicines[0].packages;
        assert_eq!(ps[0].open_period_days, None, "мусорный период отброшен");
        assert_eq!(ps[0].photo, "evil.png", "путь фото сведён к имени файла");
        assert_eq!(ps[1].open_period_days, Some(30), "корректный период цел");
    }

    #[test]
    fn migrate_normalizes_month_only_expiry_to_end_of_month() {
        // Инвариант month_only: в expiry — последний день месяца. Рукописный
        // файл с «15.01.2028, month_only» нормализуется при загрузке, а не
        // сдвигается молча при CSV-roundtrip.
        let mut db = Database::default();
        db.medicines.push(Arc::new(Medicine {
            id: 1,
            name: "Тест".to_string(),
            form: String::new(),
            note: String::new(),
            tags: Vec::new(),
            packages: vec![pkg(1, d("2028-01-15"), true), pkg(2, d("2027-02-10"), true)],
        }));
        db.migrate();
        let ps = &db.medicines[0].packages;
        assert_eq!(ps[0].expiry, d("2028-01-31"), "январь — до конца месяца");
        assert_eq!(ps[1].expiry, d("2027-02-28"), "февраль — до конца месяца");
    }

    #[test]
    fn migrate_renumbers_duplicate_ids() {
        // Дубликаты id (рукописный файл) получают уникальные: правка по id
        // не перепутывает записи
        let mut db = Database {
            next_med_id: 1,
            next_pkg_id: 1,
            ..Default::default()
        };
        for name in ["Первое", "Второе"] {
            db.medicines.push(Arc::new(Medicine {
                id: 7,
                name: name.to_string(),
                form: String::new(),
                note: String::new(),
                tags: Vec::new(),
                packages: vec![pkg(5, d("2030-01-01"), false)],
            }));
        }
        db.migrate();
        let ids: std::collections::HashSet<i32> = db.medicines.iter().map(|m| m.id).collect();
        assert_eq!(ids.len(), 2, "id лекарств уникальны");
        assert_eq!(
            db.medicine(7).unwrap().name,
            "Первое",
            "первая запись держит свой id"
        );
        let pkg_ids: std::collections::HashSet<i32> = db
            .medicines
            .iter()
            .flat_map(|m| m.packages.iter())
            .map(|p| p.id)
            .collect();
        assert_eq!(pkg_ids.len(), 2, "id упаковок уникальны");
    }
}
