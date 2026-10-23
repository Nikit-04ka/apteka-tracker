// Прячет консольное окно в релизной сборке (GUI-приложение).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Точка входа: загрузка данных, сборка окна Slint, связка колбэков.

use apteka_core::chrono::{Datelike, Local, NaiveDate};
use apteka_core::model::*;
use apteka_core::settings::{self, AppSettings};
use apteka_core::state::{initial_selection, DomainState};
use apteka_core::storage;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

mod card;
mod misc;
mod packages;
mod state;
mod ui_sync;
mod windows;

use crate::card::*;
use crate::misc::*;
use crate::packages::*;
use crate::state::*;
use crate::ui_sync::*;
use crate::windows::*;

slint::include_modules!();

/// Сегодняшняя дата (без времени).
pub(crate) fn today() -> NaiveDate {
    Local::now().date_naive()
}

/// Разворачивает стандартный каркас Slint-колбэка: захват `Weak` окна и клона
/// `Rc<RefCell<UiState>>`, upgrade внутри замыкания и заём состояния на всё тело.
///
/// - `with_state!(ui, state, on_cb, |ui, st| { ... })` — только чтение (`borrow`).
/// - `with_state!(ui, state, on_cb, |ui, mut st| { ... })` — мутация (`borrow_mut`).
/// - Аргументы колбэка перечисляются после состояния: `|ui, mut st, delta| { ... }`.
///
/// Не используется там, где заём нельзя держать всё тело (диалог `rfd` в
/// `on_pick_photo` прокачивает message loop) или нужны ещё один Weak/окно.
macro_rules! with_state {
    ($ui:expr, $state:expr, $cb:ident, |$u:ident, mut $st:ident $(, $arg:ident)*| $body:block) => {{
        let h = $ui.as_weak();
        let s = $state.clone();
        $ui.$cb(move |$($arg),*| {
            let $u = h.unwrap();
            let mut $st = s.borrow_mut();
            $body
        })
    }};
    ($ui:expr, $state:expr, $cb:ident, |$u:ident, $st:ident $(, $arg:ident)*| $body:block) => {{
        let h = $ui.as_weak();
        let s = $state.clone();
        $ui.$cb(move |$($arg),*| {
            let $u = h.unwrap();
            let $st = s.borrow();
            $body
        })
    }}; }

// ---------- Один экземпляр программы ----------

pub(crate) use with_state;

/// Состояние замка `apteka.lock` рядом с exe.
pub(crate) enum InstanceGuard {
    /// Замок захвачен — работаем. Файл держится открытым всё время работы:
    /// замок освобождается автоматически даже при сбое процесса.
    Locked(std::fs::File),
    /// Замок создать не удалось (носитель только для чтения) — работаем без него.
    NoLockAvailable,
    /// Уже запущено другим процессом.
    AlreadyRunning,
}

pub(crate) fn acquire_single_instance() -> InstanceGuard {
    acquire_lock(&storage::exe_dir().join("apteka.lock"))
}

/// Захват замка по явному пути: выделено, чтобы сценарий «второй запуск» был
/// покрыт тестом на временной папке.
pub(crate) fn acquire_lock(path: &std::path::Path) -> InstanceGuard {
    let file = match std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
    {
        Ok(f) => f,
        Err(_) => return InstanceGuard::NoLockAvailable,
    };
    match file.try_lock() {
        Ok(()) => InstanceGuard::Locked(file),
        // Занят другим процессом — это действительно «уже запущена».
        Err(std::fs::TryLockError::WouldBlock) => InstanceGuard::AlreadyRunning,
        // Ошибка блокировки (ФС без поддержки замков) — работаем без замка,
        // а не отказываем в запуске с ложным «уже запущена».
        Err(std::fs::TryLockError::Error(_)) => InstanceGuard::NoLockAvailable,
    }
}

/// Модальное сообщение для пользователя (в release консоли нет).
pub(crate) fn message_box(text: &str) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONWARNING, MB_OK};
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

/// Формирует запись crash-лога (отдельная функция — для теста).
pub(crate) fn panic_log_entry(stamp: &str, version: &str, payload: &str, place: &str) -> String {
    format!("=== {stamp} · v{version} ===\npanic: {payload}\n  at {place}\n\n")
}

/// Чистка потерянных фото безопасна только когда база реально прочитана из
/// файла данных: при Corrupt/Missing база пустая не по воле пользователя, и
/// чистка удалила бы фото, на которые ссылается восстанавливаемый из бэкапа
/// файл — данные пропали бы безвозвратно.
pub(crate) fn orphan_cleanup_allowed(outcome: &storage::LoadOutcome) -> bool {
    matches!(outcome, storage::LoadOutcome::Loaded(_))
}

