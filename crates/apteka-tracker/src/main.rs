// Прячет консольное окно в релизной сборке (GUI-приложение).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Точка входа: загрузка данных, сборка окна Slint, связка колбэков.

use apteka_core::history::History;
use apteka_core::model::*;
use apteka_core::storage;

use apteka_core::chrono::{Datelike, Local, Months, NaiveDate};
use slint::{Color, ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

slint::include_modules!();

/// Сегодняшняя дата (без времени).
fn today() -> NaiveDate {
    Local::now().date_naive()
}

// ---------- Один экземпляр программы ----------

/// Состояние замка `apteka.lock` рядом с exe.
enum InstanceGuard {
    /// Замок захвачен — работаем. Файл держится открытым всё время работы:
    /// замок освобождается автоматически даже при сбое процесса.
    Locked(std::fs::File),
    /// Замок создать не удалось (носитель только для чтения) — работаем без него.
    NoLockAvailable,
    /// Уже запущено другим процессом.
    AlreadyRunning,
}

fn acquire_single_instance() -> InstanceGuard {
    let path = storage::exe_dir().join("apteka.lock");
    let file = match std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
    {
        Ok(f) => f,
        Err(_) => return InstanceGuard::NoLockAvailable,
    };
    match file.try_lock() {
        Ok(()) => InstanceGuard::Locked(file),
        Err(_) => InstanceGuard::AlreadyRunning,
    }
}

/// Модальное сообщение для пользователя (в release консоли нет).
fn message_box(text: &str) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            MessageBoxW, MB_ICONWARNING, MB_OK,
        };
        unsafe {
            let text: Vec<u16> = format!("{text}\0").encode_utf16().collect();
            let title: Vec<u16> = "Домашняя аптечка\0".encode_utf16().collect();
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                title.as_ptr(),
                MB_OK | MB_ICONWARNING,
            );
        }
    }
    #[cfg(not(windows))]
    {
        eprintln!("{text}");
    }
}

#[cfg(windows)]
fn notify_already_running() {
    message_box("Домашняя аптечка уже запущена.");
}

#[cfg(not(windows))]
fn notify_already_running() {
    eprintln!("Домашняя аптечка уже запущена.");
}

/// Изменяемое состояние приложения. UI-свойства — только отображение.
struct State {
    db: Database,
    history: History,
    selected_id: i32,
    search: String,
    filter_idx: usize,
    sort_idx: usize,
    /// id редактируемой упаковки, -1 — режим добавления
    pkg_edit_id: i32,
    /// вид последней склеенной правки для истории (None — после структурных мутаций)
    last_commit_kind: Option<EditKind>,
    /// фото, выбранное для ещё не сохранённой упаковки
    pending_photo: Option<std::path::PathBuf>,
    /// есть несохранённые правки текста (отложенное автосохранение)
    unsaved: Cell<bool>,
    /// кэш декодированных фото (имя файла → готовое изображение)
    photo_cache: RefCell<HashMap<String, slint::Image>>,
    /// календарь: отображаемый месяц
    cal_year: i32,
    cal_month: u32,
}

const MONTHS_RU: [&str; 12] = [
    "Январь", "Февраль", "Март", "Апрель", "Май", "Июнь", "Июль", "Август", "Сентябрь",
    "Октябрь", "Ноябрь", "Декабрь",
];

impl State {
    fn selected(&self) -> Option<&Medicine> {
        let id = self.selected_id;
        self.db.medicine(id)
    }

    fn selected_mut(&mut self) -> Option<&mut Medicine> {
        let id = self.selected_id;
        self.db.medicine_mut(id)
    }
}

// ---------- Цвета валидации ----------

fn color_grey() -> Color {
    Color::from_rgb_u8(0x9c, 0xa3, 0xaf)
}
fn color_red() -> Color {
    Color::from_rgb_u8(0xdc, 0x26, 0x26)
}
fn color_amber() -> Color {
    Color::from_rgb_u8(0xd9, 0x77, 0x06)
}
fn color_green() -> Color {
    Color::from_rgb_u8(0x16, 0xa3, 0x4a)
}

// ---------- Построение строк UI ----------

fn build_med_row(m: &Medicine, info: &MedInfo) -> MedicineRow {
    let active = m.packages.iter().filter(|p| !p.is_disposed()).count();
    let subtitle = match info.nearest {
        None => {
            if m.packages.is_empty() {
                "нет упаковок — добавьте первую".to_string()
            } else {
                format!("активных упаковок нет (списано: {})", m.packages.len())
            }
        }
        Some(d) => format!(
            "ближайший срок: {} · упаковок: {} · {}",
            fmt_date_display(d, info.nearest_month_only),
            active,
            status_text(info.status, info.nearest_days)
        ),
    };
    MedicineRow {
        id: m.id,
        name: m.name.clone().into(),
        subtitle: subtitle.into(),
        days_text: days_short(info.status, info.nearest_days).into(),
        status: info.status,
    }
}

/// Пересобирает список лекарств с учётом поиска, фильтра и сортировки.
fn rebuild_list(ui: &AppWindow, st: &State) {
    let t = today();
    let warn = st.db.warn_days as i64;
    let needle = st.search.to_lowercase();

    let mut rows: Vec<(i64, String, MedicineRow)> = Vec::new();
    for m in &st.db.medicines {
        if !needle.is_empty() {
            let hay = format!("{} {} {}", m.name, m.form, m.note).to_lowercase();
            if !hay.contains(&needle) {
                continue;
            }
        }
        let info = medicine_info(m, t, warn);
        if st.filter_idx == 1 && info.status != STATUS_SOON {
            continue;
        }
        if st.filter_idx == 2 && info.status != STATUS_EXPIRED {
            continue;
        }
        let sort_key = if info.nearest.is_some() { info.nearest_days } else { i64::MAX };
        rows.push((sort_key, m.name.to_lowercase(), build_med_row(m, &info)));
    }
    match st.sort_idx {
        0 => rows.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1))),
        _ => rows.sort_by(|a, b| a.1.cmp(&b.1)),
    }

    let data: Vec<MedicineRow> = rows.into_iter().map(|(_, _, row)| row).collect();
    ui.set_medicines(ModelRc::from(Rc::new(VecModel::from(data))));

    ui.set_list_empty_text(SharedString::from(
        if st.search.is_empty() && st.filter_idx == 0 {
            "Аптечка пуста.\nНажмите «+ Добавить лекарство»."
        } else {
            "Ничего не найдено.\nИзмените поиск или фильтр."
        },
    ));
}

