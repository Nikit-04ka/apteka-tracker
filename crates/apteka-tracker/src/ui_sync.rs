// Сборка строк и моделей UI из домена + обновления списков и статусов.

use apteka_core::input::{all_tags, cal_grid, move_selection_index, visible_medicines};
use apteka_core::model::*;
use slint::{Model, ModelRc, SharedString, VecModel};

use crate::misc::*;
use crate::packages::*;
use crate::state::*;
use crate::*;

pub(crate) const MONTHS_RU: [&str; 12] = [
    "Январь",
    "Февраль",
    "Март",
    "Апрель",
    "Май",
    "Июнь",
    "Июль",
    "Август",
    "Сентябрь",
    "Октябрь",
    "Ноябрь",
    "Декабрь",
];

pub(crate) fn build_med_row(m: &Medicine, info: &MedInfo) -> MedicineRow {
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

/// Обновляет модель списка на месте: существующие строки перезаписываются
/// только при изменении (`set_row_data` — делегаты не пересоздаются, скролл
/// не прыгает), новые дописываются, лишние усекаются с конца. Если модель
/// ещё не VecModel (первое заполнение после старта) — ставится новый VecModel.
pub(crate) fn sync_model<T, G, S>(get: G, set: S, rows: Vec<T>)
where
    T: Clone + PartialEq + 'static,
    G: FnOnce() -> ModelRc<T>,
    S: FnOnce(ModelRc<T>),
{
    let target = rows.len();
    let model = get();
    if let Some(vm) = model.as_any().downcast_ref::<VecModel<T>>() {
        let current = vm.row_count();
        for (i, row) in rows.into_iter().enumerate() {
            if i < current {
                if vm.row_data(i).as_ref() == Some(&row) {
                    continue;
                }
                vm.set_row_data(i, row);
            } else {
                vm.push(row);
            }
        }
        for i in (target..current).rev() {
            vm.remove(i);
        }
    } else {
        set(ModelRc::from(Rc::new(VecModel::from(rows))));
    }
}

/// Обновляет выпадающий список категорий: «Все категории» + теги базы.
/// Выбранная категория сохраняется, если ещё существует.
pub(crate) fn refresh_tag_options(ui: &AppWindow, st: &mut UiState) {
    let mut options = vec!["Все категории".to_string()];
    options.extend(all_tags(&st.domain.db));
    let idx = st
        .tag_filter
        .as_ref()
        .and_then(|t| options.iter().position(|o| o == t))
        .unwrap_or(0);
    st.tag_filter = (idx > 0).then(|| options[idx].clone());
    ui.set_tag_options(ModelRc::from(Rc::new(VecModel::from(
        options
            .into_iter()
            .map(SharedString::from)
            .collect::<Vec<_>>(),
    ))));
    ui.set_tag_filter_index(idx as i32);
}

/// Пересобирает список лекарств с учётом поиска, фильтра и сортировки
/// (логика — чистая функция ядра `visible_medicines`).
pub(crate) fn rebuild_list(ui: &AppWindow, st: &UiState) {
    let rows: Vec<MedicineRow> = visible_medicines(
        &st.domain.db,
        &st.search,
        st.tag_filter.as_deref(),
        st.filter_idx,
        st.sort_idx,
        today(),
    )
    .into_iter()
    .map(|(m, info)| build_med_row(m, &info))
    .collect();
    sync_model(|| ui.get_medicines(), |model| ui.set_medicines(model), rows);

    ui.set_list_empty_text(SharedString::from(
        if st.search.is_empty() && st.filter_idx == 0 {
            "Аптечка пуста.\nНажмите «+ Добавить лекарство»."
        } else {
            "Ничего не найдено.\nИзмените поиск или фильтр."
        },
    ));
}

/// Обновляет сводку по всем лекарствам (без учёта фильтров).
pub(crate) fn update_summary(ui: &AppWindow, st: &UiState) {
    let t = today();
    let warn = st.domain.db.warn_days as i64;
    let mut soon = 0usize;
    let mut expired = 0usize;
    let mut packages = 0usize;
    for m in &st.domain.db.medicines {
        packages += m.packages.len();
        match medicine_info(m, t, warn).status {
            STATUS_SOON => soon += 1,
            STATUS_EXPIRED => expired += 1,
            _ => {}
        }
    }
    ui.set_summary_total(SharedString::from(format!(
        "Лекарств: {} · Упаковок: {}",
        st.domain.db.medicines.len(),
        packages
    )));
    ui.set_summary_soon(SharedString::from(format!(
        "Истекают (≤{} дн.): {}",
        st.domain.db.warn_days, soon
    )));
    ui.set_summary_expired(SharedString::from(format!("Просрочено: {}", expired)));

    let cost: f64 = st
        .domain
        .db
        .medicines
        .iter()
        .flat_map(|m| m.packages.iter())
        .filter(|p| !p.is_disposed())
        .filter_map(|p| p.price)
        .sum();
    ui.set_summary_cost(SharedString::from(format!("Стоимость: {cost:.0} руб.")));
}

/// Кэш фото привязан к просматриваемым лекарствам; при смене выделения или
/// замене базы сбрасываем — декод заново по требованию.
pub(crate) fn clear_photo_cache(st: &UiState) {
    st.photo_cache.borrow_mut().clear();
}

/// Заполняет панель деталей; при `reset_form` сбрасывает форму упаковки.
pub(crate) fn update_details(ui: &AppWindow, st: &mut UiState, reset_form: bool) {
    update_details_readonly(ui, st);
    if reset_form {
        reset_pkg_form(ui, st);
    }
}

/// Заполнение деталей без побочных эффектов — пригодно для таймера.
pub(crate) fn update_details_readonly(ui: &AppWindow, st: &UiState) {
    match st.selected() {
        None => {
            ui.set_has_selection(false);
        }
        Some(m) => {
            ui.set_has_selection(true);
            // поля переписываем только при реальном изменении: иначе часовой
            // пересчёт статусов сбрасывает курсор в LineEdit посреди набора
            if ui.get_detail_name().as_str() != m.name {
                ui.set_detail_name(SharedString::from(m.name.clone()));
            }
            if ui.get_detail_note().as_str() != m.note {
                ui.set_detail_note(SharedString::from(m.note.clone()));
            }
            let joined = m.tags.join(", ");
            // ввод «а,б,» парсится в те же теги — переписывать поле нечего
            if ui.get_detail_tags().as_str() != joined
                && parse_tags(ui.get_detail_tags().as_str()) != m.tags
            {
                ui.set_detail_tags(SharedString::from(joined));
            }
            // форма вне справочника (импорт, легаси-данные) показывается как
            // есть последним индексом модели — и не подменяется «Прочее»
            let custom = if FORMS.contains(&m.form.as_str()) {
                None
            } else {
                Some(m.form.as_str())
            };
            let model_custom = ui
                .get_form_options()
                .row_data(FORMS.len())
                .map(|s| s.to_string());
            if model_custom.as_deref() != custom {
                let mut options: Vec<SharedString> =
                    FORMS.iter().map(|s| SharedString::from(*s)).collect();
                if let Some(f) = custom {
                    options.push(SharedString::from(f));
                }
                ui.set_form_options(ModelRc::from(Rc::new(VecModel::from(options))));
            }
            let idx = FORMS
                .iter()
                .position(|f| *f == m.form)
                .unwrap_or(FORMS.len());
            ui.set_detail_form_index(idx as i32);
            rebuild_packages(ui, st);
        }
    }
}

/// Пересчёт подсветки и сводки по текущей дате (вызывается таймером).
pub(crate) fn refresh_statuses(ui: &AppWindow, st: &UiState) {
    rebuild_list(ui, st);
    update_summary(ui, st);
    update_details_readonly(ui, st);
}

/// Сдвигает выделение по видимому (отфильтрованному) списку — клавиши ↑/↓.
pub(crate) fn move_selection(ui: &AppWindow, st: &mut UiState, delta: i32) {
    let model = ui.get_medicines();
    let ids: Vec<i32> = (0..model.row_count())
        .filter_map(|i| model.row_data(i).map(|r| r.id))
        .collect();
    let Some(new_idx) = move_selection_index(&ids, st.selected_id, delta) else {
        return;
    };
    st.selected_id = ids[new_idx];
    ui.set_selected_id(st.selected_id);
    clear_photo_cache(st);
    reset_view_flags(ui);
    update_details(ui, st, true);
}

/// Календарная сетка: 6 недель × 7 дней от отображаемого месяца.
pub(crate) fn cal_rebuild(ui: &AppWindow, st: &UiState) {
    let Some(grid) = cal_grid(st.cal_year, st.cal_month, today()) else {
        return;
    };
    let rows: Vec<CalRow> = grid
        .into_iter()
        .map(|row| CalRow {
            cells: ModelRc::from(Rc::new(VecModel::from(
                row.into_iter()
                    .map(|(label, ymd, in_month, today)| CalCell {
                        label: label.into(),
                        ymd: ymd.into(),
                        in_month,
                        today,
                    })
                    .collect::<Vec<_>>(),
            ))),
        })
        .collect();
    ui.set_cal_weeks(ModelRc::from(Rc::new(VecModel::from(rows))));
    ui.set_cal_month_label(SharedString::from(format!(
        "{} {}",
        MONTHS_RU[(st.cal_month - 1) as usize],
        st.cal_year
    )));
}

pub(crate) fn refresh_all(ui: &AppWindow, st: &mut UiState, reset_form: bool) {
    refresh_tag_options(ui, st);
    rebuild_list(ui, st);
    update_summary(ui, st);
    update_details(ui, st, reset_form);
}

/// Точечное обновление строки лекарства в видимом списке (без сброса прокрутки).
pub(crate) fn update_row(ui: &AppWindow, id: i32, st: &UiState) {
    let Some(m) = st.domain.db.medicine(id) else {
        return;
    };
    let t = today();
    let warn = st.domain.db.warn_days as i64;
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

/// Обновление строки лекарства после правки имени/формы. Если активны поиск,
/// фильтр или сортировка по названию, правка может изменить видимость или
/// порядок строки — тогда список пересобирается целиком; иначе строка
/// обновляется на месте (без сброса прокрутки).
pub(crate) fn refresh_med_row(ui: &AppWindow, id: i32, st: &UiState) {
    let filter_active =
        !st.search.is_empty() || st.filter_idx != 0 || st.tag_filter.is_some() || st.sort_idx != 0;
    if filter_active {
        rebuild_list(ui, st);
    } else {
        update_row(ui, id, st);
    }
}

// ---------- Форма упаковки ----------
