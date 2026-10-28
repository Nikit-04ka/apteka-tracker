// Форма упаковки: валидация, таблица упаковок, колбёки (F6).

use apteka_core::chrono::Months;
use apteka_core::csv;
use apteka_core::input::{parse_package_input, PackageForm};
use slint::{ComponentHandle, SharedString};
use std::cell::RefCell;
use std::rc::Rc;

use crate::card::*;
use crate::misc::*;
use crate::state::*;
use crate::ui_sync::*;
use crate::with_state;
use crate::*;

/// Пересобирает таблицу упаковок выбранного лекарства (списанные — по флагу).
pub(crate) fn rebuild_packages(ui: &AppWindow, st: &UiState) {
    let Some(m) = st.selected() else {
        sync_model(
            || ui.get_packages(),
            |model| ui.set_packages(model),
            Vec::new(),
        );
        ui.set_packages_header(SharedString::from("Упаковки"));
        return;
    };
    let t = today();
    let warn = st.domain.db.warn_days as i64;
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
            let (photo, photo_missing) = if p.photo.is_empty() {
                (slint::Image::default(), false)
            } else {
                // кэш: фото декодируется один раз; неудачные загрузки не
                // кэшируются — файл может появиться позже
                let mut cache = st.photo_cache.borrow_mut();
                match cache.get(&p.photo) {
                    Some(img) => (img.clone(), false),
                    None => {
                        match slint::Image::load_from_path(&storage::photos_dir().join(&p.photo)) {
                            Ok(img) => {
                                // предохранитель RAM: за долгую сессию просмотренных
                                // фото может набраться много — сбрасываем целиком
                                // (декод дешёвый, миниатюра читается с диска заново)
                                if cache.len() >= PHOTO_CACHE_CAP {
                                    cache.clear();
                                }
                                cache.insert(p.photo.clone(), img.clone());
                                (img, false)
                            }
                            Err(_) => (slint::Image::default(), true),
                        }
                    }
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
                photo_missing,
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
    sync_model(|| ui.get_packages(), |model| ui.set_packages(model), rows);
}

/// Краткое описание упаковки для журнала: срок — реальный (с учётом вскрытия
/// и периода после вскрытия), месяцный формат — как в списке покупок.
pub(crate) fn pkg_desc(p: &Package) -> String {
    let eff = p.effective_expiry();
    format!(
        "срок {}",
        fmt_date_display(eff, p.month_only && eff == p.expiry)
    )
}

pub(crate) fn reset_pkg_form(ui: &AppWindow, st: &mut UiState) {
    st.pkg_edit_id = -1;
    st.pending_photo = None;
    ui.set_pending_photo(false);
    ui.set_pkg_editing(false);
    ui.set_pkg_expiry(SharedString::new());
    ui.set_pkg_quantity(SharedString::new());
    ui.set_pkg_note(SharedString::new());
    ui.set_pkg_price(SharedString::new());
    ui.set_pkg_period(SharedString::new());
    ui.set_pkg_purchase_date(SharedString::new());
    ui.set_pkg_validation(SharedString::from(
        "Формат: ГГГГ-ММ-ДД, ДД.ММ.ГГГГ или ММ.ГГГГ (годен до конца месяца)",
    ));
    ui.set_pkg_validation_color(color_grey());
    ui.set_pkg_save_enabled(false);
}

/// Живая валидация формы упаковки: срок, период после вскрытия, цена.
pub(crate) fn validate_pkg_form(ui: &AppWindow, st: &UiState) {
    match parse_package_input(
        ui.get_pkg_expiry().as_str(),
        ui.get_pkg_period().as_str(),
        ui.get_pkg_price().as_str(),
    ) {
        Err(msg) => {
            let neutral = msg.starts_with("Формат:");
            ui.set_pkg_validation(SharedString::from(msg));
            ui.set_pkg_validation_color(if neutral { color_grey() } else { color_red() });
            ui.set_pkg_save_enabled(false);
        }
        Ok(input) => {
            // дата покупки необязательна, но заполненная должна читаться
            let purchased = ui.get_pkg_purchase_date().trim().to_string();
            if !purchased.is_empty() && parse_date_input(&purchased).is_none() {
                ui.set_pkg_validation(SharedString::from(
                    "Дата покупки: ГГГГ-ММ-ДД или ДД.ММ.ГГГГ (можно оставить пустым)",
                ));
                ui.set_pkg_validation_color(color_red());
                ui.set_pkg_save_enabled(false);
                return;
            }
            let days = days_left(input.expiry, today());
            let s = status_for(days, st.domain.db.warn_days as i64);
            let mut msg = if input.month_only {
                format!(
                    "месяц: годен до {} · {}",
                    fmt_date(input.expiry),
                    status_text(s, days)
                )
            } else {
                status_text(s, days)
            };
            if input.open_period_days.is_some() {
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
    }
}

// ---------- Служебные ----------

/// Регистрирует колбэки формы упаковки и её жизненного цикла (F6).
pub(crate) fn register_package_cbs(ui: &AppWindow, state: &Rc<RefCell<UiState>>) {
    with_state!(ui, state, on_pkg_start_edit, |ui, mut st, pid| {
        disarm_delete(&ui);
        let found = st
            .selected()
            .and_then(|m| m.packages.iter().find(|p| p.id == pid))
            .map(|p| {
                (
                    p.expiry,
                    p.month_only,
                    p.quantity.clone(),
                    p.note.clone(),
                    p.price,
                    p.open_period_days,
                    p.purchase_date,
                )
            });
        if let Some((expiry, month_only, quantity, note, price, period, purchased)) = found {
            // выбранное фото из незавершённого создания не должно «переехать»
            // на редактируемую упаковку
            st.pending_photo = None;
            ui.set_pending_photo(false);
            // месяц без дня показываем как «ММ.ГГГГ» — тогда parse сохранит
            // month_only и правка упаковки не превратит «01.2028» в «31.01.2028»
            ui.set_pkg_expiry(SharedString::from(fmt_date_display(expiry, month_only)));
            ui.set_pkg_quantity(SharedString::from(quantity));
            ui.set_pkg_note(SharedString::from(note));
            ui.set_pkg_price(price.map(|v| format!("{v:.2}")).unwrap_or_default().into());
            ui.set_pkg_period(period.map(|v| v.to_string()).unwrap_or_default().into());
            ui.set_pkg_purchase_date(
                purchased
                    .map(|d| SharedString::from(fmt_date(d)))
                    .unwrap_or_default(),
            );
            st.pkg_edit_id = pid;
            ui.set_pkg_editing(true);
            validate_pkg_form(&ui, &st);
        }
    });

    with_state!(ui, state, on_pkg_cancel_edit, |ui, mut st| {
        disarm_delete(&ui);
        reset_pkg_form(&ui, &mut st);
    });

    with_state!(ui, state, on_pkg_expiry_edited, |ui, st, _text| {
        disarm_delete(&ui);
        validate_pkg_form(&ui, &st);
    });

    // Живая валидация остальных полей формы (цена, период, «Куплена»):
    // иначе кнопка сохранения устаревает и молча ничего не делает.
    with_state!(ui, state, on_pkg_form_edited, |ui, st| {
        disarm_delete(&ui);
        validate_pkg_form(&ui, &st);
    });

    with_state!(ui, state, on_pkg_quick, |ui, st, months| {
        disarm_delete(&ui);
        if let Some(d) = today().checked_add_months(Months::new(months.max(0) as u32)) {
            ui.set_pkg_expiry(SharedString::from(fmt_date(d)));
            validate_pkg_form(&ui, &st);
        }
    });

    with_state!(ui, state, on_pkg_clone, |ui, mut st, pid| {
        disarm_delete(&ui);
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
        let Some((quantity, note, price, period)) = found else {
            return;
        };
        let Some(expiry) = today().checked_add_months(Months::new(12)) else {
            return;
        };
        ui.set_pkg_expiry(SharedString::from(fmt_date(expiry)));
        ui.set_pkg_quantity(SharedString::from(quantity));
        ui.set_pkg_note(SharedString::from(note));
        ui.set_pkg_price(price.map(|v| format!("{v:.2}")).unwrap_or_default().into());
        ui.set_pkg_period(period.map(|v| v.to_string()).unwrap_or_default().into());
        // «Куплена» не копируется: дата покупки относится к исходной пачке и
        // не должна утекать в новый пакет (она искажает отчёт «Покупки по годам»)
        ui.set_pkg_purchase_date(SharedString::new());
        // фото, выбранное для прежней формы, не переезжает на клон —
        // как и в on_pkg_start_edit
        st.pending_photo = None;
        ui.set_pending_photo(false);
        st.pkg_edit_id = -1;
        ui.set_pkg_editing(false);
        validate_pkg_form(&ui, &st);
    });

    with_state!(ui, state, on_pkg_save, |ui, mut st| {
        disarm_delete(&ui);
        // Разобранная форма — единая точка «форма ↔ Package»: поля не
        // присваиваются поштучно. Кнопка сохранения активна только после живой
        // валидации, так что Err здесь почти недостижим.
        let form = match PackageForm::parse(
            &ui.get_pkg_expiry(),
            &ui.get_pkg_quantity(),
            &ui.get_pkg_note(),
            &ui.get_pkg_price(),
            &ui.get_pkg_period(),
            &ui.get_pkg_purchase_date(),
        ) {
            Ok(f) => f,
            Err(_) => {
                // кнопка могла устареть — показываем причину вместо тихого отказа
                validate_pkg_form(&ui, &st);
                return;
            }
        };
        let prev = st.domain.db.clone();
        let sel = st.selected_id;
        let edit_id = st.pkg_edit_id;
        let mut journal_note: Option<(String, String)> = None;
        let mut photo_error: Option<String> = None;
        match edit_id {
            -1 => {
                let pid = st.domain.db.new_pkg_id();
                // отложенное фото выбранной в режиме создания упаковки
                let mut attached_photo = String::new();
                if let Some(src) = st.pending_photo.take() {
                    ui.set_pending_photo(false);
                    match copy_photo_to_store(&src, pid) {
                        Ok(fname) => {
                            st.photo_cache.borrow_mut().remove(&fname);
                            attached_photo = fname;
                        }
                        Err(e) => photo_error = Some(e),
                    }
                }
                let mut pkg = form.into_package(pid);
                pkg.photo = attached_photo;
                let desc = pkg_desc(&pkg);
                if let Some(m) = st.domain.db.medicine_mut(sel) {
                    journal_note = Some((m.name.clone(), desc));
                    m.packages.push(pkg);
                }
            }
            pid => {
                if let Some(m) = st.domain.db.medicine_mut(sel) {
                    if let Some(p) = m.packages.iter_mut().find(|p| p.id == pid) {
                        // правка без изменений — не мутация: не создаём фиктивный
                        // шаг истории и событие журнала (как и on_form_selected)
                        if !form.matches(p) {
                            form.apply_to(p);
                            journal_note = Some((m.name.clone(), pkg_desc(p)));
                        }
                    }
                }
            }
        }
        if let Some((name, pdesc)) = &journal_note {
            let kind = if edit_id >= 0 {
                "изменена упаковка"
            } else {
                "добавлена упаковка"
            };
            st.domain.db.journal_push(kind, name, pdesc);
        }
        reset_pkg_form(&ui, &mut st);
        if journal_note.is_some() {
            commit_refresh_persist(&ui, &mut st, prev, false);
        } else {
            set_status_info(&ui, "Изменений нет — упаковка осталась прежней".to_string());
        }
        if let Some(e) = photo_error {
            // упаковка сохранена, но фото не прикрепилось — честно сообщаем
            // и говорим, как прикрепить позже
            report_error(
                &ui,
                format!(
                    "Упаковка сохранена, но {e}. Прикрепить фото позже: \
                     кнопка «Изменить» → «Фото…»"
                ),
            );
        }
    });

    with_state!(ui, state, on_pkg_delete, |ui, mut st, pid| {
        disarm_delete(&ui);
        let prev = st.domain.db.clone();
        let mut jnote: Option<(String, String)> = None;
        if let Some(m) = st.selected_mut() {
            if let Some(p) = m.packages.iter().find(|p| p.id == pid) {
                jnote = Some((m.name.clone(), pkg_desc(p)));
            }
            m.packages.retain(|p| p.id != pid);
        }
        if let Some((n, d)) = &jnote {
            st.domain.db.journal_push("упаковка удалена", n, d);
        }
        // удалили упаковку, которая была в редактировании, — форма возвращается
        // в режим «новая упаковка»: иначе кнопка «Сохранить» создала бы дубль
        let was_editing = st.pkg_edit_id == pid;
        if was_editing {
            st.pkg_edit_id = -1;
        }
        commit_refresh_persist(&ui, &mut st, prev, was_editing);
        if let Some((n, d)) = jnote {
            // подтверждение с путём возврата: удаление двухшаговое, но факт
            // и способ отката стоит назвать словами
            set_status_info(&ui, format!("Удалено: {n} ({d}) — вернуть: Ctrl+Z"));
        }
    });

    with_state!(ui, state, on_export_csv, |ui, st| {
        match csv::export_csv(&st.domain.db) {
            Ok(exported) => ui.set_status_message(SharedString::from(format!(
                "Экспортировано: {}",
                exported.display()
            ))),
            Err(e) => report_error(
                &ui,
                format!(
                    "Ошибка экспорта: {e}. Проверьте, что файл не открыт в Excel, \
                     и что папка программы доступна для записи"
                ),
            ),
        }
    });

    // ===== Жизненный цикл упаковки: открытие, списание, возврат =====

    with_state!(ui, state, on_pkg_open, |ui, mut st, pid| {
        disarm_delete(&ui);
        let prev = st.domain.db.clone();
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
        if let Some((n, d)) = jnote {
            st.domain.db.journal_push("упаковка открыта", &n, &d);
            commit_refresh_persist(&ui, &mut st, prev, false);
            // обратной кнопки «не открыта» нет (кнопки карточки и так на пределе
            // ширины окна) — rollback через Ctrl+Z, говорим о нём сразу
            set_status_info(
                &ui,
                "Упаковка отмечена открытой сегодня — отменить: Ctrl+Z".to_string(),
            );
        }
    });

    with_state!(ui, state, on_pkg_dispose, |ui, mut st, pid, reason| {
        disarm_delete(&ui);
        let prev = st.domain.db.clone();
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
        if let Some((n, d)) = jnote {
            st.domain.db.journal_push("упаковка списана", &n, &d);
            commit_refresh_persist(&ui, &mut st, prev, false);
        }
    });

    with_state!(ui, state, on_pkg_restore, |ui, mut st, pid| {
        disarm_delete(&ui);
        let prev = st.domain.db.clone();
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
        if let Some((n, d)) = jnote {
            st.domain
                .db
                .journal_push("упаковка возвращена в работу", &n, &d);
            commit_refresh_persist(&ui, &mut st, prev, false);
        }
    });

    with_state!(ui, state, on_toggle_show_disposed, |ui, st| {
        ui.set_show_disposed(!ui.get_show_disposed());
        rebuild_packages(&ui, &st);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use apteka_core::chrono::NaiveDate;

    #[test]
    fn pkg_desc_shows_real_expiry_of_opened_package() {
        // журнал описывает упаковку реальным сроком: вскрытая с периодом —
        // датой «вскрытие + период», а не заводским «до месяца»
        let mut p = Package {
            id: 1,
            expiry: NaiveDate::from_ymd_opt(2028, 1, 31).unwrap(),
            month_only: true,
            quantity: String::new(),
            note: String::new(),
            price: None,
            purchase_date: None,
            opened: None,
            open_period_days: None,
            disposed: None,
            photo: String::new(),
        };
        assert_eq!(pkg_desc(&p), "срок 01.2028");
        p.opened = Some(NaiveDate::from_ymd_opt(2026, 11, 1).unwrap());
        p.open_period_days = Some(30);
        assert_eq!(pkg_desc(&p), "срок 01.12.2026");
    }
}