/// Обновляет сводку по всем лекарствам (без учёта фильтров).
fn update_summary(ui: &AppWindow, st: &State) {
    let t = today();
    let warn = st.db.warn_days as i64;
    let mut soon = 0usize;
    let mut expired = 0usize;
    let mut packages = 0usize;
    for m in &st.db.medicines {
        packages += m.packages.len();
        match medicine_info(m, t, warn).status {
            STATUS_SOON => soon += 1,
            STATUS_EXPIRED => expired += 1,
            _ => {}
        }
    }
    ui.set_summary_total(SharedString::from(format!(
        "Лекарств: {} · Упаковок: {}",
        st.db.medicines.len(),
        packages
    )));
    ui.set_summary_soon(SharedString::from(format!(
        "Истекают (≤{} дн.): {}",
        st.db.warn_days, soon
    )));
    ui.set_summary_expired(SharedString::from(format!("Просрочено: {}", expired)));

    let cost: f64 = st
        .db
        .medicines
        .iter()
        .flat_map(|m| m.packages.iter())
        .filter(|p| !p.is_disposed())
        .filter_map(|p| p.price)
        .sum();
    ui.set_summary_cost(SharedString::from(format!("Стоимость: {cost:.0} руб.")));
}

/// Пересобирает таблицу упаковок выбранного лекарства (списанные — по флагу).
fn rebuild_packages(ui: &AppWindow, st: &State) {
    let Some(m) = st.selected() else {
        ui.set_packages(ModelRc::default());
        ui.set_packages_header(SharedString::from("Упаковки"));
        return;
    };
    let t = today();
    let warn = st.db.warn_days as i64;
    let show_disposed = ui.get_show_disposed();

    let mut pkgs: Vec<&Package> = m
        .packages
        .iter()
        .filter(|p| show_disposed || !p.is_disposed())
        .collect();
    pkgs.sort_by_key(|p| p.effective_expiry());

    let rows: Vec<PackageRow> = pkgs
        .iter()
        .map(|p| {
            let eff = p.effective_expiry();
            let (days, status, days_text) = if let Some(_d) = &p.disposed {
                (0, STATUS_DISPOSED, "списано".to_string())
            } else {
                let days = days_left(eff, t);
                let s = status_for(days, warn);
                (days, s, days_short(s, days))
            };
            let opened_info = if let Some(d) = &p.disposed {
                format!(
                    "списано {} ({})",
                    fmt_date(d.date),
                    dispose_reason_text(d.reason)
                )
            } else if let Some(o) = p.opened {
                if p.open_period_days.is_some() {
                    format!("открыта {} · годен до {}", fmt_date(o), fmt_date(eff))
                } else {
                    format!("открыта {} · без периода после вскрытия", fmt_date(o))
                }
            } else {
                String::new()
            };
            let photo = if p.photo.is_empty() {
                slint::Image::default()
            } else {
                // кэш: фото с диска читается и декодируется один раз
                let mut cache = st.photo_cache.borrow_mut();
                if let Some(img) = cache.get(&p.photo) {
                    img.clone()
                } else {
                    let img = slint::Image::load_from_path(
                        &storage::exe_dir().join("photos").join(&p.photo),
                    )
                    .unwrap_or_default();
                    cache.insert(p.photo.clone(), img.clone());
                    img
                }
            };
            PackageRow {
                id: p.id,
                expiry: fmt_date_display(p.expiry, p.month_only).into(),
                quantity: p.quantity.clone().into(),
                price: p
                    .price
                    .map(|v| format!("{v:.2} руб."))
                    .unwrap_or_default()
                    .into(),
                note: p.note.clone().into(),
                opened_info: opened_info.into(),
                status,
                days_text: days_text.into(),
                status_text: status_text(status, days).into(),
                opened: p.opened.is_some(),
                disposed: p.is_disposed(),
                photo,
            }
        })
        .collect();

    let disposed_count = m.packages.iter().filter(|p| p.is_disposed()).count();
    let header = if disposed_count > 0 {
        format!("Упаковки ({}, списано: {})", rows.len(), disposed_count)
    } else {
        format!("Упаковки ({})", rows.len())
    };
    ui.set_packages_header(SharedString::from(header));

    // Пустой список: разный текст в зависимости от причины
    let total = m.packages.len();
    let empty_text = if total == 0 {
        "Упаковок пока нет. Заполните форму ниже и нажмите «Добавить упаковку».".to_string()
    } else {
        format!(
            "Все упаковки списаны ({}). Нажмите «Показать списанные», чтобы увидеть их, или добавьте новую — форма ниже.",
            total
        )
    };
    ui.set_packages_empty_text(SharedString::from(empty_text));
    ui.set_packages(ModelRc::from(Rc::new(VecModel::from(rows))));
}

/// Заполняет панель деталей; при `reset_form` сбрасывает форму упаковки.
fn update_details(ui: &AppWindow, st: &mut State, reset_form: bool) {
    update_details_readonly(ui, st);
    if reset_form {
        reset_pkg_form(ui, st);
    }
}

/// Заполнение деталей без побочных эффектов — пригодно для таймера.
fn update_details_readonly(ui: &AppWindow, st: &State) {
    match st.selected() {
        None => {
            ui.set_has_selection(false);
        }
        Some(m) => {
            ui.set_has_selection(true);
            ui.set_detail_name(SharedString::from(m.name.clone()));
            ui.set_detail_note(SharedString::from(m.note.clone()));
            let idx = FORMS
                .iter()
                .position(|f| *f == m.form)
                .unwrap_or(FORMS.len() - 1);
            ui.set_detail_form_index(idx as i32);
            rebuild_packages(ui, st);
        }
    }
}

/// Пересчёт подсветки и сводки по текущей дате (вызывается таймером).
fn refresh_statuses(ui: &AppWindow, st: &State) {
    rebuild_list(ui, st);
    update_summary(ui, st);
    update_details_readonly(ui, st);
}

/// Сдвигает выделение по видимому (отфильтрованному) списку — клавиши ↑/↓.
fn move_selection(ui: &AppWindow, st: &mut State, delta: i32) {
    let model = ui.get_medicines();
    let ids: Vec<i32> = (0..model.row_count())
        .filter_map(|i| model.row_data(i).map(|r| r.id))
        .collect();
    if ids.is_empty() {
        return;
    }
    let new_idx = match ids.iter().position(|&id| id == st.selected_id) {
        Some(i) => ((i as i64) + delta as i64).clamp(0, ids.len() as i64 - 1) as usize,
        None if delta < 0 => ids.len() - 1,
        None => 0,
    };
    st.selected_id = ids[new_idx];
    ui.set_selected_id(st.selected_id);
    reset_delete_armed(ui);
    update_details(ui, st, true);
}

