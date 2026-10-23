//! Доменное состояние приложения — то, что живёт без UI: база, история
//! правок, флаг «есть несохранённое». UI-состояние (поиск, фильтры,
//! выделение, форма упаковки, кэш фото) остаётся в слое окна.
//!
//! Граница слоёв — чистые функции: ядро принимает `&DomainState`/`&Database`
//! и данные, возвращает данные; про Slint не знает ничего.

use crate::chrono::NaiveDate;
use crate::history::History;
use crate::model::{medicine_info, Database};
use std::cell::Cell;

/// Вид последовательной правки одного поля — для «склейки» шагов истории:
/// правка имени по буквам — один шаг отмены, а не десяток.
#[derive(PartialEq, Clone, Copy)]
pub enum EditKind {
    Name,
    Note,
    Form,
    WarnDays,
    Tags,
}

/// Scope склейки для правок вне лекарств (порог «истекает» в статусной строке).
pub const GLOBAL_SCOPE: i32 = i32::MIN;

/// Доменное состояние: база плюс всё, что к ней относится (история отмены,
/// признак несохранённых правок). Мутируется только методами — так
/// удерживается инвариант «снимок в историю снят до правки».
pub struct DomainState {
    pub db: Database,
    history: History,
    /// вид последней склеенной правки — пара (поле, лекарство): склеиваются
    /// правки одного поля у одного и того же лекарства
    /// (None — после структурных мутаций и шагов по истории)
    last_commit_key: Option<(EditKind, i32)>,
    /// есть несохранённые правки текста (отложенное автосохранение)
    pub unsaved: Cell<bool>,
}

impl DomainState {
    /// `history_capacity` — глубина стека отмены (в приложении — 50).
    pub fn new(db: Database, history_capacity: usize) -> Self {
        Self {
            db,
            history: History::new(history_capacity),
            last_commit_key: None,
            unsaved: Cell::new(false),
        }
    }

    /// Обычный коммит снимка, снятого ДО мутации: структурные правки
    /// никогда не склеиваются с соседними.
    pub fn commit(&mut self, snapshot: Database) {
        self.history.commit(snapshot);
        self.last_commit_key = None;
    }

    /// Коммит со «склейкой» последовательных правок одного поля у одного
    /// лекарства: `scope` — id лекарства, `GLOBAL_SCOPE` — правки вне лекарств.
    /// Та же правка поля у другого лекарства — отдельный шаг отмены.
    pub fn commit_coalesced(&mut self, snapshot: Database, kind: EditKind, scope: i32) {
        let key = (kind, scope);
        if self.last_commit_key != Some(key) {
            self.history.commit(snapshot);
        }
        self.last_commit_key = Some(key);
    }

    /// Шаг по истории: назад при `backwards`, иначе вперёд. Подменяет `db`
    /// и сбрасывает склейку. `false` — в эту сторону истории нет.
    pub fn step_history(&mut self, backwards: bool) -> bool {
        let current = self.db.clone();
        let next = if backwards {
            self.history.undo(current)
        } else {
            self.history.redo(current)
        };
        let Some(next) = next else { return false };
        self.db = next;
        self.last_commit_key = None;
        true
    }

    pub fn can_undo(&self) -> bool {
        self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }
}

