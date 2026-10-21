//! Хранение и загрузка данных: JSON рядом с exe (портативный режим) и экспорт/импорт CSV.

use crate::model::Database;
use chrono::Local;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Папка, где лежит exe (портативный режим).
pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Путь к файлу данных.
pub fn data_file_path() -> PathBuf {
    exe_dir().join("apteka_data.json")
}

/// Путь к файлу экспорта CSV.
pub fn csv_file_path() -> PathBuf {
    exe_dir().join("apteka_export.csv")
}

/// Каталог резервных копий (рядом с exe).
pub fn backup_dir() -> PathBuf {
    exe_dir().join("backups")
}

/// Каталог фото упаковок (рядом с exe).
pub fn photos_dir() -> PathBuf {
    exe_dir().join("photos")
}

/// Резервная копия файла данных при запуске; хранятся последние 5 копий.
pub fn backup() -> io::Result<Option<PathBuf>> {
    backup_from_to(&data_file_path(), &backup_dir(), 5)
}

/// Копирует `src` в каталог `dir` под именем с меткой времени и оставляет
/// только `keep` самых свежих копий.
pub fn backup_from_to(src: &Path, dir: &Path, keep: usize) -> io::Result<Option<PathBuf>> {
    if !src.exists() {
        return Ok(None);
    }
    fs::create_dir_all(dir)?;
    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let dst = dir.join(format!("apteka_{stamp}.json"));
    fs::copy(src, &dst)?;
    rotate_backups(dir, keep);
    Ok(Some(dst))
}

/// Удаляет самые старые резервные копии, оставляя `keep` последних.
/// Имена вида `apteka_YYYYMMDD-HHMMSS.json` сортируются хронологически.
pub fn rotate_backups(dir: &Path, keep: usize) {
    let mut files: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok().map(|e| e.path())).collect(),
        Err(_) => return,
    };
    files.retain(|p| p.is_file() && p.extension().is_some_and(|ext| ext == "json"));
    files.sort();
    while files.len() > keep {
        match files.first() {
            Some(oldest) => {
                let _ = fs::remove_file(oldest);
                files.remove(0);
            }
            None => break,
        }
    }
}

/// Список резервных копий, самые свежие первыми.
pub fn list_backups() -> Vec<PathBuf> {
    let dir = backup_dir();
    let mut files: Vec<PathBuf> = match fs::read_dir(&dir) {
        Ok(rd) => rd.filter_map(|e| e.ok().map(|e| e.path())).collect(),
        Err(_) => return Vec::new(),
    };
    files.retain(|p| p.is_file() && p.extension().is_some_and(|ext| ext == "json"));
    files.sort();
    files.reverse();
    files
}

/// Удаляет потерянные фото: файлы вида `pkg_*.{png,jpg,jpeg}`, на которые
/// нет ссылок в базе. Возвращает количество удалённых файлов.
pub fn cleanup_orphan_photos(db: &Database) -> usize {
    cleanup_orphan_photos_in(&photos_dir(), db)
}

/// То же с явным каталогом — для тестов.
pub fn cleanup_orphan_photos_in(dir: &Path, db: &Database) -> usize {
    let used: std::collections::HashSet<String> = db
        .medicines
        .iter()
        .flat_map(|m| m.packages.iter())
        .filter(|p| !p.photo.is_empty())
        .map(|p| p.photo.clone())
        .collect();
    let entries = match fs::read_dir(dir) {
        Ok(rd) => rd,
        Err(_) => return 0,
    };
    let mut removed = 0usize;
    for e in entries.flatten() {
        let path = e.path();
        if !path.is_file() {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_string();
        let ext = path
            .extension()
            .and_then(|x| x.to_str())
            .map(|x| x.to_ascii_lowercase())
            .unwrap_or_default();
        let is_app_photo =
            name.starts_with("pkg_") && matches!(ext.as_str(), "png" | "jpg" | "jpeg");
        if is_app_photo && !used.contains(&name) && fs::remove_file(&path).is_ok() {
            removed += 1;
        }
    }
    removed
}

/// Загрузка базы со стороны старта приложения: с полной диагностикой.
pub fn load() -> LoadOutcome {
    load_outcome_from(&data_file_path())
}

/// Результат загрузки базы с диагностикой.
pub enum LoadOutcome {
    /// Успешно загружена
    Loaded(Database),
    /// Файла нет — первый запуск
    Missing,
    /// Файл повреждён (переименован в *.corrupt-*) — работаем с пустой базой
    Corrupt {
        database: Database,
        backup: PathBuf,
        msg: String,
    },
    /// Файл не читается (занят/права) — вызывающий должен завершиться,
    /// не перезаписывая данные
    Io(String),
}

/// Загрузка с полной диагностикой (для старта приложения и тестов).
pub fn load_outcome_from(path: &Path) -> LoadOutcome {
    if !path.exists() {
        return LoadOutcome::Missing;
    }
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => return LoadOutcome::Io(format!("{}: {e}", path.display())),
    };
    match serde_json::from_str::<Database>(&text) {
        Ok(mut db) => {
            db.migrate();
            LoadOutcome::Loaded(db)
        }
        Err(e) => {
            let stamp = Local::now().format("%Y%m%d-%H%M%S");
            let backup = PathBuf::from(format!("{}.corrupt-{stamp}", path.to_string_lossy()));
            let msg = format!("{e}");
            let _ = fs::rename(path, &backup);
            LoadOutcome::Corrupt {
                database: Database::default(),
                backup,
                msg,
            }
        }
    }
}

