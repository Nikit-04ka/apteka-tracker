//! Настройки приложения — не домен: геометрия окна и тема.
//! Хранятся в `settings.json` рядом с exe (портативный режим, как данные).

use crate::storage::exe_dir;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    /// позиция окна в логических пикселях (None — доверить ОС)
    pub win_x: Option<i32>,
    pub win_y: Option<i32>,
    /// размер окна в логических пикселях (None — размер из .slint)
    pub win_w: Option<f32>,
    pub win_h: Option<f32>,
    pub maximized: bool,
    /// тёмная тема (Palette.color-scheme)
    pub dark_theme: bool,
}

/// Путь к файлу настроек.
pub fn settings_path() -> PathBuf {
    exe_dir().join("settings.json")
}

/// Загрузка настроек; отсутствующий/битый файл — значения по умолчанию.
pub fn load_settings() -> AppSettings {
    std::fs::read(settings_path())
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

/// Сохранение настроек — атомарно, как файл данных.
pub fn save_settings(s: &AppSettings) {
    let path = settings_path();
    let tmp = path.with_extension("json.tmp");
    if let Ok(bytes) = serde_json::to_vec_pretty(s) {
        if std::fs::write(&tmp, bytes).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_roundtrip_and_defaults() {
        let s = AppSettings {
            win_x: Some(120),
            win_y: Some(-40),
            win_w: Some(1150.0),
            win_h: Some(680.0),
            maximized: true,
            dark_theme: true,
        };
        let text = serde_json::to_string(&s).unwrap();
        let back: AppSettings = serde_json::from_str(&text).unwrap();
        assert_eq!(back.win_x, Some(120));
        assert_eq!(back.win_y, Some(-40));
        assert_eq!(back.win_w, Some(1150.0));
        assert!(back.maximized);
        assert!(back.dark_theme);
        // пустой/частичный JSON — значения по умолчанию, без ошибок
        let partial: AppSettings = serde_json::from_str(r#"{"dark_theme": true}"#).unwrap();
        assert!(partial.dark_theme);
        assert_eq!(partial.win_x, None);
        assert!(!partial.maximized);
    }
}
