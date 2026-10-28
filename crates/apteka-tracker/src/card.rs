// Карточка лекарства: правки полей, фото упаковок, колбёки (F6).

use apteka_core::chrono::Local;
use apteka_core::state::{EditKind, GLOBAL_SCOPE};
use image::ImageDecoder;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

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

/// Максимальная сторона миниатюры фото упаковки. В карточке фото показывается
/// в 52×58 px (и в плейсхолдере), запас — на будущие крупные вьюверы.
const PHOTO_THUMB_MAX: u32 = 480;

/// Миниатюра фото: уменьшенное изображение и расширение для имени файла.
struct Thumbnail {
    image: image::DynamicImage,
    ext: &'static str,
}

/// Читает изображение и уменьшает до `PHOTO_THUMB_MAX` по большей стороне
/// (маленькие изображения не увеличиваются). EXIF-поворот применяется:
/// портретные снимки с телефона не должны ложиться на бок.
fn make_thumbnail(src: &std::path::Path) -> Result<Thumbnail, String> {
    let mut reader = image::ImageReader::open(src)
        .and_then(|r| r.with_guessed_format())
        .map_err(|e| format!("файл не читается: {e}"))?;
    // лимиты декодера: легальный PNG может объявлять огромные размеры
    // (декомпрессионная бомба) — без лимита это OOM-abort приложения
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(20000);
    limits.max_image_height = Some(20000);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let ext = match reader.format() {
        Some(image::ImageFormat::Png) => "png",
        _ => "jpg",
    };
    let mut decoder = reader
        .into_decoder()
        .map_err(|e| format!("изображение не декодируется: {e}"))?;
    let orientation = decoder.orientation().map_err(|e| e.to_string())?;
    let mut img = image::DynamicImage::from_decoder(decoder)
        .map_err(|e| format!("изображение не декодируется: {e}"))?;
    img.apply_orientation(orientation);
    // уменьшаем только крупные: маленькие изображения не растягиваются
    if img.width() > PHOTO_THUMB_MAX || img.height() > PHOTO_THUMB_MAX {
        img = img.thumbnail(PHOTO_THUMB_MAX, PHOTO_THUMB_MAX);
    }
    Ok(Thumbnail { image: img, ext })
}