/// Краткое описание упаковки для журнала.
fn pkg_desc(p: &Package) -> String {
    format!("срок {}", fmt_date_display(p.expiry, p.month_only))
}

/// Календарная сетка: 6 недель × 7 дней от отображаемого месяца.
fn cal_rebuild(ui: &AppWindow, st: &State) {
    let Some(first) = NaiveDate::from_ymd_opt(st.cal_year, st.cal_month, 1) else { return };
    let week_start =
        first - apteka_core::chrono::Duration::days(first.weekday().num_days_from_monday() as i64);
    let t = today();
    let mut rows: Vec<CalRow> = Vec::new();
    for w in 0..6u32 {
        let cells: Vec<CalCell> = (0..7u32)
            .map(|d| {
                let day = week_start + apteka_core::chrono::Duration::days((w * 7 + d) as i64);
                CalCell {
                    label: format!("{}", day.day()).into(),
                    ymd: day.format("%Y-%m-%d").to_string().into(),
                    in_month: day.month() == st.cal_month && day.year() == st.cal_year,
                    today: day == t,
                }
            })
            .collect();
        rows.push(CalRow {
            cells: ModelRc::from(Rc::new(VecModel::from(cells))),
        });
    }
    ui.set_cal_weeks(ModelRc::from(Rc::new(VecModel::from(rows))));
    ui.set_cal_month_label(SharedString::from(format!(
        "{} {}",
        MONTHS_RU[(st.cal_month - 1) as usize],
        st.cal_year
    )));
}

fn refresh_all(ui: &AppWindow, st: &mut State, reset_form: bool) {
    rebuild_list(ui, st);
    update_summary(ui, st);
    update_details(ui, st, reset_form);
}

/// Точечное обновление строки лекарства в видимом списке (без сброса прокрутки).
fn update_row(ui: &AppWindow, id: i32, st: &State) {
    let Some(m) = st.db.medicine(id) else { return };
    let t = today();
    let warn = st.db.warn_days as i64;
    let row = build_med_row(m, &medicine_info(m, t, warn));

    let model = ui.get_medicines();
    if let Some(vm) = model.as_any().downcast_ref::<VecModel<MedicineRow>>() {
        for i in 0..vm.row_count() {
            if vm.row_data(i).map(|r| r.id == id).unwrap_or(false) {
                vm.set_row_data(i, row);
                break;
            }
        }
    }
}

// ---------- Форма упаковки ----------

fn reset_pkg_form(ui: &AppWindow, st: &mut State) {
    st.pkg_edit_id = -1;
    st.pending_photo = None;
    ui.set_pending_photo(false);
    ui.set_pkg_editing(false);
    ui.set_pkg_expiry(SharedString::new());
    ui.set_pkg_quantity(SharedString::new());
    ui.set_pkg_note(SharedString::new());
    ui.set_pkg_price(SharedString::new());
    ui.set_pkg_period(SharedString::new());
    ui.set_pkg_validation(SharedString::from(
        "Формат: ГГГГ-ММ-ДД, ДД.ММ.ГГГГ или ММ.ГГГГ (годен до конца месяца)",
    ));
    ui.set_pkg_validation_color(color_grey());
    ui.set_pkg_save_enabled(false);
}

/// Живая валидация формы упаковки: срок, период после вскрытия, цена.
fn validate_pkg_form(ui: &AppWindow, st: &State) {
    let text = ui.get_pkg_expiry().to_string();
    if text.trim().is_empty() {
        ui.set_pkg_validation(SharedString::from(
            "Формат: ГГГГ-ММ-ДД, ДД.ММ.ГГГГ или ММ.ГГГГ (годен до конца месяца)",
        ));
        ui.set_pkg_validation_color(color_grey());
        ui.set_pkg_save_enabled(false);
        return;
    }
    let Some((d, month_only)) = parse_date_input(&text) else {
        ui.set_pkg_validation(SharedString::from("Неверная дата"));
        ui.set_pkg_validation_color(color_red());
        ui.set_pkg_save_enabled(false);
        return;
    };
    // период после вскрытия: пусто → не применим
    let period_text = ui.get_pkg_period().to_string();
    let open_period_days = if period_text.trim().is_empty() {
        None
    } else {
        match period_text.trim().parse::<i32>() {
            Ok(v) if (1..=3650).contains(&v) => Some(v),
            _ => {
                ui.set_pkg_validation(SharedString::from(
                    "Период после вскрытия: целое число дней от 1 до 3650",
                ));
                ui.set_pkg_validation_color(color_red());
                ui.set_pkg_save_enabled(false);
                return;
            }
        }
    };
    // цена: пусто → не указана
    let price_text = ui.get_pkg_price().to_string();
    if !price_text.trim().is_empty() {
        let ok = price_text
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .map(|v| (0.0..=1_000_000.0).contains(&v))
            .unwrap_or(false);
        if !ok {
            ui.set_pkg_validation(SharedString::from("Цена: число, например 350 или 249.99"));
            ui.set_pkg_validation_color(color_red());
            ui.set_pkg_save_enabled(false);
            return;
        }
    }
    let days = days_left(d, today());
    let s = status_for(days, st.db.warn_days as i64);
    let mut msg = if month_only {
        format!("месяц: годен до {} · {}", fmt_date(d), status_text(s, days))
    } else {
        status_text(s, days)
    };
    if open_period_days.is_some() {
        msg.push_str(" · период применится после вскрытия");
    }
    ui.set_pkg_validation(SharedString::from(msg));
    ui.set_pkg_validation_color(match s {
        STATUS_EXPIRED => color_red(),
        STATUS_SOON => color_amber(),
        _ => color_green(),
    });
    ui.set_pkg_save_enabled(true);
}

// ---------- Служебные ----------

fn reset_delete_armed(ui: &AppWindow) {
    ui.set_delete_armed(false);
    ui.set_delete_btn_text(SharedString::from("Удалить лекарство"));
}

/// Сохраняет базу на диск и показывает результат в статусной строке.
fn persist(ui: &AppWindow, st: &State) {
    match storage::save(&st.db) {
        Ok(path) => {
            st.unsaved.set(false);
            ui.set_status_message(SharedString::from(format!(
                "Сохранено {} · {}",
                Local::now().format("%H:%M:%S"),
                path.display()
            )));
        }
        Err(e) => ui.set_status_message(SharedString::from(format!("Ошибка сохранения: {e}"))),
    }
}

