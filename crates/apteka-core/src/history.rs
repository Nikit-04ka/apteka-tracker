//! История изменений для отмены (Ctrl+Z) и повтора (Ctrl+Y).
//!
//! Хранит полные снимки состояния: база домашней аптечки занимает килобайты,
//! поэтому снимки дешевле и надёжнее любых диффов.

use crate::model::Database;

#[derive(Default)]
pub struct History {
    undo_stack: Vec<Database>,
    redo_stack: Vec<Database>,
    cap: usize,
}

impl History {
    pub fn new(cap: usize) -> Self {
        Self {
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            cap: cap.max(1),
        }
    }

    /// Вызывается перед каждой мутацией: сохраняет состояние «как было».
    /// Очищает redo-стек — после новой правки старая ветка повтора неактуальна.
    pub fn commit(&mut self, snapshot: Database) {
        self.undo_stack.push(snapshot);
        if self.undo_stack.len() > self.cap {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Отменить последнюю мутацию: возвращает предыдущее состояние,
    /// текущее уходит в redo-стек.
    pub fn undo(&mut self, current: Database) -> Option<Database> {
        let prev = self.undo_stack.pop()?;
        self.redo_stack.push(current);
        Some(prev)
    }

    /// Повторить отменённое: возвращает состояние из redo-стека,
    /// текущее уходит в undo-стек.
    pub fn redo(&mut self, current: Database) -> Option<Database> {
        let next = self.redo_stack.pop()?;
        self.undo_stack.push(current);
        Some(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn db_with(name: &str) -> Database {
        let mut db = Database::default();
        db.medicines.push(crate::model::Medicine {
            id: 1,
            name: name.to_string(),
            form: String::new(),
            note: String::new(),
            packages: Vec::new(),
        });
        db
    }

    fn d(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn undo_returns_previous_state() {
        let mut h = History::new(10);
        let v1 = db_with("Первый");
        h.commit(v1.clone());
        let v2 = db_with("Второй");
        assert_eq!(h.undo(v2).unwrap().medicines[0].name, "Первый");
        assert!(h.can_redo());
        assert!(!h.can_undo());
    }

    #[test]
    fn redo_returns_discarded_state() {
        let mut h = History::new(10);
        h.commit(db_with("Первый"));
        let v2 = db_with("Второй");
        let restored = h.undo(v2).unwrap();
        // «Второй» был текущим до отмены — теперь повтор вернёт его
        assert_eq!(h.redo(restored).unwrap().medicines[0].name, "Второй");
        assert!(h.can_undo());
        assert!(!h.can_redo());
    }

    #[test]
    fn new_commit_clears_redo_branch() {
        let mut h = History::new(10);
        h.commit(db_with("A"));
        let _ = h.undo(db_with("B")).unwrap();
        assert!(h.can_redo());
        // новая правка после отмены — redo-ветка исчезает
        h.commit(db_with("C"));
        assert!(!h.can_redo());
    }

    #[test]
    fn undo_on_empty_history_is_none() {
        let mut h = History::new(10);
        assert!(h.undo(db_with("X")).is_none());
        assert!(h.redo(db_with("X")).is_none());
    }

    #[test]
    fn capacity_rotates_oldest_snapshots() {
        let mut h = History::new(3);
        for i in 0..6 {
            h.commit(db_with(&format!("v{i}")));
        }
        let mut names = Vec::new();
        let mut cur = db_with("final");
        while let Some(prev) = h.undo(cur) {
            names.push(prev.medicines[0].name.clone());
            cur = prev;
        }
        // снимки v0..v2 вытеснены лимитом; доступны три последних состояния
        assert_eq!(names, vec!["v5", "v4", "v3"]);
    }

    #[test]
    fn dates_survive_snapshots() {
        let mut h = History::new(5);
        let mut db = Database::default();
        db.medicines.push(crate::model::Medicine {
            id: 1,
            name: "Тест".into(),
            form: "Таблетки".into(),
            note: String::new(),
            packages: vec![crate::model::Package {
                id: 1,
                expiry: d("2028-01-31"),
                month_only: true,
                quantity: "1 уп.".into(),
                note: String::new(),
                photo: String::new(),
                price: None,
                opened: None,
                open_period_days: None,
                disposed: None,
            }],
        });
        h.commit(db);
        let restored = h.undo(Database::default()).unwrap();
        assert!(restored.medicines[0].packages[0].month_only);
        assert_eq!(restored.medicines[0].packages[0].expiry, d("2028-01-31"));
    }
}
