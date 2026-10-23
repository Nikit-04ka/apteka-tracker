// Карточка лекарства: правки полей, фото упаковок, колбёки (F6).

use apteka_core::chrono::Local;
use apteka_core::model::*;
use apteka_core::state::{EditKind, GLOBAL_SCOPE};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;

use crate::misc::*;
use crate::state::*;
use crate::ui_sync::*;
use crate::with_state;
use crate::*;

/// Уникальное имя файла фото упаковки: `pkg_<id>-<метка>-<счётчик>.<ext>`.
/// Метка времени плюс счётчик процесса делают имя новым при каждом
/// прикреплении (даже в одну миллисекунду): замена фото не перезаписывает
/// прежний файл, поэтому Ctrl+Z реально возвращает старый снимок (F3).
pub(crate) fn unique_photo_name(pid: i32, ext: &str) -> String {
    static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let stamp = Local::now().format("%Y%m%d-%H%M%S-%3f");
    format!("pkg_{pid}-{stamp}-{n:03}.{ext}")
}

/// Копирует выбранный файл в каталог `photos/` под уникальным именем.
/// Возвращает имя файла либо текст ошибки для статусной строки — сбой
/// копирования не проглатывается молча (F5).
pub(crate) fn copy_photo_to_store(src: &std::path::Path, pid: i32) -> Result<String, String> {
    copy_photo_to_store_in(&storage::photos_dir(), src, pid)
}

/// То же, но в указанный каталог — так сценарий «каждое сохранение — новый
/// файл, прежний не перезаписывается» покрыт тестом (F3).
pub(crate) fn copy_photo_to_store_in(
    dir: &std::path::Path,
    src: &std::path::Path,
    pid: i32,
) -> Result<String, String> {
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .unwrap_or_else(|| "png".to_string());
    std::fs::create_dir_all(dir).map_err(|e| format!("Не удалось создать папку photos: {e}"))?;
    let fname = unique_photo_name(pid, &ext);
    std::fs::copy(src, dir.join(&fname))
        .map_err(|e| format!("Не удалось скопировать фото: {e}"))?;
    Ok(fname)
}

/// Нетронутое «Новое лекарство» от прошлых кликов «+ Добавить»: без упаковок,
/// заметок и категорий. Повторный клик открывает его, а не плодит пустышки.
pub(crate) fn empty_placeholder_id(db: &Database) -> Option<i32> {
    db.medicines
        .iter()
        .find(|m| {
            m.name == "Новое лекарство"
                && m.form == FORMS[0]
                && m.note.is_empty()
                && m.tags.is_empty()
                && m.packages.is_empty()
        })
        .map(|m| m.id)
}

