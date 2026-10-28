//! Одноразовый замер производительности на синтетической нагрузке.
//! Запуск: cargo test --release -p apteka-core --test perf_probe -- --nocapture
use apteka_core::chrono::NaiveDate;
use apteka_core::input::visible_medicines;
use apteka_core::model::{medicine_info, Database, Medicine, Package};
use apteka_core::storage::{load_from, save_to};
use std::sync::Arc;
use std::time::Instant;

// Счётчик аллокаций: измеряет реальную занятую память (пик — после утечек нет,
// dealloc вычитается) — для оценки стоимости 50 снимков истории.
struct CountingAlloc;

static ALLOCATED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

unsafe impl std::alloc::GlobalAlloc for CountingAlloc {
    unsafe fn alloc(&self, layout: std::alloc::Layout) -> *mut u8 {
        ALLOCATED.fetch_add(layout.size(), std::sync::atomic::Ordering::Relaxed);
        unsafe { std::alloc::System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: std::alloc::Layout) {
        ALLOCATED.fetch_sub(layout.size(), std::sync::atomic::Ordering::Relaxed);
        unsafe { std::alloc::System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: std::alloc::Layout, new_size: usize) -> *mut u8 {
        let a = &ALLOCATED;
        use std::sync::atomic::Ordering::Relaxed;
        a.fetch_add(new_size, Relaxed);
        a.fetch_sub(layout.size(), Relaxed);
        unsafe { std::alloc::System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: CountingAlloc = CountingAlloc;

fn allocated_mib() -> f64 {
    ALLOCATED.load(std::sync::atomic::Ordering::Relaxed) as f64 / (1024.0 * 1024.0)
}

fn build_db(n_meds: usize, pkgs_per_med: usize) -> Database {
    let mut db = Database::default();
    let expiry = NaiveDate::from_ymd_opt(2030, 1, 1).unwrap();
    for i in 0..n_meds {
        let packages: Vec<Package> = (0..pkgs_per_med)
            .map(|j| Package {
                id: (i * pkgs_per_med + j + 1) as i32,
                expiry,
                month_only: j % 4 == 0,
                quantity: "2 уп.".to_string(),
                note: "в ящике, верхняя полка".to_string(),
                price: Some(350.5),
                purchase_date: None,
                opened: if j == 0 { Some(expiry) } else { None },
                open_period_days: if j == 0 { Some(30) } else { None },
                disposed: None,
                photo: String::new(),
            })
            .collect();
        db.medicines.push(Arc::new(Medicine {
            id: (i + 1) as i32,
            name: format!("Препарат Противовоспалительный №{i}"),
            form: "Таблетки покрытые оболочкой".to_string(),
            note: "принимать по инструкции, хранить в прохладном месте".to_string(),
            tags: vec!["жаропонижающее".to_string()],
            packages,
        }));
    }
    db
}

fn bench(name: &str, n_meds: usize) {
    let mut db = build_db(n_meds, 3);
    let dir = std::env::temp_dir().join(format!("apteka-perf-{n_meds}"));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("data.json");
    let today = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();

    println!("=== {n_meds} лекарств × 3 упаковки ({name}) ===");

    // rebuild_list: полный скан + сортировка
    let t = Instant::now();
    let rows = visible_medicines(&db, "", None, 0, 0, today);
    println!(
        "  visible_medicines (все, сорт. по сроку): {:>8.2?}  ({} строк)",
        t.elapsed(),
        rows.len()
    );

    // поиск без совпадений — худший случай скана
    let t = Instant::now();
    let rows = visible_medicines(&db, "неттакогословавообще", None, 0, 0, today);
    println!(
        "  visible_medicines (поиск, 0 совпадений): {:>8.2?}",
        t.elapsed()
    );
    assert!(rows.is_empty());

    // update_summary: medicine_info по всем + сумма цен
    let t = Instant::now();
    let warn = db.warn_days as i64;
    for m in &db.medicines {
        let _ = medicine_info(m, today, warn);
        let _cost: f64 = m.packages.iter().filter_map(|p| p.price).sum();
    }
    println!(
        "  update_summary (info+цены):              {:>8.2?}",
        t.elapsed()
    );

    // снимок истории: db.clone() на каждую мутацию
    let t = Instant::now();
    for _ in 0..10 {
        let _prev = db.clone();
    }
    println!(
        "  db.clone() (снимок истории × 10):        {:>8.2?}",
        t.elapsed()
    );

    // память истории: 50 снимков, как держит History (стек отмены);
    // между снимками — правки разных лекарств (COW «открепляет» по одному)
    let before = allocated_mib();
    let mut history: Vec<Database> = Vec::new();
    for i in 0..50 {
        history.push(db.clone());
        if let Some(m) = db.medicine_mut(i % n_meds as i32 + 1) {
            m.note = format!("правка {i}");
        }
    }
    let held = allocated_mib() - before;
    println!(
        "  память 50 снимков истории:               {:>8.1} МиБ ({:.2} МиБ/снимок)",
        held,
        held / 50.0
    );
    drop(history);

    // сохранение на диск (fsync внутри), среднее по 5
    save_to(&db, &path).unwrap();
    let t = Instant::now();
    for _ in 0..5 {
        save_to(&db, &path).unwrap();
    }
    println!(
        "  save_to с fsync (среднее):               {:>8.2?}/шт",
        t.elapsed() / 5
    );

    // загрузка
    let t = Instant::now();
    let loaded = load_from(&path);
    println!(
        "  load_from (парс JSON):                   {:>8.2?}  ({} лекарств)",
        t.elapsed(),
        loaded.medicines.len()
    );

    let bytes = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
    println!("  размер файла: {:.1} КиБ", bytes as f64 / 1024.0);
    std::fs::remove_dir_all(&dir).ok();
    println!();
}

#[test]
fn perf_probe() {
    bench("реалистичный домашний", 200);
    bench("стрессовый", 1000);
    bench("экстрим", 5000);
}