/// Паника: подробности — в `apteka_crash.log` рядом с exe и в сообщение.
/// В release консоли нет, молчаливый вылет не оставляет следов; лог нужен,
/// чтобы сбой можно было сообщить и починить.
pub(crate) fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let stamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let payload = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| s.to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "неизвестная паника".to_string());
        let place = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "место неизвестно".to_string());
        let thread = std::thread::current()
            .name()
            .unwrap_or("<без имени>")
            .to_string();
        let entry = panic_log_entry(&stamp, env!("CARGO_PKG_VERSION"), &payload, &place);
        let path = storage::exe_dir().join("apteka_crash.log");
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            use std::io::Write;
            let _ = writeln!(f, "{entry}поток: {thread}");
        }
        message_box(
            "Программа аварийно завершается.\nПодробности записаны в apteka_crash.log рядом с программой — пришлите его вместе с описанием действий, которые к этому привели.",
        );
        default_hook(info);
    }));
}

#[cfg(windows)]
pub(crate) fn notify_already_running() {
    message_box("Домашняя аптечка уже запущена.");
}

#[cfg(not(windows))]
pub(crate) fn notify_already_running() {
    eprintln!("Домашняя аптечка уже запущена.");
}

fn main() -> Result<(), slint::PlatformError> {
    // Паника: лог + сообщение вместо молчаливого вылета (в release консоли нет).
    install_panic_hook();

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
    let load_outcome = storage::load();
    let cleanup_photos = orphan_cleanup_allowed(&load_outcome);
    let mut startup_warning: Option<String> = None;
    let db = match load_outcome {
        storage::LoadOutcome::Loaded(db) => db,
        storage::LoadOutcome::Missing => Database::default(),
        storage::LoadOutcome::Corrupt {
            database,
            backup,
            msg,
        } => {
            startup_warning = Some(format!(
                "Файл данных был повреждён и сохранён как {}. Работаем с пустой базой ({msg}).\n\nЕсли раньше делали «Бэкапы» — откройте их кнопкой «Бэкапы» и восстановите последнюю копию.",
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
    // backup_done = false (сбой) → первая мутация повторит бэкап сегодня же.
    let (backup_note, backup_done) = match storage::backup() {
        Ok(Some(path)) => (format!(" · Бэкап: {}", path.display()), true),
        Ok(None) => (String::new(), true),
        Err(e) => (format!(" · Бэкап не удался: {e}"), false),
    };
    // Потерянные фото (pkg_* без ссылок в базе) — мусор от прошлых удалений.
    // Чистим только при реально прочитанной базе — см. orphan_cleanup_allowed.
    if cleanup_photos {
        storage::cleanup_orphan_photos(&db);
    }
    if !storage::data_file_path().exists() {
        // При первом запуске сразу создаём файл данных рядом с exe;
        // ошибка не проглатывается — иначе она вскроется только при первой
        // мутации (F5).
        if let Err(e) = storage::save(&db) {
            message_box(&format!(
                "Не удалось создать файл данных:\n{e}\n\nФайл: {}",
                storage::data_file_path().display()
            ));
        }
    }
    let ui = AppWindow::new()?;

    // Настройки окна и темы (settings.json рядом с exe).
    let settings: Rc<RefCell<AppSettings>> = Rc::new(RefCell::new(settings::load_settings()));
    apply_window_settings(&ui, &settings.borrow());

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

    let state = Rc::new(RefCell::new(UiState {
        domain: DomainState::new(db, 50),
        selected_id: -1,
        search: String::new(),
        filter_idx: 0,
        sort_idx: 0,
        tag_filter: None,
        pkg_edit_id: -1,
        photo_cache: RefCell::new(HashMap::new()),
        last_backup_date: Cell::new(backup_done.then(today)),
        pending_photo: None,
        cal_year: Local::now().year(),
        cal_month: Local::now().month(),
    }));

    // Стартовое выделение: лекарство с самым близким сроком годности
    {
        let mut st = state.borrow_mut();
        st.selected_id = initial_selection(&st.domain.db, today());
        ui.set_selected_id(st.selected_id);
        refresh_all(&ui, &mut st, true);
        sync_history_ui(&ui, &st);
    }

    // Отмена последнего действия (Ctrl+Z / кнопка «Отменить»).
    with_state!(ui, state, on_undo_history, |ui, mut st| {
        history_step(&ui, &mut st, true);
    });

    // Повтор отменённого (Ctrl+Y / Ctrl+Shift+Z / кнопка «Вернуть»).
    with_state!(ui, state, on_redo_history, |ui, mut st| {
        history_step(&ui, &mut st, false);
    });

    // Отложенное автосохранение правок текста (диск — не на каждую букву):
    // таймер перезапускается при каждой правке, диск трогается один раз
    // через 800 мс после последнего изменения.
    let save_timer: Rc<slint::Timer> = Rc::new(slint::Timer::default());
    {
        let h = ui.as_weak();
        let s = state.clone();
        save_timer.start(
            slint::TimerMode::SingleShot,
            std::time::Duration::from_millis(800),
            move || {
                if let Some(ui) = h.upgrade() {
                    let st = s.borrow();
                    if st.domain.unsaved.get() {
                        persist(&ui, &st);
                    }
                }
            },
        );
    }

    // Переключение темы: состояние уже перевернуто в .slint — сохраняем выбор.
    {
        let s_set = settings.clone();
        with_state!(ui, state, on_theme_dark_changed, |_ui, _st, dark| {
            s_set.borrow_mut().dark_theme = dark;
            settings::save_settings(&s_set.borrow());
        });
    }

    // ----- Колбэки -----

    register_card_cbs(&ui, &state, &save_timer);

    register_package_cbs(&ui, &state);

    // Журнал событий: отдельное окно, наполняется при каждом открытии.
    // Вторичные окна: создаются и связываются в одном месте (держим до конца main).
    let _windows = build_windows(&ui, &state)?;
    register_misc_cbs(&ui, &state);
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
    with_state!(ui, state, on_move_selection, |ui, mut st, delta| {
        move_selection(&ui, &mut st, delta);
    });

    ui.run()?;

    // Окно закрыто: сохраняем геометрию окна и несохранённые правки текста.
    collect_window_settings(&ui, &mut settings.borrow_mut());
    settings::save_settings(&settings.borrow());
    {
        let st = state.borrow();
        if st.domain.unsaved.get() {
            if let Err(e) = storage::save(&st.domain.db) {
                // окно уже закрыто — статусную строку уже не увидеть,
                // ошибка только в сообщении
                message_box(&format!(
                    "Не удалось сохранить данные при выходе:\n{e}\n\nФайл: {}",
                    storage::data_file_path().display()
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{acquire_lock, orphan_cleanup_allowed, panic_log_entry, InstanceGuard};
    use apteka_core::model::Database;
    use apteka_core::storage::LoadOutcome;
    use std::path::PathBuf;

    #[test]
    fn panic_entry_contains_all_parts() {
        let e = panic_log_entry(
            "2026-10-21 12:00:00",
            "1.3.0",
            "index out of bounds",
            "src/main.rs:10:5",
        );
        assert!(e.contains("=== 2026-10-21 12:00:00 · v1.3.0 ==="));
        assert!(e.contains("panic: index out of bounds"));
        assert!(e.contains("at src/main.rs:10:5"));
    }

    #[test]
    fn orphan_photos_cleanup_only_when_database_loaded() {
        // при Corrupt/Missing база пустая «по ошибке»: чистка сирот удалила бы
        // все фото и добила бы восстановление из бэкапа
        assert!(orphan_cleanup_allowed(&LoadOutcome::Loaded(
            Database::default()
        )));
        assert!(!orphan_cleanup_allowed(&LoadOutcome::Missing));
        assert!(!orphan_cleanup_allowed(&LoadOutcome::Corrupt {
            database: Database::default(),
            backup: PathBuf::from("apteka_data.json.corrupt-test"),
            msg: "битый json".to_string(),
        }));
        assert!(!orphan_cleanup_allowed(&LoadOutcome::Io(
            "файл занят".to_string()
        )));
    }

    #[test]
    fn lock_blocks_second_instance_and_releases_on_drop() {
        // второй запуск видит «уже запущена», пока замок держится;
        // после выхода замок освобождается автоматически (F2)
        let dir = std::env::temp_dir().join(format!("apteka-lock-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("apteka.lock");
        let first = acquire_lock(&path);
        assert!(
            matches!(&first, InstanceGuard::Locked(_)),
            "первый запуск получает замок"
        );
        assert!(
            matches!(acquire_lock(&path), InstanceGuard::AlreadyRunning),
            "пока замок держится — «уже запущена»"
        );
        drop(first);
        let again = acquire_lock(&path);
        assert!(
            matches!(&again, InstanceGuard::Locked(_)),
            "после выхода замок свободен"
        );
        drop(again);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
