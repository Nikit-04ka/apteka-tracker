// Состояние окна: домен (база + история) и UI-состояние (UiState).

use apteka_core::chrono::NaiveDate;
use apteka_core::model::*;
use apteka_core::state::DomainState;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;

impl UiState {
    pub(crate) fn selected(&self) -> Option<&Medicine> {
        let id = self.selected_id;
        self.domain.db.medicine(id)
    }

    pub(crate) fn selected_mut(&mut self) -> Option<&mut Medicine> {
        let id = self.selected_id;
        self.domain.db.medicine_mut(id)
    }
}

// ---------- Цвета валидации ----------

/// Состояние окна: доменная часть (база+история, живёт в ядре) плюс
/// собственно UI-состояние. Истина — в `domain.db`; UI-свойства Slint —
/// только отображение.
pub(crate) struct UiState {
    /// домен: база, история отмены, флаг несохранённого (apteka-core)
    pub(crate) domain: DomainState,
    pub(crate) selected_id: i32,
    pub(crate) search: String,
    pub(crate) filter_idx: usize,
    pub(crate) sort_idx: usize,
    /// выбранная категория-фильтр (None — все)
    pub(crate) tag_filter: Option<String>,
    /// id редактируемой упаковки, -1 — режим добавления
    pub(crate) pkg_edit_id: i32,
    /// фото, выбранное для ещё не сохранённой упаковки
    pub(crate) pending_photo: Option<std::path::PathBuf>,
    /// кэш декодированных фото (имя файла → готовое изображение)
    pub(crate) photo_cache: RefCell<HashMap<String, slint::Image>>,
    /// день последнего успешного бэкапа (при старте и раз в сутки при первой
    /// мутации); None — бэкапа ещё не было сегодня, повторить при первой мутации
    pub(crate) last_backup_date: Cell<Option<NaiveDate>>,
    /// календарь: отображаемый месяц
    pub(crate) cal_year: i32,
    pub(crate) cal_month: u32,
}