/// Загрузка базы из файла (для восстановления из бэкапа) — без побочных эффектов.
pub fn load_database_from(path: &Path) -> Result<Database, String> {
    match load_outcome_from(path) {
        LoadOutcome::Loaded(db) => Ok(db),
        LoadOutcome::Missing => Err(format!("Файл не найден: {}", path.display())),
        LoadOutcome::Corrupt { msg, .. } => Err(format!("Повреждённый JSON: {msg}")),
        LoadOutcome::Io(e) => Err(e),
    }
}

/// Загрузка «как раньше» — для тестов и внутренних нужд: пустая база при проблемах.
pub fn load_from(path: &Path) -> Database {
    match load_outcome_from(path) {
        LoadOutcome::Loaded(db) => db,
        _ => Database::default(),
    }
}

/// Сохраняет базу рядом с exe.
pub fn save(db: &Database) -> io::Result<PathBuf> {
    let path = data_file_path();
    save_to(db, &path)?;
    Ok(path)
}

/// Атомарная запись: сначала во временный файл, затем переименование.
pub fn save_to(db: &Database, path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("json.tmp");
    {
        let text = serde_json::to_string_pretty(db).map_err(io::Error::other)?;
        let mut f = fs::File::create(&tmp)?;
        f.write_all(text.as_bytes())?;
        f.sync_all().ok();
    }
    fs::rename(&tmp, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Medicine, Package};
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// Уникальная временная папка для каждого теста.
    fn temp_dir(tag: &str) -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "apteka-test-{}-{}-{tag}{n}",
            std::process::id(),
            Local::now().timestamp_millis()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample_db() -> Database {
        let mut db = Database {
            warn_days: 45,
            ..Default::default()
        };
        db.medicines.push(Medicine {
            id: 1,
            name: "Парацетамол \"Тева\"".to_string(),
            form: "Таблетки".to_string(),
            note: "от головы".to_string(),
            tags: Vec::new(),
            packages: vec![
                Package {
                    id: 1,
                    expiry: chrono::NaiveDate::from_ymd_opt(2030, 1, 1).unwrap(),
                    month_only: false,
                    quantity: "2 уп.".to_string(),
                    note: "в ящике".to_string(),
                    price: Some(350.0),
                    opened: None,
                    open_period_days: None,
                    disposed: None,
                    photo: String::new(),
                },
                // «Годен до 01 2028» → 31.01.2028, месяц без дня
                Package {
                    id: 2,
                    expiry: chrono::NaiveDate::from_ymd_opt(2028, 1, 31).unwrap(),
                    month_only: true,
                    quantity: "1 фл.".to_string(),
                    note: String::new(),
                    price: None,
                    opened: None,
                    open_period_days: None,
                    disposed: None,
                    photo: String::new(),
                },
            ],
        });
        db
    }

    #[test]
    fn load_outcome_reports_each_failure_kind() {
        let dir = temp_dir("outcome");
        // Missing: файла нет
        assert!(matches!(
            load_outcome_from(&dir.join("nope.json")),
            LoadOutcome::Missing
        ));
        // Io: путь указывает на каталог
        assert!(matches!(load_outcome_from(&dir), LoadOutcome::Io(_)));
        // Corrupt: мусор переименовывается в *.corrupt-*
        let bad = dir.join("bad.json");
        fs::write(&bad, "{битый json").unwrap();
        match load_outcome_from(&bad) {
            LoadOutcome::Corrupt { backup, .. } => {
                assert!(backup.exists(), "повреждённый файл сохранён для спасения");
                assert!(!bad.exists());
            }
            _ => panic!("ожидали Corrupt"),
        }
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn roundtrip_saves_and_loads() {
        let dir = temp_dir("roundtrip");
        let path = dir.join("data.json");
        let mut db = sample_db();
        db.journal_push("тест события", "лек", "уп");
        save_to(&db, &path).unwrap();
        let loaded = load_from(&path);
        assert_eq!(loaded.warn_days, 45);
        assert_eq!(loaded.medicines.len(), 1);
        assert_eq!(loaded.medicines[0].name, "Парацетамол \"Тева\"");
        assert_eq!(loaded.medicines[0].packages[0].quantity, "2 уп.");
        let mp = &loaded.medicines[0].packages[1];
        assert!(mp.month_only, "флаг month_only должен пережить сохранение");
        assert_eq!(mp.expiry.to_string(), "2028-01-31");
        assert_eq!(loaded.events.len(), 1, "журнал переживает round-trip");
        assert_eq!(loaded.events[0].kind, "тест события");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn load_missing_file_returns_default() {
        let dir = temp_dir("missing");
        let db = load_from(&dir.join("nope.json"));
        assert_eq!(db.warn_days, 60);
        assert!(db.medicines.is_empty());
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn cleanup_orphan_photos_removes_only_unreferenced() {
        let dir = temp_dir("photos");
        let photos = dir.join("photos");
        fs::create_dir_all(&photos).unwrap();
        fs::write(photos.join("pkg_1.png"), "a").unwrap();
        fs::write(photos.join("pkg_9.jpg"), "b").unwrap();
        fs::write(photos.join("pkg_8.PNG"), "c").unwrap();
        fs::write(photos.join("notes.txt"), "d").unwrap();
        let mut db = sample_db();
        db.medicines[0].packages[0].photo = "pkg_1.png".to_string();
        let removed = cleanup_orphan_photos_in(&photos, &db);
        assert_eq!(removed, 2, "удалены только сироты pkg_*");
        assert!(
            photos.join("pkg_1.png").exists(),
            "используемое фото на месте"
        );
        assert!(photos.join("notes.txt").exists(), "чужие файлы не тронуты");
        assert!(!photos.join("pkg_9.jpg").exists());
        assert!(
            !photos.join("pkg_8.PNG").exists(),
            "расширение без регистра"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn legacy_json_without_version_is_normalized() {
        let dir = temp_dir("legacy");
        let path = dir.join("data.json");
        // файл старой схемы: без поля version
        fs::write(
            &path,
            r#"{"warn_days": 30, "next_med_id": 2, "next_pkg_id": 1,
                "medicines": [{"id": 1, "name": "Йод", "form": "Раствор", "note": "",
                "packages": [{"id": 1, "expiry": "2030-05-01", "quantity": "1 фл.",
                "note": "", "photo": ""}]}]}"#,
        )
        .unwrap();
        let db = load_from(&path);
        assert_eq!(
            db.version,
            crate::model::DATA_VERSION,
            "версия нормализована"
        );
        assert_eq!(db.warn_days, 30);
        assert_eq!(db.medicines[0].name, "Йод");
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn corrupt_file_is_backed_up_not_deleted() {
        let dir = temp_dir("corrupt");
        let path = dir.join("data.json");
        fs::write(&path, "{ это не json").unwrap();
        let db = load_from(&path);
        assert!(db.medicines.is_empty());
        let leftovers: Vec<_> = fs::read_dir(&dir).unwrap().collect();
        assert_eq!(
            leftovers.len(),
            1,
            "повреждённый файл должен остаться как .corrupt-*"
        );
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn backup_copies_data_file() {
        let dir = temp_dir("backup");
        let src = dir.join("apteka_data.json");
        save_to(&sample_db(), &src).unwrap();
        let bdir = dir.join("backups");
        // нет исходника → нет копии
        assert!(backup_from_to(&dir.join("missing.json"), &bdir, 5)
            .unwrap()
            .is_none());
        // есть исходник → копия создана и читается
        let copied = backup_from_to(&src, &bdir, 5).unwrap();
        assert!(copied.is_some(), "бэкап должен создаться");
        let loaded = load_from(&copied.unwrap());
        assert_eq!(loaded.medicines.len(), 1);
        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rotation_keeps_only_newest_backups() {
        let dir = temp_dir("rotate");
        fs::create_dir_all(&dir).unwrap();
        for name in [
            "apteka_20200101-000001.json",
            "apteka_20210101-000001.json",
            "apteka_20220101-000001.json",
            "apteka_20230101-000001.json",
        ] {
            fs::write(dir.join(name), "{}").unwrap();
        }
        rotate_backups(&dir, 2);
        let mut left: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(
            left,
            vec!["apteka_20220101-000001.json", "apteka_20230101-000001.json"],
            "должны остаться две самые свежие копии"
        );
        fs::remove_dir_all(&dir).ok();
    }
}
