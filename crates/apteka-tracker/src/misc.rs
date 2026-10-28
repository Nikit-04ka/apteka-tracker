// Служебное: цвета, флаги вида, persist, история отмены, настройки окна.

use apteka_core::chrono::{Datelike, Local, NaiveDate};
use apteka_core::model::*;
use apteka_core::settings::AppSettings;
use apteka_core::state::EditKind;
use slint::private_unstable_api::re_exports::ColorScheme;
use slint::{Color, ComponentHandle, Model, ModelRc, SharedString};
use std::cell::RefCell;

use crate::card::*;
use crate::packages::*;
use crate::state::*;
use crate::ui_sync::*;
use crate::with_state;
use crate::*;

pub(crate) fn color_grey() -> Color {
    Color::from_rgb_u8(0x9c, 0xa3, 0xaf)
}

pub(crate) fn color_red() -> Color {
    Color::from_rgb_u8(0xdc, 0x26, 0x26)
}

pub(crate) fn color_amber() -> Color {
    Color::from_rgb_u8(0xd9, 0x77, 0x06)
}

pub(crate) fn color_green() -> Color {
    Color::from_rgb_u8(0x16, 0xa3, 0x4a)
}

// ---------- Построение строк UI ----------

/// Сбрасывает «взведённые» флаги вида: подтверждение удаления и показ
/// списанных не должны перетекать между лекарствами и через шаги истории.
pub(crate) fn reset_view_flags(ui: &AppWindow) {
    ui.set_delete_armed(false);
    ui.set_delete_btn_text(SharedString::from("Удалить лекарство"));
    ui.set_pkg_open_armed(-1);
    ui.set_pkg_delete_armed(-1);
    ui.set_show_disposed(false);
}

/// Сбрасывает взведённые подтверждения (удаление лекарства кнопкой/клавишей
/// Delete, «Точно открыть?» и «Точно удалить?» на карточке упаковки), не трогая
/// остальные флаги вида. Вызывается при действиях, отличных от самого
/// подтверждаемого, чтобы «Точно …?» не переживало чужое действие и не
/// выполнялось с одного нажатия (F19).
pub(crate) fn disarm_delete(ui: &AppWindow) {
    ui.set_delete_armed(false);
    ui.set_delete_btn_text(SharedString::from("Удалить лекарство"));
    ui.set_pkg_open_armed(-1);
    ui.set_pkg_delete_armed(-1);
}

/// Показывает ошибку в статусной строке: красной переносимой строкой, до
/// следующей успешной операции (успешный `persist` сбрасывает). Обычные
/// сообщения при этом не теряются — ошибка видна, пока её не починили.
pub(crate) fn report_error(ui: &AppWindow, msg: String) {
    ui.set_save_error(true);
    ui.set_status_message(SharedString::from(msg));
}

/// Информационное сообщение в статусную строку: активную ошибку не затирает —
/// подтверждения вида «Удалено: …» не должны вытеснять текст сбоя сохранения.
pub(crate) fn set_status_info(ui: &AppWindow, msg: String) {
    if !ui.get_save_error() {
        ui.set_status_message(SharedString::from(msg));
    }
}

/// В форме упаковки есть несохранённый ввод пользователя? Нужно, чтобы при
/// смене лекарства сообщать о сбросе формы, а не молча терять ввод.
pub(crate) fn pkg_form_has_input(ui: &AppWindow) -> bool {
    ui.get_pkg_editing()
        || ui.get_pending_photo()
        || !ui.get_pkg_expiry().trim().is_empty()
        || !ui.get_pkg_quantity().trim().is_empty()
        || !ui.get_pkg_note().trim().is_empty()
        || !ui.get_pkg_price().trim().is_empty()
        || !ui.get_pkg_period().trim().is_empty()
        || !ui.get_pkg_purchase_date().trim().is_empty()
}

/// Сохраняет базу на диск и показывает результат в статусной строке.
pub(crate) fn persist(ui: &AppWindow, st: &UiState) {
    // Бэкап раз в сутки: первая мутация нового дня — копия прежнего состояния
    // до записи. При успешном старточном бэкапе здесь тихо до полуночи; если
    // стартовый бэкап не удался — первая мутация повторит попытку.
    if st.last_backup_date.get() != Some(today()) && storage::backup().is_ok() {
        st.last_backup_date.set(Some(today()));
    }
    match storage::save(&st.domain.db) {
        Ok(path) => {
            st.domain.unsaved.set(false);
            ui.set_save_error(false);
            ui.set_status_message(SharedString::from(format!(
                "Сохранено {} · {}",
                Local::now().format("%H:%M:%S"),
                path.display()
            )));
        }
        Err(e) => {
            st.domain.unsaved.set(true);
            report_error(
                ui,
                format!("ОШИБКА сохранения: {e} — проверьте, что диск и папка доступны для записи"),
            );
        }
    }
}