/// Отложенное автосохранение для правок «на каждое нажатие клавиши»:
/// диск трогается один раз через 800 мс после последнего изменения.
fn schedule_save(
    ui_h: &slint::Weak<AppWindow>,
    s: &Rc<RefCell<State>>,
    timer: &Rc<RefCell<slint::Timer>>,
) {
    s.borrow().unsaved.set(true);
    let ui_h = ui_h.clone();
    let s = s.clone();
    timer.borrow_mut().start(
        slint::TimerMode::SingleShot,
        std::time::Duration::from_millis(800),
        move || {
            if let Some(ui) = ui_h.upgrade() {
                let st = s.borrow();
                if st.unsaved.get() {
                    persist(&ui, &st);
                }
            }        },
    );
}

/// Снимок состояния до мутации + синхронизация кнопок отмены/повтора.
fn commit_history(ui: &AppWindow, st: &mut State, snapshot: Database) {
    st.history.commit(snapshot);
    st.last_commit_kind = None;
    sync_history_ui(ui, st);
}

/// То же с «склейкой» последовательных правок одного поля: правка имени по
/// буквам — это один шаг отмены, а не десяток.
fn commit_history_coalesced(
    ui: &AppWindow,
    st: &mut State,
    snapshot: Database,
    kind: EditKind,
) {
    if st.last_commit_kind != Some(kind) {
        st.history.commit(snapshot);
    }
    st.last_commit_kind = Some(kind);
    sync_history_ui(ui, st);
}

#[derive(PartialEq, Clone, Copy)]
enum EditKind {
    Name,
    Note,
    Form,
    WarnDays,
}

fn sync_history_ui(ui: &AppWindow, st: &State) {
    ui.set_can_undo(st.history.can_undo());
    ui.set_can_redo(st.history.can_redo());
}