/// Регистрирует колбэки карточки лекарства: выбор, добавление/удаление,
/// правки полей (имя/заметки/категории/форма), фильтры списка и порог (F6).
pub(crate) fn register_card_cbs(
    ui: &AppWindow,
    state: &Rc<RefCell<UiState>>,
    save_timer: &Rc<slint::Timer>,
) {
    with_state!(ui, state, on_select_medicine, |ui, mut st, id| {
        st.selected_id = id;
        ui.set_selected_id(id);
        ui.set_suggestions(ModelRc::default());
        clear_photo_cache(&st);
        reset_view_flags(&ui);
        update_details(&ui, &mut st, true);
    });

    with_state!(ui, state, on_add_medicine, |ui, mut st| {
        // повторные клики не плодят пустышки: пока в базе есть нетронутое
        // «Новое лекарство» — открываем его вместо создания нового
        if let Some(id) = empty_placeholder_id(&st.domain.db) {
            st.selected_id = id;
            st.search.clear();
            st.filter_idx = 0;
            st.tag_filter = None;
            st.pending_photo = None;
            ui.set_pending_photo(false);
            ui.set_search_text(SharedString::new());
            ui.set_filter_index(0);
            ui.set_selected_id(id);
            reset_view_flags(&ui);
            refresh_all(&ui, &mut st, true);
            ui.set_status_message(SharedString::from(
                "Уже есть пустое «Новое лекарство» — открываем его",
            ));
            return;
        }
        let prev = st.domain.db.clone();
        let id = st.domain.db.new_med_id();
        st.domain.db.medicines.push(Medicine {
            id,
            name: "Новое лекарство".to_string(),
            form: FORMS[0].to_string(),
            note: String::new(),
            tags: Vec::new(),
            packages: Vec::new(),
        });
        st.domain
            .db
            .journal_push("лекарство добавлено", "Новое лекарство", "");
        st.selected_id = id;
        st.search.clear();
        st.filter_idx = 0;
        st.tag_filter = None;
        st.pending_photo = None;
        ui.set_pending_photo(false);
        ui.set_search_text(SharedString::new());
        ui.set_filter_index(0);
        ui.set_selected_id(id);
        reset_view_flags(&ui);
        commit_refresh_persist(&ui, &mut st, prev, true);
    });

    with_state!(ui, state, on_delete_medicine, |ui, mut st| {
        let sel = st.selected_id;
        let prev = st.domain.db.clone();
        let name = st.selected().map(|m| m.name.clone());
        st.domain.db.medicines.retain(|m| m.id != sel);
        if let Some(n) = name {
            st.domain.db.journal_push("лекарство удалено", &n, "");
        }
        st.selected_id = -1;
        ui.set_selected_id(-1);
        clear_photo_cache(&st);
        reset_view_flags(&ui);
        commit_refresh_persist(&ui, &mut st, prev, true);
    });

    let timer = Rc::clone(save_timer);
    with_state!(ui, state, on_name_edited, |ui, mut st, text| {
        disarm_delete(&ui);
        let prev = st.domain.db.clone();
        if let Some(m) = st.selected_mut() {
            m.name = text.to_string();
        }
        let scope = st.selected_id;
        commit_history_coalesced(&ui, &mut st, prev, EditKind::Name, scope);
        // автодополнение: имена других лекарств с тем же началом
        let needle = text.trim().to_lowercase();
        let mut sugg: Vec<SharedString> = Vec::new();
        if !needle.is_empty() {
            for m in &st.domain.db.medicines {
                if m.id == st.selected_id {
                    continue;
                }
                if m.name.to_lowercase().starts_with(&needle) && !m.name.to_lowercase().eq(&needle)
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
        refresh_med_row(&ui, id, &st);
        st.domain.unsaved.set(true);
        timer.restart();
    });

    let timer = Rc::clone(save_timer);
    with_state!(ui, state, on_note_edited, |ui, mut st, text| {
        disarm_delete(&ui);
        let prev = st.domain.db.clone();
        if let Some(m) = st.selected_mut() {
            m.note = text.to_string();
        }
        let scope = st.selected_id;
        commit_history_coalesced(&ui, &mut st, prev, EditKind::Note, scope);
        st.domain.unsaved.set(true);
        timer.restart();
    });

    let timer = Rc::clone(save_timer);
    with_state!(ui, state, on_tags_edited, |ui, mut st, text| {
        disarm_delete(&ui);
        let prev = st.domain.db.clone();
        if let Some(m) = st.selected_mut() {
            m.tags = parse_tags(&text);
        }
        let scope = st.selected_id;
        commit_history_coalesced(&ui, &mut st, prev, EditKind::Tags, scope);
        st.domain.unsaved.set(true);
        timer.restart();
    });

    with_state!(ui, state, on_form_selected, |ui, mut st, idx| {
        disarm_delete(&ui);
        // последний индекс модели — «родная» форма лекарства вне справочника
        let form = if (idx as usize) < FORMS.len() {
            FORMS[idx as usize].to_string()
        } else {
            match ui.get_form_options().row_data(idx as usize) {
                Some(s) => s.to_string(),
                None => return,
            }
        };
        if st.selected().map(|m| m.form.as_str()) == Some(form.as_str()) {
            return; // выбрано то, что и было — шаг истории не нужен
        }
        let prev = st.domain.db.clone();
        if let Some(m) = st.selected_mut() {
            m.form = form;
        }
        let scope = st.selected_id;
        commit_history_coalesced(&ui, &mut st, prev, EditKind::Form, scope);
        let id = st.selected_id;
        refresh_med_row(&ui, id, &st);
        persist(&ui, &st);
    });

    with_state!(ui, state, on_search_edited, |ui, mut st, text| {
        st.search = text.to_string();
        rebuild_list(&ui, &st);
    });

    with_state!(ui, state, on_filter_selected, |ui, mut st, idx| {
        st.filter_idx = idx as usize;
        rebuild_list(&ui, &st);
    });

    with_state!(ui, state, on_tag_filter_selected, |ui, mut st, idx| {
        let model = ui.get_tag_options();
        st.tag_filter = model
            .row_data(idx as usize)
            .map(|s| s.to_string())
            .filter(|s| s != "Все категории");
        rebuild_list(&ui, &st);
    });

    with_state!(ui, state, on_sort_selected, |ui, mut st, idx| {
        st.sort_idx = idx as usize;
        rebuild_list(&ui, &st);
    });

    with_state!(ui, state, on_warn_days_changed, |ui, mut st, value| {
        let prev = st.domain.db.clone();
        st.domain.db.warn_days = value;
        commit_history_coalesced(&ui, &mut st, prev, EditKind::WarnDays, GLOBAL_SCOPE);
        refresh_all(&ui, &mut st, false);
        persist(&ui, &st);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_photo_keeps_previous_file_for_undo() {
        // каждое прикрепление — новый файл: Ctrl+Z реально возвращает
        // прежний снимок (F3), поэтому имена не должны совпадать
        let dir = std::env::temp_dir().join(format!("apteka-card-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("src.png");
        std::fs::write(&src, b"v1").unwrap();
        // имена уникальны даже в одну миллисекунду — без пауз
        assert_ne!(unique_photo_name(7, "png"), unique_photo_name(7, "png"));
        let f1 = copy_photo_to_store_in(&dir, &src, 7).unwrap();
        std::fs::write(&src, b"v2").unwrap();
        let f2 = copy_photo_to_store_in(&dir, &src, 7).unwrap();
        assert_ne!(f1, f2, "каждое сохранение — новое имя файла");
        assert_eq!(
            std::fs::read(dir.join(&f1)).unwrap(),
            b"v1".to_vec(),
            "прежнее фото не перезаписано — иначе Ctrl+Z не вернёт его"
        );
        assert_eq!(std::fs::read(dir.join(&f2)).unwrap(), b"v2".to_vec());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_placeholder_is_reused_not_duplicated() {
        // повторный клик «+ Добавить» находит нетронутое «Новое лекарство»,
        // а не создаёт второе
        let mut db = Database::default();
        assert_eq!(empty_placeholder_id(&db), None);
        let id = db.new_med_id();
        db.medicines.push(Medicine {
            id,
            name: "Новое лекарство".to_string(),
            form: FORMS[0].to_string(),
            note: String::new(),
            tags: Vec::new(),
            packages: Vec::new(),
        });
        assert_eq!(empty_placeholder_id(&db), Some(id));
        // заполненная запись с тем же именем — уже не плейсхолдер
        db.medicines[0].note = "реальная заметка".to_string();
        assert_eq!(empty_placeholder_id(&db), None);
    }
}
