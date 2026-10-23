// Служебное: цвета, флаги вида, persist, история отмены, настройки окна.

use apteka_core::chrono::{Datelike, Local, NaiveDate};
use apteka_core::model::*;
use apteka_core::settings::AppSettings;
use apteka_core::state::EditKind;
use slint::private_unstable_api::re_exports::ColorScheme;
use slint::{Color, ComponentHandle, ModelRc, SharedString};
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
    ui.set_show_disposed(false);
}

/// Сбрасывает взведённое подтверждение удаления лекарства (кнопка/клавиша
/// Delete), не трогая остальные флаги вида. Вызывается при действиях, отличных
/// от самого удаления (правки карточки), чтобы «Точно удалить?» не переживало
/// чужое действие и не удаляло лекарство с одного нажатия (F19).
pub(crate) fn disarm_delete(ui: &AppWindow) {
    ui.set_delete_armed(false);
    ui.set_delete_btn_text(SharedString::from("Удалить лекарство"));
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
            ui.set_save_error(true);
            ui.set_status_message(SharedString::from(format!(
                "ОШИБКА сохранения: {e} — проверьте, что диск и папка доступны для записи"
            )));
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

/// То же со «склейкой» последовательных правок одного поля — логика в ядре
/// (`DomainState::commit_coalesced`). `scope` — id лекарства, у которого правится
/// поле (`GLOBAL_SCOPE` для правок вне лекарств): правки одного поля у разных
/// лекарств не склеиваются в один шаг отмены.
pub(crate) fn commit_history_coalesced(
    ui: &AppWindow,
    st: &mut UiState,
    snapshot: Database,
    kind: EditKind,
    scope: i32,
) {
    st.domain.commit_coalesced(snapshot, kind, scope);
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
    clear_photo_cache(st);
    reset_view_flags(ui);
    refresh_all(ui, st, true);
    sync_history_ui(ui, st);
    persist(ui, st);
}

/// Регистрирует автодополнение названия, календарь выбора даты и прикрепление
/// фото упаковки (F6).
pub(crate) fn register_misc_cbs(ui: &AppWindow, state: &Rc<RefCell<UiState>>) {
    // Автодополнение названия по уже заведённым лекарствам
    with_state!(ui, state, on_suggestion_picked, |ui, mut st, text| {
        let prev = st.domain.db.clone();
        if let Some(m) = st.selected_mut() {
            m.name = text.to_string();
        }
        commit_history(&ui, &mut st, prev);
        ui.set_detail_name(text);
        ui.set_suggestions(ModelRc::default());
        let id = st.selected_id;
        refresh_med_row(&ui, id, &st);
        persist(&ui, &st);
    });

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
            // ВАЖНО: не держим заём состояния во время модального диалога rfd —
            // диалог прокачивает message loop, и сработавший таймер мог бы
            // захватить borrow() поверх borrow_mut() (паника BorrowMutError).
            let editing = {
                let st = s.borrow();
                (st.pkg_edit_id, st.pending_photo.is_some())
            };
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
                let fname = match copy_photo_to_store(&file, pid) {
                    Ok(f) => f,
                    Err(e) => {
                        ui.set_status_message(SharedString::from(e));
                        return;
                    }
                };
                let mut st = s.borrow_mut();
                st.photo_cache.borrow_mut().remove(&fname);
                // Мутация домена: снимок в историю ДО правки и сохранение на
                // диск. Имя файла уникально, прежний снимок не перезаписан —
                // Ctrl+Z реально вернёт старое фото (F3).
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
                    commit_history(&ui, &mut st, prev);
                }
                rebuild_packages(&ui, &st);
                if changed {
                    persist(&ui, &st);
                }
                ui.set_status_message(SharedString::from("Фото упаковки обновлено"));
            } else {
                // новая упаковка: прикрепим файл при сохранении
                let mut st = s.borrow_mut();
                st.pending_photo = Some(file);
                ui.set_pending_photo(true);
            }
        });
    }
}