fn main() -> Result<(), slint::PlatformError> {
    // Один экземпляр: повторный запуск показывает сообщение и завершается.
    let _instance_guard: Option<std::fs::File> = match acquire_single_instance() {
        InstanceGuard::Locked(file) => Some(file),
        InstanceGuard::NoLockAvailable => None,
        InstanceGuard::AlreadyRunning => {
            notify_already_running();
            return Ok(());
        }
    };

    // Загрузка с диагностикой: при нечитаемом файле НЕ перезаписываем данные.
    let mut startup_warning: Option<String> = None;
    let db = match storage::load() {
        storage::LoadOutcome::Loaded(db) => db,
        storage::LoadOutcome::Missing => Database::default(),
        storage::LoadOutcome::Corrupt { database, backup, msg } => {
            startup_warning = Some(format!(
                "Файл данных был повреждён и переименован в {}. Работаем с пустой базой ({msg}).\n\nЕсли раньше делали «Бэкапы» — откройте их кнопкой «Бэкапы» и восстановите последнюю копию.",
                backup.display()
            ));
            database
        }
        storage::LoadOutcome::Io(e) => {
            message_box(&format!(
                "Не удалось прочитать файл данных:\n{e}\n\nПрограмма закроется, чтобы не перезаписать ваши данные.\nЕсли файл занят другой программой (Excel, редактор) — закройте её и запустите аптечку снова."
            ));
            return Ok(());
        }
    };
    if let Some(w) = &startup_warning {
        message_box(w);
    }

    // Резервная копия текущего состояния — до любых правок этого запуска.
    let backup_note = match storage::backup() {
        Ok(Some(path)) => format!(" · Бэкап: {}", path.display()),
        Ok(None) => String::new(),
        Err(e) => format!(" · Бэкап не удался: {e}"),
    };
    if !storage::data_file_path().exists() {
        // При первом запуске сразу создаём файл данных рядом с exe
        storage::save(&db).ok();
    }
    let ui = AppWindow::new()?;

    // Справочник форм для комбобокса
    let forms: Vec<SharedString> = FORMS.iter().map(|s| SharedString::from(*s)).collect();
    ui.set_form_options(ModelRc::from(Rc::new(VecModel::from(forms))));

    ui.set_warn_days(db.warn_days);
    let startup_msg = match &startup_warning {
        Some(w) => format!(
            "ВНИМАНИЕ: {w} · Данные: {}{backup_note}",
            storage::data_file_path().display()
        ),
        None => format!(
            "Данные: {}{backup_note}",
            storage::data_file_path().display()
        ),
    };
    ui.set_status_message(SharedString::from(startup_msg));

    let state = Rc::new(RefCell::new(State {
        db,
        history: History::new(50),
        selected_id: -1,
        search: String::new(),
        filter_idx: 0,
        sort_idx: 0,
        pkg_edit_id: -1,
        last_commit_kind: None,
        unsaved: Cell::new(false),
        photo_cache: RefCell::new(HashMap::new()),
        pending_photo: None,
        cal_year: Local::now().year(),
        cal_month: Local::now().month(),
    }));

    // Стартовое выделение: лекарство с самым близким сроком годности
    {
        let mut st = state.borrow_mut();
        let t = today();
        let warn = st.db.warn_days as i64;
        let mut best: Option<(i64, i32)> = None;
        for m in &st.db.medicines {
            let info = medicine_info(m, t, warn);
            if info.nearest.is_some() {
                let key = (info.nearest_days, m.id);
                if best.is_none_or(|b| key < b) {
                    best = Some(key);
                }
            }
        }
        st.selected_id = best.map(|b| b.1).unwrap_or(-1);
        ui.set_selected_id(st.selected_id);
        refresh_all(&ui, &mut st, true);
        sync_history_ui(&ui, &st);
    }

    // Отмена последнего действия (Ctrl+Z / кнопка «Отменить»).
    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_undo_history(move || {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            let cur = st.db.clone();
            if let Some(prev) = st.history.undo(cur) {
                st.db = prev;
                st.last_commit_kind = None;
                if st.selected().is_none() {
                    st.selected_id = -1;
                    ui.set_selected_id(-1);
                }
                reset_delete_armed(&ui);
                refresh_all(&ui, &mut st, true);
                sync_history_ui(&ui, &st);
                persist(&ui, &st);
            }
        });
    }

    // Повтор отменённого (Ctrl+Y / Ctrl+Shift+Z / кнопка «Вернуть»).
    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_redo_history(move || {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            let cur = st.db.clone();
            if let Some(next) = st.history.redo(cur) {
                st.db = next;
                st.last_commit_kind = None;
                if st.selected().is_none() {
                    st.selected_id = -1;
                    ui.set_selected_id(-1);
                }
                reset_delete_armed(&ui);
                refresh_all(&ui, &mut st, true);
                sync_history_ui(&ui, &st);
                persist(&ui, &st);
            }
        });
    }

    // Отложенное автосохранение правок текста (диск — не на каждую букву)
    let save_timer: Rc<RefCell<slint::Timer>> =
        Rc::new(RefCell::new(slint::Timer::default()));

    // ----- Колбэки -----

    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_select_medicine(move |id| {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            st.selected_id = id;
            ui.set_selected_id(id);
            ui.set_suggestions(ModelRc::default());
            reset_delete_armed(&ui);
            update_details(&ui, &mut st, true);
        });
    }

    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_add_medicine(move || {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            let prev = st.db.clone();
            let id = st.db.new_med_id();
            st.db.medicines.push(Medicine {
                id,
                name: "Новое лекарство".to_string(),
                form: FORMS[0].to_string(),
                note: String::new(),
                packages: Vec::new(),
            });
            st.db.journal_push("лекарство добавлено", "Новое лекарство", "");
            st.selected_id = id;
            st.search.clear();
            st.filter_idx = 0;
            st.pending_photo = None;
            ui.set_pending_photo(false);
            commit_history(&ui, &mut st, prev);
            ui.set_search_text(SharedString::new());
            ui.set_filter_index(0);
            ui.set_selected_id(id);
            reset_delete_armed(&ui);
            refresh_all(&ui, &mut st, true);
            persist(&ui, &st);
        });
    }

    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_delete_medicine(move || {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            let sel = st.selected_id;
            let prev = st.db.clone();
            let name = st.selected().map(|m| m.name.clone());
            st.db.medicines.retain(|m| m.id != sel);
            if let Some(n) = name {
                st.db.journal_push("лекарство удалено", &n, "");
            }
            st.selected_id = -1;
            ui.set_selected_id(-1);
            reset_delete_armed(&ui);
            commit_history(&ui, &mut st, prev);
            refresh_all(&ui, &mut st, true);
            persist(&ui, &st);
        });
    }

    {
        let h = ui.as_weak();
        let s = state.clone();
        let timer = save_timer.clone();
        ui.on_name_edited(move |text| {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            let prev = st.db.clone();
            if let Some(m) = st.selected_mut() {
                m.name = text.to_string();
            }
            commit_history_coalesced(&ui, &mut st, prev, EditKind::Name);
            // автодополнение: имена других лекарств с тем же началом
            let needle = text.trim().to_lowercase();
            let mut sugg: Vec<SharedString> = Vec::new();
            if !needle.is_empty() {
                for m in &st.db.medicines {
                    if m.id == st.selected_id {
                        continue;
                    }
                    if m.name.to_lowercase().starts_with(&needle)
                        && !m.name.to_lowercase().eq(&needle)
                    {
                        let cand = SharedString::from(m.name.clone());
                        if !sugg.contains(&cand) {
                            sugg.push(cand);
                        }
                        if sugg.len() >= 6 {
                            break;
                        }
                    }
                }
            }
            ui.set_suggestions(ModelRc::from(Rc::new(VecModel::from(sugg))));
            let id = st.selected_id;
            update_row(&ui, id, &st);
            schedule_save(&h, &s, &timer);
        });
    }

    {
        let h = ui.as_weak();
        let s = state.clone();
        let timer = save_timer.clone();
        ui.on_note_edited(move |text| {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            let prev = st.db.clone();
            if let Some(m) = st.selected_mut() {
                m.note = text.to_string();
            }
            commit_history_coalesced(&ui, &mut st, prev, EditKind::Note);
            schedule_save(&h, &s, &timer);
        });
    }

    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_form_selected(move |idx| {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            let prev = st.db.clone();
            let form = FORMS
                .get(idx as usize)
                .map(|f| f.to_string())
                .unwrap_or_else(|| FORMS[FORMS.len() - 1].to_string());
            if let Some(m) = st.selected_mut() {
                m.form = form;
            }
            commit_history_coalesced(&ui, &mut st, prev, EditKind::Form);
            let id = st.selected_id;
            update_row(&ui, id, &st);
            persist(&ui, &st);
        });
    }

    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_search_edited(move |text| {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            st.search = text.to_string();
            rebuild_list(&ui, &st);
        });
    }

    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_filter_selected(move |idx| {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            st.filter_idx = idx as usize;
            rebuild_list(&ui, &st);
        });
    }

    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_sort_selected(move |idx| {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            st.sort_idx = idx as usize;
            rebuild_list(&ui, &st);
        });
    }

    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_warn_days_changed(move |value| {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            let prev = st.db.clone();
            st.db.warn_days = value;
            commit_history_coalesced(&ui, &mut st, prev, EditKind::WarnDays);
            refresh_all(&ui, &mut st, false);
            persist(&ui, &st);
        });
    }

    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_pkg_start_edit(move |pid| {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            let found = st
                .selected()
                .and_then(|m| m.packages.iter().find(|p| p.id == pid))
                .map(|p| {
                    (
                        p.expiry,
                        p.quantity.clone(),
                        p.note.clone(),
                        p.price,
                        p.open_period_days,
                    )
                });
            if let Some((expiry, quantity, note, price, period)) = found {
                // выбранное фото из незавершённого создания не должно «переехать»
                // на редактируемую упаковку
                st.pending_photo = None;
                ui.set_pending_photo(false);
                ui.set_pkg_expiry(SharedString::from(fmt_date(expiry)));
                ui.set_pkg_quantity(SharedString::from(quantity));
                ui.set_pkg_note(SharedString::from(note));
                ui.set_pkg_price(
                    price
                        .map(|v| format!("{v:.2}"))
                        .unwrap_or_default()
                        .into(),
                );
                ui.set_pkg_period(period.map(|v| v.to_string()).unwrap_or_default().into());
                st.pkg_edit_id = pid;
                ui.set_pkg_editing(true);
                validate_pkg_form(&ui, &st);
            }
        });
    }

    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_pkg_cancel_edit(move || {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            reset_pkg_form(&ui, &mut st);
        });
    }

    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_pkg_expiry_edited(move |_| {
            let ui = h.unwrap();
            let st = s.borrow();
            validate_pkg_form(&ui, &st);
        });
    }

    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_pkg_quick(move |months| {
            let ui = h.unwrap();
            let t = today();
            if let Some(d) = t.checked_add_months(Months::new(months.max(0) as u32)) {
                ui.set_pkg_expiry(SharedString::from(fmt_date(d)));
                let st = s.borrow();
                validate_pkg_form(&ui, &st);
            }
        });
    }

    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_pkg_clone(move |pid| {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            // Новая упаковка с теми же количеством/ценой/комментарием и сроком «сегодня + 1 год».
            let found = st
                .selected()
                .and_then(|m| m.packages.iter().find(|p| p.id == pid))
                .map(|p| {
                    (
                        p.quantity.clone(),
                        p.note.clone(),
                        p.price,
                        p.open_period_days,
                    )
                });
            let Some((quantity, note, price, period)) = found else { return };
            let Some(expiry) = today().checked_add_months(Months::new(12)) else { return };
            ui.set_pkg_expiry(SharedString::from(fmt_date(expiry)));
            ui.set_pkg_quantity(SharedString::from(quantity));
            ui.set_pkg_note(SharedString::from(note));
            ui.set_pkg_price(price.map(|v| format!("{v:.2}")).unwrap_or_default().into());
            ui.set_pkg_period(period.map(|v| v.to_string()).unwrap_or_default().into());
            st.pkg_edit_id = -1;
            ui.set_pkg_editing(false);
            validate_pkg_form(&ui, &st);
        });
    }

    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_pkg_save(move || {
            let ui = h.unwrap();
            let Some((expiry, month_only)) = parse_date_input(&ui.get_pkg_expiry()) else {
                return
            };
            let quantity = ui.get_pkg_quantity().to_string();
            let note = ui.get_pkg_note().to_string();
            let price_text = ui.get_pkg_price().to_string();
            let price = if price_text.trim().is_empty() {
                None
            } else {
                price_text.trim().replace(',', ".").parse::<f64>().ok()
            };
            let period_text = ui.get_pkg_period().to_string();
            let open_period_days = if period_text.trim().is_empty() {
                None
            } else {
                period_text.trim().parse::<i32>().ok()
            };
            if open_period_days.is_some_and(|v| !(1..=3650).contains(&v)) {
                return;
            }

            let mut st = s.borrow_mut();
            let prev = st.db.clone();
            let sel = st.selected_id;
            let edit_id = st.pkg_edit_id;
            let mut journal_note: Option<(String, String)> = None;
            match edit_id {
                -1 => {
                    let pid = st.db.new_pkg_id();
                    // отложенное фото выбранной в режиме создания упаковки
                    let mut attached_photo = String::new();
                    if let Some(src) = st.pending_photo.take() {
                        ui.set_pending_photo(false);
                        if let Some(ext) = src.extension().and_then(|e| e.to_str()) {
                            let dir = storage::exe_dir().join("photos");
                            let _ = std::fs::create_dir_all(&dir);
                            let fname = format!("pkg_{pid}.{ext}");
                            if std::fs::copy(&src, dir.join(&fname)).is_ok() {
                                st.photo_cache.borrow_mut().remove(&fname);
                                attached_photo = fname;
                            }
                        }
                    }
                    if let Some(m) = st.db.medicine_mut(sel) {
                        m.packages.push(Package {
                            id: pid,
                            expiry,
                            month_only,
                            quantity: quantity.clone(),
                            note: note.clone(),
                            price,
                            opened: None,
                            open_period_days,
                            disposed: None,
                            photo: attached_photo,
                        });
                        journal_note = Some((
                            m.name.clone(),
                            format!("срок {}", fmt_date_display(expiry, month_only)),
                        ));
                    }
                }
                pid => {
                    if let Some(m) = st.db.medicine_mut(sel) {
                        if let Some(p) = m.packages.iter_mut().find(|p| p.id == pid) {
                            p.expiry = expiry;
                            p.month_only = month_only;
                            p.quantity = quantity.clone();
                            p.note = note.clone();
                            p.price = price;
                            p.open_period_days = open_period_days;
                            journal_note = Some((
                                m.name.clone(),
                                format!("срок {}", fmt_date_display(expiry, month_only)),
                            ));
                        }
                    }
                }
            }
            commit_history(&ui, &mut st, prev);
            if let Some((name, pdesc)) = journal_note {
                let kind = if edit_id >= 0 {
                    "изменена упаковка"
                } else {
                    "добавлена упаковка"
                };
                st.db.journal_push(kind, &name, &pdesc);
            }
            reset_pkg_form(&ui, &mut st);
            refresh_all(&ui, &mut st, false);
            persist(&ui, &st);
        });
    }

    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_pkg_delete(move |pid| {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            let prev = st.db.clone();
            let mut jnote: Option<(String, String)> = None;
            if let Some(m) = st.selected_mut() {
                if let Some(p) = m.packages.iter().find(|p| p.id == pid) {
                    jnote = Some((m.name.clone(), pkg_desc(p)));
                }
                m.packages.retain(|p| p.id != pid);
            }
            if let Some((n, d)) = jnote {
                st.db.journal_push("упаковка удалена", &n, &d);
            }
            if st.pkg_edit_id == pid {
                st.pkg_edit_id = -1;
            }
            commit_history(&ui, &mut st, prev);
            refresh_all(&ui, &mut st, false);
            persist(&ui, &st);
        });
    }

    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_export_csv(move || {
            let ui = h.unwrap();
            let st = s.borrow();
            match storage::export_csv(&st.db) {
                Ok(path) => ui.set_status_message(SharedString::from(format!(
                    "Экспортировано: {}",
                    path.display()
                ))),
                Err(e) => {
                    ui.set_status_message(SharedString::from(format!("Ошибка экспорта: {e}")))
                }
            }
        });
    }

    // ===== Жизненный цикл упаковки: открытие, списание, возврат =====

    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_pkg_open(move |pid| {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            let prev = st.db.clone();
            let mut jnote: Option<(String, String)> = None;
            if let Some(m) = st.selected_mut() {
                if let Some(p) = m
                    .packages
                    .iter_mut()
                    .find(|p| p.id == pid && !p.is_disposed())
                {
                    p.opened = Some(today());
                    let eff = p.effective_expiry();
                    jnote = Some((
                        m.name.clone(),
                        format!("{} · годен до {}", pkg_desc(p), fmt_date(eff)),
                    ));
                }
            }
            if jnote.is_some() {
                commit_history(&ui, &mut st, prev);
                if let Some((n, d)) = jnote {
                    st.db.journal_push("упаковка открыта", &n, &d);
                }
                refresh_all(&ui, &mut st, false);
                persist(&ui, &st);
            }
        });
    }

    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_pkg_dispose(move |pid, reason| {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            let prev = st.db.clone();
            let mut jnote: Option<(String, String)> = None;
            if let Some(m) = st.selected_mut() {
                if let Some(p) = m
                    .packages
                    .iter_mut()
                    .find(|p| p.id == pid && !p.is_disposed())
                {
                    p.disposed = Some(DisposedInfo {
                        date: today(),
                        reason,
                    });
                    jnote = Some((
                        m.name.clone(),
                        format!("{} ({})", pkg_desc(p), dispose_reason_text(reason)),
                    ));
                }
            }
            if jnote.is_some() {
                commit_history(&ui, &mut st, prev);
                if let Some((n, d)) = jnote {
                    st.db.journal_push("упаковка списана", &n, &d);
                }
                refresh_all(&ui, &mut st, false);
                persist(&ui, &st);
            }
        });
    }

    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_pkg_restore(move |pid| {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            let prev = st.db.clone();
            let mut jnote: Option<(String, String)> = None;
            if let Some(m) = st.selected_mut() {
                if let Some(p) = m
                    .packages
                    .iter_mut()
                    .find(|p| p.id == pid && p.is_disposed())
                {
                    p.disposed = None;
                    jnote = Some((m.name.clone(), pkg_desc(p)));
                }
            }
            if jnote.is_some() {
                commit_history(&ui, &mut st, prev);
                if let Some((n, d)) = jnote {
                    st.db.journal_push("упаковка возвращена в работу", &n, &d);
                }
                refresh_all(&ui, &mut st, false);
                persist(&ui, &st);
            }
        });
    }

    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_toggle_show_disposed(move || {
            let ui = h.unwrap();
            let st = s.borrow();
            ui.set_show_disposed(!ui.get_show_disposed());
            rebuild_packages(&ui, &st);
        });
    }

    // Журнал событий: отдельное окно, наполняется при каждом открытии.
    let journal = JournalWindow::new()?;
    {
        let jh = journal.as_weak();
        journal.on_close_clicked(move || {
            let j = jh.unwrap();
            let _ = j.hide();
        });
    }
    {
        let s = state.clone();
        let jh = journal.as_weak();
        ui.on_open_journal(move || {
            let j = jh.unwrap();
            let st = s.borrow();
            let rows: Vec<SharedString> = st
                .db
                .events
                .iter()
                .rev()
                .map(|e| {
                    SharedString::from(format!(
                        "{} · {} — {} {}",
                        e.at.format("%d.%m.%Y %H:%M"),
                        e.medicine,
                        e.kind,
                        e.package
                    ))
                })
                .collect();
            j.set_events(ModelRc::from(Rc::new(VecModel::from(rows))));
            let _ = j.show();
        });
    }

    // Автодополнение названия по уже заведённым лекарствам
    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_suggestion_picked(move |text| {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            let prev = st.db.clone();
            if let Some(m) = st.selected_mut() {
                m.name = text.to_string();
            }
            commit_history(&ui, &mut st, prev);
            ui.set_detail_name(text);
            ui.set_suggestions(ModelRc::default());
            let id = st.selected_id;
            update_row(&ui, id, &st);
            persist(&ui, &st);
        });
    }

    // Календарь: открытие (месяц из текущего срока), листание, выбор дня
    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_cal_month_opened(move || {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            match parse_date_input(&ui.get_pkg_expiry()) {
                Some((d, _)) => {
                    st.cal_year = d.year();
                    st.cal_month = d.month();
                }
                None => {
                    let t = today();
                    st.cal_year = t.year();
                    st.cal_month = t.month();
                }
            }
            cal_rebuild(&ui, &st);
        });
    }
    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_cal_prev_month(move || {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            st.cal_month -= 1;
            if st.cal_month == 0 {
                st.cal_month = 12;
                st.cal_year -= 1;
            }
            cal_rebuild(&ui, &st);
        });
    }
    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_cal_next_month(move || {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            st.cal_month += 1;
            if st.cal_month > 12 {
                st.cal_month = 1;
                st.cal_year += 1;
            }
            cal_rebuild(&ui, &st);
        });
    }
    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_cal_pick(move |ymd| {
            let ui = h.unwrap();
            let st = s.borrow_mut();
            if let Ok(d) = NaiveDate::parse_from_str(ymd.as_str(), "%Y-%m-%d") {
                ui.set_pkg_expiry(SharedString::from(fmt_date(d)));
                validate_pkg_form(&ui, &st);
            }
        });
    }

    // Список покупок: окно + копирование в буфер обмена
    let shopping = ShoppingWindow::new()?;
    {
        let s = state.clone();
        let jh = shopping.as_weak();
        ui.on_open_shopping(move || {
            let j = jh.unwrap();
            let st = s.borrow();
            let rows: Vec<SharedString> = shopping_list(&st.db, today())
                .into_iter()
                .map(SharedString::from)
                .collect();
            j.set_lines(ModelRc::from(Rc::new(VecModel::from(rows))));
            j.set_note(SharedString::new());
            let _ = j.show();
        });
    }
    {
        let jh = shopping.as_weak();
        shopping.on_copy_to_clipboard(move || {
            let j = jh.unwrap();
            let model = j.get_lines();
            if model.row_count() == 0 {
                j.set_note(SharedString::from("Список пуст — нечего копировать."));
                return;
            }
            let text: String = (0..model.row_count())
                .filter_map(|i| model.row_data(i))
                .collect::<Vec<_>>()
                .join("\n");
            let note = match arboard::Clipboard::new() {
                Ok(mut cb) => match cb.set_text(text.clone()) {
                    Ok(()) => format!(
                        "Скопировано в буфер обмена ({} поз.)",
                        text.lines().count()
                    ),
                    Err(e) => format!("Не удалось скопировать: {e}"),
                },
                Err(e) => format!("Не удалось скопировать: {e}"),
            };
            j.set_note(SharedString::from(note));
        });
    }
    {
        let jh = shopping.as_weak();
        shopping.on_close_clicked(move || {
            let _ = jh.unwrap().hide();
        });
    }

    // Резервные копии: окно восстановления
    let backups = BackupsWindow::new()?;
    let backup_files: Rc<RefCell<Vec<std::path::PathBuf>>> = Rc::new(RefCell::new(Vec::new()));
    {
        let files = backup_files.clone();
        let jh = backups.as_weak();
        ui.on_open_backups(move || {
            let j = jh.unwrap();
            let files_now = storage::list_backups();
            let rows: Vec<SharedString> = files_now
                .iter()
                .map(|p| SharedString::from(p.file_name().unwrap_or_default().to_string_lossy().to_string()))
                .collect();
            j.set_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
            j.set_selected(-1);
            j.set_note(SharedString::new());
            *files.borrow_mut() = files_now;
            let _ = j.show();
        });
    }
    {
        let h = ui.as_weak();
        let s = state.clone();
        let files = backup_files.clone();
        let jh = backups.as_weak();
        backups.on_restore(move |idx| {
            let ui = h.unwrap();
            let j = jh.unwrap();
            let Some(path) = files.borrow().get(idx as usize).cloned() else { return };
            // страховка: текущее состояние — в бэкап, и его же можно вернуть Ctrl+Z.
            // Если страховочный бэкап создать не удалось — восстановление отменяем:
            // иначе сбой диска приведёт к безвозвратной потере текущих данных.
            if let Err(e) = storage::backup() {
                j.set_note(SharedString::from(format!(
                    "Не удалось сохранить бэкап текущих данных ({e}) — восстановление отменено."
                )));
                return;
            }
            let db = match storage::load_database_from(&path) {
                Ok(db) => db,
                Err(e) => {
                    j.set_note(SharedString::from(format!(
                        "Не удалось восстановить: {e}. Текущие данные не тронуты."
                    )));
                    return;
                }
            };
            let fname = path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let mut st = s.borrow_mut();
            let prev = st.db.clone();
            st.history.commit(prev);
            st.last_commit_kind = None;
            st.db = db;
            st.selected_id = -1;
            ui.set_selected_id(-1);
            reset_delete_armed(&ui);
            refresh_all(&ui, &mut st, true);
            sync_history_ui(&ui, &st);
            st.db.journal_push("восстановлено из бэкапа", &fname, "");
            persist(&ui, &st);
            j.set_note(SharedString::from(format!("Восстановлено: {fname} (отменить — Ctrl+Z)")));
        });
    }
    {
        let jh = backups.as_weak();
        backups.on_close_clicked(move || {
            let _ = jh.unwrap().hide();
        });
    }

    // Импорт CSV: окно выбора файла + разбор
    let import_win = ImportWindow::new()?;
    let import_files: Rc<RefCell<Vec<std::path::PathBuf>>> = Rc::new(RefCell::new(Vec::new()));
    {
        let files = import_files.clone();
        let jh = import_win.as_weak();
        ui.on_open_import(move || {
            let j = jh.unwrap();
            let mut files_now: Vec<std::path::PathBuf> = std::fs::read_dir(storage::exe_dir())
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.is_file()
                        && p.extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("csv"))
                })
                .collect();
            files_now.sort();
            files_now.reverse();
            let rows: Vec<SharedString> = files_now
                .iter()
                .map(|p| SharedString::from(p.file_name().unwrap_or_default().to_string_lossy().to_string()))
                .collect();
            j.set_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
            j.set_selected(-1);
            j.set_note(SharedString::new());
            *files.borrow_mut() = files_now;
            let _ = j.show();
        });
    }
    {
        let h = ui.as_weak();
        let s = state.clone();
        let files = import_files.clone();
        let jh = import_win.as_weak();
        import_win.on_do_import(move |idx| {
            let ui = h.unwrap();
            let j = jh.unwrap();
            let Some(path) = files.borrow().get(idx as usize).cloned() else { return };
            let (mut meds, skipped) = match storage::import_csv(&path) {
                Ok(r) => r,
                Err(e) => {
                    j.set_note(SharedString::from(format!("Ошибка чтения: {e}")));
                    return;
                }
            };
            if meds.is_empty() {
                j.set_note(SharedString::from("В файле нет корректных строк."));
                return;
            }
            let mut st = s.borrow_mut();
            let prev = st.db.clone();
            let mut total_pkgs = 0usize;
            for m in &mut meds {
                m.id = st.db.new_med_id();
                for p in &mut m.packages {
                    p.id = st.db.new_pkg_id();
                    total_pkgs += 1;
                }
                st.db.journal_push(
                    "импорт из CSV",
                    &m.name,
                    &format!("упаковок: {}", m.packages.len()),
                );
            }
            let imported_count = meds.len();
            st.db.medicines.append(&mut meds);
            commit_history(&ui, &mut st, prev);
            st.selected_id = -1;
            ui.set_selected_id(-1);
            refresh_all(&ui, &mut st, true);
            persist(&ui, &st);
            j.set_note(SharedString::from(format!(
                "Импортировано лекарств: {imported_count} (упаковок: {total_pkgs}, пропущено строк: {skipped})",
            )));
        });
    }
    {
        let jh = import_win.as_weak();
        import_win.on_close_clicked(move || {
            let _ = jh.unwrap().hide();
        });
    }

    // Фото упаковки: выбор файла и копия в photos/
    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_pick_photo(move || {
            let ui = h.unwrap();
            // ВАЖНО: не держим заём состояния во время модального диалога rfd —
            // диалог прокачивает message loop, и сработавший таймер мог бы
            // захватить borrow() поверх borrow_mut() (паника BorrowMutError).
            let editing = { let st = s.borrow(); (st.pkg_edit_id, st.pending_photo.is_some()) };
            let Some(file) = rfd::FileDialog::new()
                .set_title("Выберите фото упаковки")
                .add_filter("Изображения", &["png", "jpg", "jpeg"])
                .pick_file()
            else {
                return;
            };

            if editing.0 >= 0 {
                // редактируем существующую — копируем сразу
                let pid = editing.0;
                let ext = file
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| e.to_lowercase())
                    .unwrap_or_else(|| "png".into());
                let dir = storage::exe_dir().join("photos");
                if let Err(e) = std::fs::create_dir_all(&dir) {
                    ui.set_status_message(SharedString::from(format!(
                        "Не удалось создать папку photos: {e}"
                    )));
                    return;
                }
                let fname = format!("pkg_{pid}.{ext}");
                match std::fs::copy(&file, dir.join(&fname)) {
                    Ok(_) => {
                        {
                            let mut st = s.borrow_mut();
                            st.photo_cache.borrow_mut().remove(&fname);
                            if let Some(m) = st.selected_mut() {
                                if let Some(p) = m.packages.iter_mut().find(|p| p.id == pid) {
                                    p.photo = fname;
                                }
                            }
                        }
                        rebuild_packages(&ui, &s.borrow());
                        ui.set_status_message(SharedString::from("Фото упаковки обновлено"));
                    }
                    Err(e) => {
                        ui.set_status_message(SharedString::from(format!(
                            "Не удалось скопировать фото: {e}"
                        )));
                    }
                }
            } else {
                // новая упаковка: прикрепим файл при сохранении
                let mut st = s.borrow_mut();
                st.pending_photo = Some(file);
                ui.set_pending_photo(true);
            }
        });
    }

    // Часовой пересчёт статусов: подсветка «истекает / просрочено» не устаревает,
    // пока приложение открыто через полночь.
    let status_timer = slint::Timer::default();
    {
        let h = ui.as_weak();
        let s = state.clone();
        status_timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_secs(3600),
            move || {
                let ui = h.unwrap();
                let st = s.borrow();
                refresh_statuses(&ui, &st);
            },
        );
    }

    // Навигация по списку с клавиатуры (стрелки ↑/↓ в FocusScope).
    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_move_selection(move |delta| {
            let ui = h.unwrap();
            let mut st = s.borrow_mut();
            move_selection(&ui, &mut st, delta);
        });
    }

    ui.run()?;

    // Окно закрыто: сбрасываем несохранённые правки текста (окно дебаунса).
    {
        let st = state.borrow();
        if st.unsaved.get() {
            let _ = storage::save(&st.db);
        }
    }
    Ok(())
}