/// Применяет сохранённые настройки к окну: тема, размер, позиция/развернутость.
pub(crate) fn apply_window_settings(ui: &AppWindow, s: &AppSettings) {
    if s.dark_theme {
        ui.set_theme_dark(true);
        ui.global::<Palette>().set_color_scheme(ColorScheme::Dark);
    }
    let win = ui.window();
    if s.maximized {
        win.set_maximized(true);
        return;
    }
    if let (Some(w), Some(h)) = (s.win_w, s.win_h) {
        if w >= 400.0 && h >= 300.0 {
            win.set_size(slint::WindowSize::Logical(slint::LogicalSize::new(w, h)));
        }
    }
    if let (Some(x), Some(y)) = (s.win_x, s.win_y) {
        win.set_position(slint::WindowPosition::Logical(slint::LogicalPosition::new(
            x as f32, y as f32,
        )));
    }
}

/// Снимает текущую геометрию окна в настройки (при выходе). Всё хранится
/// в логических пикселях — не зависит от масштаба DPI.
pub(crate) fn collect_window_settings(ui: &AppWindow, s: &mut AppSettings) {
    let win = ui.window();
    let sf = win.scale_factor().max(1.0);
    let size = win.size();
    s.win_w = Some(size.width as f32 / sf);
    s.win_h = Some(size.height as f32 / sf);
    s.maximized = win.is_maximized();
    if !s.maximized {
        // у развернутого окна Windows сообщает мусорную позицию — не сохраняем;
        // API возвращает физические пиксели — приводим к логическим
        let p = win.position();
        s.win_x = Some((p.x as f32 / sf).round() as i32);
        s.win_y = Some((p.y as f32 / sf).round() as i32);
    }
}

/// Снимок состояния до мутации + синхронизация кнопок отмены/повтора.
pub(crate) fn commit_history(ui: &AppWindow, st: &mut UiState, snapshot: Database) {
    st.domain.commit(snapshot);
    sync_history_ui(ui, st);
}

/// Правка одного поля со «склейкой» последовательных правок — логика в ядре
/// (`DomainState::edit_coalesced`). `scope` — id лекарства, у которого правится
/// поле (`GLOBAL_SCOPE` для правок вне лекарств): правки одного поля у разных
/// лекарств не склеиваются в один шаг отмены. Снимок «до правки» снимает ядро —
/// и только когда правка начинает новый шаг: набор текста по буквам не
/// клонирует базу впустую на каждое нажатие.
pub(crate) fn edit_coalesced(
    ui: &AppWindow,
    st: &mut UiState,
    kind: EditKind,
    scope: i32,
    f: impl FnOnce(&mut Database),
) {
    st.domain.edit_coalesced(kind, scope, f);
    sync_history_ui(ui, st);
}

pub(crate) fn sync_history_ui(ui: &AppWindow, st: &UiState) {
    ui.set_can_undo(st.domain.can_undo());
    ui.set_can_redo(st.domain.can_redo());
}

/// Стандартный хвост мутационного колбэка: снимок в историю → полный refresh
/// → сохранение на диск. `prev` — снимок `db`, снятый ДО мутации.
pub(crate) fn commit_refresh_persist(
    ui: &AppWindow,
    st: &mut UiState,
    prev: Database,
    reset_form: bool,
) {
    commit_history(ui, st, prev);
    refresh_all(ui, st, reset_form);
    persist(ui, st);
}

/// Шаг по истории (undo при `backwards`, иначе redo): подмену базы делает
/// ядро (`step_history`), UI сбрасывает выделение, перерисовывает и сохраняет.
pub(crate) fn history_step(ui: &AppWindow, st: &mut UiState, backwards: bool) {
    if !st.domain.step_history(backwards) {
        return;
    }
    // база заменена состоянием из истории — контрол порога синхронизируем с ней
    ui.set_warn_days(st.domain.db.warn_days);
    if st.selected().is_none() {
        st.selected_id = -1;
        ui.set_selected_id(-1);
    }
    clear_photo_cache(st); // заменена база целиком — консервативный сброс кэша
    reset_view_flags(ui);
    let had_form = pkg_form_has_input(ui);
    refresh_all(ui, st, true);
    sync_history_ui(ui, st);
    persist(ui, st);
    if had_form {
        // отмена сбросила форму — говорим об этом, а не молча теряем ввод
        set_status_info(
            ui,
            "Правка отменена — несохранённая форма упаковки очищена".to_string(),
        );
    }
}

