//! Ядро «Домашней аптечки»: доменная модель, хранение и история изменений.
//! Не содержит UI — пригодно для тестов, CLI и будущих интерфейсов.

pub mod csv;
pub mod history;
pub mod input;
pub mod model;
pub mod settings;
pub mod state;
pub mod storage;

// Реэкспорт для приложений поверх ядра: одна версия даты-времени на весь воркспейс.
pub use chrono;