/// Стартовое выделение: id лекарства с самым близким сроком годности
/// (при равенстве — с меньшим id); -1, если сроков нет вообще.
pub fn initial_selection(db: &Database, today: NaiveDate) -> i32 {
    let warn = db.warn_days as i64;
    let mut best: Option<(i64, i32)> = None;
    for m in &db.medicines {
        let info = medicine_info(m, today, warn);
        if info.nearest.is_some() {
            let key = (info.nearest_days, m.id);
            if best.is_none_or(|b| key < b) {
                best = Some(key);
            }
        }
    }
    best.map(|b| b.1).unwrap_or(-1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Medicine, Package};

    /// Стартовый порог `warn_days` в `Database::default()`.
    const DEFAULT_WARN: i32 = 60;

    fn med(id: i32, nearest_days: i64) -> Medicine {
        let expiry = chrono::Utc::now().date_naive() + chrono::Duration::days(nearest_days);
        Medicine {
            id,
            name: format!("лек-{id}"),
            form: "таблетки".to_string(),
            note: String::new(),
            tags: Vec::new(),
            packages: vec![Package {
                id,
                expiry,
                month_only: false,
                quantity: "1 шт.".to_string(),
                note: String::new(),
                price: None,
                purchase_date: None,
                opened: None,
                open_period_days: None,
                disposed: None,
                photo: String::new(),
            }],
        }
    }

    #[test]
    fn coalesce_glues_same_kind_into_one_step() {
        let mut s = DomainState::new(Database::default(), 10);
        let a = s.db.clone();
        s.db.warn_days = 30;
        s.commit_coalesced(a, EditKind::Name, 1);
        let b = s.db.clone();
        s.db.warn_days = 45;
        s.commit_coalesced(b, EditKind::Name, 1); // склейка: второй шаг не создан
        assert!(s.step_history(true));
        assert_eq!(s.db.warn_days, DEFAULT_WARN); // один шаг — к исходному
        assert!(!s.step_history(true));
    }

    #[test]
    fn different_kinds_do_not_coalesce() {
        let mut s = DomainState::new(Database::default(), 10);
        let a = s.db.clone();
        s.db.warn_days = 30;
        s.commit_coalesced(a, EditKind::Name, 1);
        let b = s.db.clone();
        s.db.warn_days = 45;
        s.commit_coalesced(b, EditKind::Note, 1); // другое поле — отдельный шаг
        assert!(s.step_history(true));
        assert_eq!(s.db.warn_days, 30);
        assert!(s.step_history(true));
        assert_eq!(s.db.warn_days, DEFAULT_WARN);
    }

    #[test]
    fn same_kind_other_medicine_does_not_coalesce() {
        let mut s = DomainState::new(Database::default(), 10);
        let a = s.db.clone();
        s.db.warn_days = 30;
        s.commit_coalesced(a, EditKind::Name, 1); // правка у лекарства 1
        let b = s.db.clone();
        s.db.warn_days = 45;
        s.commit_coalesced(b, EditKind::Name, 2); // у лекарства 2 — отдельный шаг
        assert!(s.step_history(true));
        assert_eq!(s.db.warn_days, 30, "откат только второй правки");
        assert!(s.step_history(true));
        assert_eq!(s.db.warn_days, DEFAULT_WARN);
        assert!(!s.step_history(true));
    }

    #[test]
    fn structural_commit_breaks_coalescing() {
        let mut s = DomainState::new(Database::default(), 10);
        let a = s.db.clone(); // 60
        s.db.warn_days = 30;
        s.commit_coalesced(a, EditKind::Name, 1);
        let b = s.db.clone(); // 30 — снимок ДО структурной мутации
        s.db.warn_days = 50; // структурная мутация посреди правок имени
        s.commit(b);
        let c = s.db.clone(); // 50
        s.db.warn_days = 45;
        s.commit_coalesced(c, EditKind::Name, 1); // склеиться уже не с чем: новый шаг
                                                  // три отдельных шага отмены: 45 → 50 → 30 → 60
        assert!(s.step_history(true));
        assert_eq!(s.db.warn_days, 50);
        assert!(s.step_history(true));
        assert_eq!(s.db.warn_days, 30);
        assert!(s.step_history(true));
        assert_eq!(s.db.warn_days, DEFAULT_WARN);
        assert!(!s.step_history(true));
    }

    #[test]
    fn step_history_roundtrip_and_directions() {
        let mut s = DomainState::new(Database::default(), 10);
        assert!(!s.step_history(true)); // пустая история назад
        let a = s.db.clone();
        s.db.warn_days = 90;
        s.commit(a);
        assert!(s.can_undo());
        assert!(!s.can_redo());
        assert!(s.step_history(true));
        assert_eq!(s.db.warn_days, DEFAULT_WARN);
        assert!(s.can_redo());
        assert!(s.step_history(false)); // вперёд
        assert_eq!(s.db.warn_days, 90);
        assert!(!s.step_history(false));
    }

    #[test]
    fn history_step_resets_coalescing() {
        let mut s = DomainState::new(Database::default(), 10);
        let a = s.db.clone();
        s.db.warn_days = 30;
        s.commit_coalesced(a, EditKind::Name, 1);
        assert!(s.step_history(true)); // шаг отмены сбрасывает склейку
        let b = s.db.clone();
        s.db.warn_days = 45;
        s.commit_coalesced(b, EditKind::Name, 1);
        assert!(s.step_history(true));
        assert_eq!(s.db.warn_days, DEFAULT_WARN); // не перескочили два шага разом
    }

    #[test]
    fn initial_selection_picks_nearest_and_empty_db_is_minus_one() {
        let today = chrono::Utc::now().date_naive();
        assert_eq!(initial_selection(&Database::default(), today), -1);

        let mut db = Database::default();
        db.medicines.push(med(1, 100));
        db.medicines.push(med(2, 10));
        assert_eq!(initial_selection(&db, today), 2);

        // равные сроки → меньший id
        db.medicines.clear();
        db.medicines.push(med(7, 10));
        db.medicines.push(med(3, 10));
        assert_eq!(initial_selection(&db, today), 3);
    }

    #[test]
    fn journal_events_are_part_of_history_snapshots() {
        // события журнала живут в снимках базы: отмена убирает событие вместе
        // со снимком, повтор возвращает — это контракт (менять только с тестом)
        let mut st = DomainState::new(Database::default(), 50);
        let prev = st.db.clone();
        st.db.journal_push("упаковка открыта", "Нурофен", "уп. 1");
        st.commit(prev);
        assert_eq!(st.db.events.len(), 1);
        assert!(st.step_history(true), "шаг назад есть");
        assert!(
            st.db.events.is_empty(),
            "undo откатывает событие журнала вместе со снимком"
        );
        assert!(st.step_history(false), "шаг вперёд есть");
        assert_eq!(st.db.events.len(), 1, "redo возвращает событие");
    }
}