/// Регистрирует автодополнение названия, календарь выбора даты и прикрепление
/// фото упаковки (F6).
pub(crate) fn register_misc_cbs(ui: &AppWindow, state: &Rc<RefCell<UiState>>) {
    // Автодополнение названия по уже заведённым лекарствам: клик по подсказке
    // и Enter в поле названия идут через одну точку применения (без with_state! —
    // заём состояния делает сама apply_suggestion, как и в фото-диалоге)
    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_suggestion_picked(move |text| {
            let ui = h.unwrap();
            apply_suggestion(&ui, &s, text.as_str());
        });
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_suggestion_accept(move || {
            let ui = h.unwrap();
            if let Some(first) = ui.get_suggestions().row_data(0) {
                apply_suggestion(&ui, &s, first.as_str());
            }
        });
    }

    // Календарь: открытие (месяц из текущего срока), листание, выбор дня
    with_state!(ui, state, on_cal_month_opened, |ui, mut st| {
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
    with_state!(ui, state, on_cal_prev_month, |ui, mut st| {
        st.cal_month -= 1;
        if st.cal_month == 0 {
            st.cal_month = 12;
            st.cal_year -= 1;
        }
        cal_rebuild(&ui, &st);
    });
    with_state!(ui, state, on_cal_next_month, |ui, mut st| {
        st.cal_month += 1;
        if st.cal_month > 12 {
            st.cal_month = 1;
            st.cal_year += 1;
        }
        cal_rebuild(&ui, &st);
    });
    with_state!(ui, state, on_cal_pick, |ui, st, ymd| {
        if let Ok(d) = NaiveDate::parse_from_str(ymd.as_str(), "%Y-%m-%d") {
            disarm_delete(&ui);
            ui.set_pkg_expiry(SharedString::from(fmt_date(d)));
            validate_pkg_form(&ui, &st);
        }
    });

    // Список покупок: окно + копирование в буфер обмена
    // Фото упаковки: выбор файла и копия в photos/
    {
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_pick_photo(move || {
            let ui = h.unwrap();
            run_photo_dialog(&ui, &s, None);
        });
        // Клик по плейсхолдеру «фото недоступно»: прикрепить файл к этой
        // упаковке, не трогая режим формы (создание/правка)
        let h = ui.as_weak();
        let s = state.clone();
        ui.on_pkg_replace_photo(move |pid| {
            let ui = h.unwrap();
            run_photo_dialog(&ui, &s, Some(pid));
        });
    }
}

/// Применяет выбранную подсказку автодополнения к названию лекарства. Одна
/// точка для клика по подсказке и для Enter в поле названия.
fn apply_suggestion(ui: &AppWindow, state: &Rc<RefCell<UiState>>, text: &str) {
    disarm_delete(ui);
    let mut st = state.borrow_mut();
    let prev = st.domain.db.clone();
    if let Some(m) = st.selected_mut() {
        m.name = text.to_string();
    }
    commit_history(ui, &mut st, prev);
    ui.set_detail_name(SharedString::from(text));
    ui.set_suggestions(ModelRc::default());
    let id = st.selected_id;
    refresh_med_row(ui, id, &st);
    persist(ui, &st);
}

/// Диалог выбора фото: для существующей упаковки (`force_pid` или режим правки
/// формы) файл копируется и прикрепляется сразу; в режиме создания — откладывается
/// до сохранения (`pending_photo`). Заём состояния не держится во время модального
/// диалога rfd: он прокачивает message loop, и сработавший таймер мог бы захватить
/// borrow() поверх borrow_mut() (паника BorrowMutError).
fn run_photo_dialog(ui: &AppWindow, state: &Rc<RefCell<UiState>>, force_pid: Option<i32>) {
    let edit_pid = force_pid.or_else(|| {
        let st = state.borrow();
        (st.pkg_edit_id >= 0).then_some(st.pkg_edit_id)
    });
    let Some(file) = rfd::FileDialog::new()
        .set_title("Выберите фото упаковки")
        .add_filter("Изображения", &["png", "jpg", "jpeg"])
        .pick_file()
    else {
        return;
    };
    match edit_pid {
        Some(pid) => attach_photo(ui, state, pid, &file),
        None => {
            // новая упаковка: прикрепим файл при сохранении
            let mut st = state.borrow_mut();
            st.pending_photo = Some(file);
            ui.set_pending_photo(true);
        }
    }
}

/// Прикрепляет выбранный файл к существующей упаковке: копия в photos/,
/// снимок в историю, обновление карточки и сохранение.
fn attach_photo(ui: &AppWindow, state: &Rc<RefCell<UiState>>, pid: i32, file: &std::path::Path) {
    let fname = match copy_photo_to_store(file, pid) {
        Ok(f) => f,
        Err(e) => {
            // сбой копирования не проглатывается молча (F5) — красная строка
            // с ошибкой до следующей успешной операции
            report_error(ui, e);
            return;
        }
    };
    let mut st = state.borrow_mut();
    st.photo_cache.borrow_mut().remove(&fname);
    // Мутация домена: снимок в историю ДО правки и сохранение на диск. Имя
    // файла уникально, прежний снимок не перезаписан — Ctrl+Z реально вернёт
    // старое фото (F3).
    let prev = st.domain.db.clone();
    let mut changed = false;
    if let Some(m) = st.selected_mut() {
        if let Some(p) = m.packages.iter_mut().find(|p| p.id == pid) {
            if p.photo != fname {
                p.photo = fname;
                changed = true;
            }
        }
    }
    if changed {
        commit_history(ui, &mut st, prev);
    }
    rebuild_packages(ui, &st);
    if changed {
        persist(ui, &st);
    }
    set_status_info(ui, "Фото упаковки обновлено".to_string());
}