/// Копирует фото упаковки в каталог `photos/` миниатюрой (≤480 px) под
/// уникальным именем. Оригинал не сохраняется: декод мегапиксельного снимка —
/// десятки МБ RAM и паузы при листании упаковок, а показывается картинка
/// в 52×58. Битое/неподдерживаемое изображение — честная ошибка, а не
/// молча неработающее фото. Возвращает имя файла либо текст ошибки (F5).
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
    std::fs::create_dir_all(dir).map_err(|e| format!("Не удалось создать папку photos: {e}"))?;
    let thumb = make_thumbnail(src)?;
    let fname = unique_photo_name(pid, thumb.ext);
    let dst = dir.join(&fname);
    let result = match thumb.ext {
        "png" => thumb.image.save_with_format(&dst, image::ImageFormat::Png),
        _ => thumb.image.save_with_format(&dst, image::ImageFormat::Jpeg),
    };
    result.map_err(|e| format!("Не удалось сохранить миниатюру фото: {e}"))?;
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
        // если в форме упаковки был несохранённый ввод — сброс сообщается,
        // а не происходит молча
        let had_form = pkg_form_has_input(&ui);
        st.selected_id = id;
        ui.set_selected_id(id);
        ui.set_suggestions(ModelRc::default());
        // кэш фото не сбрасываем: имена файлов уникальны и неизменяемы —
        // возврат к лекарству не перечитывает и не декодирует фото заново
        reset_view_flags(&ui);
        update_details(&ui, &mut st, true);
        if had_form {
            set_status_info(&ui, "Несохранённая форма упаковки очищена".to_string());
        }
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
        st.domain.db.medicines.push(Arc::new(Medicine {
            id,
            name: "Новое лекарство".to_string(),
            form: FORMS[0].to_string(),
            note: String::new(),
            tags: Vec::new(),
            packages: Vec::new(),
        }));
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
        if let Some(n) = &name {
            st.domain.db.journal_push("лекарство удалено", n, "");
        }
        st.selected_id = -1;
        ui.set_selected_id(-1);
        // кэш фото не сбрасываем: имена неизменяемы, сами файлы в сессии
        // не удаляются (сироты чистятся только при следующем старте)
        reset_view_flags(&ui);
        commit_refresh_persist(&ui, &mut st, prev, true);
        if let Some(n) = &name {
            // факт и способ отката — словами, а не только в журнале
            set_status_info(&ui, format!("Удалено: «{n}» — вернуть: Ctrl+Z"));
        }
    });

    // Взведено подтверждение удаления (клавиша Delete или кнопка): имя
    // лекарства знает только Rust — подсказка о втором нажатии идёт в статус
    with_state!(ui, state, on_delete_arm_hint, |ui, st| {
        if let Some(m) = st.selected() {
            ui.set_status_message(SharedString::from(format!(
                "Удалить «{}»? Нажмите Delete ещё раз (Esc — отмена)",
                m.name
            )));
        }
    });

    let timer = Rc::clone(save_timer);
    with_state!(ui, state, on_name_edited, |ui, mut st, text| {
        disarm_delete(&ui);
        let scope = st.selected_id;
        edit_coalesced(&ui, &mut st, EditKind::Name, scope, |db| {
            if let Some(m) = db.medicine_mut(scope) {
                m.name = text.to_string();
            }
        });
        // автодополнение: имена других лекарств с тем же началом
        let needle = text.trim().to_lowercase();
        let mut sugg: Vec<SharedString> = Vec::new();
        if !needle.is_empty() {
            for m in &st.domain.db.medicines {
                if m.id == st.selected_id {
                    continue;
                }
                // to_lowercase — один раз на лекарство, не дважды
                let lower = m.name.to_lowercase();
                if lower.starts_with(&needle) && lower != needle {
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
        let scope = st.selected_id;
        edit_coalesced(&ui, &mut st, EditKind::Note, scope, |db| {
            if let Some(m) = db.medicine_mut(scope) {
                m.note = text.to_string();
            }
        });
        st.domain.unsaved.set(true);
        timer.restart();
    });

    let timer = Rc::clone(save_timer);
    with_state!(ui, state, on_tags_edited, |ui, mut st, text| {
        disarm_delete(&ui);
        let scope = st.selected_id;
        edit_coalesced(&ui, &mut st, EditKind::Tags, scope, |db| {
            if let Some(m) = db.medicine_mut(scope) {
                m.tags = parse_tags(&text);
            }
        });
        // категории входят в поиск и в фильтр «Категории»: выпадающий фильтр
        // и список обновляются сразу, а не до ближайшего refresh_all.
        // Порядок: сначала фильтр (может сброситься вместе с исчезнувшей
        // категорией), затем список — уже с его итоговым состоянием
        refresh_tag_options(&ui, &mut st);
        rebuild_list(&ui, &st);
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
        let scope = st.selected_id;
        edit_coalesced(&ui, &mut st, EditKind::Form, scope, |db| {
            if let Some(m) = db.medicine_mut(scope) {
                m.form = form;
            }
        });
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

    // Порог «Истекает»: правка склеивается в один шаг истории, а сохранение
    // дебаунсится общим save_timer — иначе каждая стрелка SpinBox и каждая
    // цифра давали полный fsync файла (на флешке — заметно).
    let timer = Rc::clone(save_timer);
    with_state!(ui, state, on_warn_days_changed, |ui, mut st, value| {
        disarm_delete(&ui);
        edit_coalesced(&ui, &mut st, EditKind::WarnDays, GLOBAL_SCOPE, |db| {
            db.warn_days = value;
        });
        refresh_all(&ui, &mut st, false);
        st.domain.unsaved.set(true);
        timer.restart();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_photo_makes_thumbnail_keeps_previous_file_for_undo() {
        // каждое прикрепление — миниатюра в новый файл: Ctrl+Z реально возвращает
        // прежний снимок (F3), поэтому имена не должны совпадать
        let dir = std::env::temp_dir().join(format!("apteka-card-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        // «снимок с телефона»: 3000×2000, не влезает в миниатюру целиком
        let big = image::RgbImage::from_fn(3000, 2000, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        });
        let src = dir.join("src.png");
        image::DynamicImage::ImageRgb8(big).save(&src).unwrap();
        // имена уникальны даже в одну миллисекунду — без пауз
        assert_ne!(unique_photo_name(7, "png"), unique_photo_name(7, "png"));
        let f1 = copy_photo_to_store_in(&dir, &src, 7).unwrap();
        // второе «фото» отличается содержимым
        let big2 =
            image::RgbImage::from_fn(3000, 2000, |x, _y| image::Rgb([(x % 256) as u8, 200, 90]));
        image::DynamicImage::ImageRgb8(big2).save(&src).unwrap();
        let f2 = copy_photo_to_store_in(&dir, &src, 7).unwrap();
        assert_ne!(f1, f2, "каждое сохранение — новое имя файла");
        assert_ne!(
            std::fs::read(dir.join(&f1)).unwrap(),
            std::fs::read(dir.join(&f2)).unwrap(),
            "прежнее фото не перезаписано — иначе Ctrl+Z не вернёт его"
        );
        // обе копии — миниатюры: большая сторона не превышает лимит
        for f in [&f1, &f2] {
            let img = image::open(dir.join(f)).unwrap();
            assert!(img.width() <= PHOTO_THUMB_MAX && img.height() <= PHOTO_THUMB_MAX);
        }
        // маленькое исходное изображение не растягивается
        let tiny_src = dir.join("tiny.png");
        image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(64, 40, |x, y| {
            image::Rgb([x as u8, y as u8, 7])
        }))
        .save(&tiny_src)
        .unwrap();
        let f3 = copy_photo_to_store_in(&dir, &tiny_src, 8).unwrap();
        let tiny = image::open(dir.join(&f3)).unwrap();
        assert_eq!((tiny.width(), tiny.height()), (64, 40), "апскейла нет");
        // не-изображение — честная ошибка, а не молча битое фото
        let bad = dir.join("bad.png");
        std::fs::write(&bad, "это не изображение".as_bytes()).unwrap();
        assert!(copy_photo_to_store_in(&dir, &bad, 9).is_err());
        // декомпрессионная бомба: настоящее изображение шире лимита — отказ
        // декодера, а не выделение гигабайт
        let wide_src = dir.join("wide.png");
        let wide = image::RgbImage::from_fn(20001, 3, |x, _y| image::Rgb([(x % 256) as u8, 1, 2]));
        image::DynamicImage::ImageRgb8(wide)
            .save(&wide_src)
            .unwrap();
        assert!(
            make_thumbnail(&wide_src).is_err(),
            "изображение за лимитами декодера — честная ошибка"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn thumbnail_honors_limits_without_huge_alloc() {
        // отдельная проверка самого make_thumbnail: PNG с шириной за лимитом
        // декодируется с ошибкой лимита, а не попыткой выделить память
        let dir = std::env::temp_dir().join(format!("apteka-thumb-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("tall.png");
        let tall = image::RgbImage::from_fn(3, 20001, |_x, y| image::Rgb([1, 2, (y % 256) as u8]));
        image::DynamicImage::ImageRgb8(tall).save(&src).unwrap();
        assert!(make_thumbnail(&src).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_placeholder_is_reused_not_duplicated() {
        // повторный клик «+ Добавить» находит нетронутое «Новое лекарство»,
        // а не создаёт второе
        let mut db = Database::default();
        assert_eq!(empty_placeholder_id(&db), None);
        let id = db.new_med_id();
        db.medicines.push(Arc::new(Medicine {
            id,
            name: "Новое лекарство".to_string(),
            form: FORMS[0].to_string(),
            note: String::new(),
            tags: Vec::new(),
            packages: Vec::new(),
        }));
        assert_eq!(empty_placeholder_id(&db), Some(id));
        // заполненная запись с тем же именем — уже не плейсхолдер
        db.medicine_mut(id).unwrap().note = "реальная заметка".to_string();
        assert_eq!(empty_placeholder_id(&db), None);
    }
}
