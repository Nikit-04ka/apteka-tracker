# apteka-tracker — Домашняя аптечка

## Overview
Windows-приложение на Rust (GUI на Slint) для учёта лекарств домашней аптечки
и их сроков годности. У одного лекарства может быть несколько упаковок
с разными сроками. Записи с истекающим/истекшим сроком подсвечиваются.

## Tech Stack
- Язык: Rust 1.95 (x86_64-pc-windows-msvc)
- GUI: Slint 1.18 (стиль fluent, сборка через slint-build)
- Данные: serde + serde_json, даты: chrono
- Хранение: JSON рядом с exe (`apteka_data.json`), атомарная запись через `.tmp` + rename

## Project Structure

Cargo workspace (repo root = workspace root): ядро отделено от UI.

```
Cargo.toml                      # workspace
crates/apteka-core/src/         # lib: model.rs, storage.rs, history.rs
crates/apteka-tracker/          # bin: build.rs, ui/app.slint, assets/, src/main.rs
docs/screenshot.png
```

## Conventions
- Язык интерфейса и пользовательских строк — русский.
- Статусы — `i32`-константы: `STATUS_OK=0, STATUS_SOON=1, STATUS_EXPIRED=2, STATUS_EMPTY=3`.
- Истина — в `UiState.domain.db` (структура `Database`); UI-свойства Slint — только отображение.
- Сохранение на диск после каждого мутационного колбэка (`persist`).
- Даты в UI: `ДД.ММ.ГГГГ`; ввод принимает также `ГГГГ-ММ-ДД` и месяц без дня
  (`ММ.ГГГГ`, `ГГГГ-ММ`, `ММ/ГГГГ`, `ММ ГГГГ`) — `model::parse_date_input`.
- Категории лекарств: `Medicine.tags` (свободные теги) — редактируются в
  деталях строкой через запятую (`parse_tags`), попадают в поиск и в CSV
  (колонка «Категории»); правки склеиваются в истории (`EditKind::Tags`).
- Настройки окна/темы — `settings.json` рядом с exe
  (`apteka_core::settings`), не домен: геометрия в логических пикселях,
  тёмная тема (`Palette.color-scheme`, экспортирован в Rust).
- В .slint не использовать имя `row` для свойств — зарезервировано раскладкой.

## Key Decisions
- Slint выбран пользователем вместо egui/Tauri; портативное хранение (рядом с exe);
  порог «истекает» по умолчанию 60 дней (настраивается в UI, 5..365).
- **Аптечная конвенция «до месяца»**: `Package.month_only=true` означает «годен до конца
  месяца»; в `expiry` хранится последний день месяца (31.01.2028 для «01 2028»), вся
  логика статусов работает без ветвлений; отображается как `ММ.ГГГГ` (`fmt_date_display`).
- Мастер-детей интерфейс без модальных окон: редактирование на месте,
  добавление лекарства создаёт запись «Новое лекарство» и открывает её в деталях.
- Удаление лекарства — двухшаговое подтверждение (кнопка меняет текст на «Точно удалить?»).
- Повреждённый JSON переименовывается в `*.corrupt-<дата>`, не удаляется.
- CSV: разделитель `;`, BOM, экранирование кавычек — для прямого открытия в Excel.
- Иконка рисуется скриптом `assets/make_icon.py` (Pillow) → `assets/icon.png` (окно,
  `@image-url`) + `assets/icon.ico` (ресурс exe через `winresource` в build.rs).
- Один экземпляр: файловый замок `apteka.lock` (std `try_lock`, сам освобождается
  при сбое); второй запуск показывает MessageBoxW и завершается. Релиз собирается
  с `windows_subsystem = "windows"` (без консоли).
- Автобэкап при старте + раз в сутки при первой мутации нового дня
  (`persist` → `last_backup_date`): `storage::backup()` → `backups/apteka_*.json`,
  ротация 5 копий.
- Клавиатура: корневой `FocusScope` + `forward-focus` — ↑/↓ (`move-selection`),
  Ctrl+N/Ctrl+F, Delete (двухшаговое), Escape. Стрелки обрабатываются по `Key.*`.
- Статусы пересчитываются ежечасно (`slint::Timer` → `refresh_statuses`, только
  чтение; форма редактирования не сбрасывается).
- «Клонировать упаковку» (`pkg-clone`): форма новой упаковки предзаполняется
  количеством/комментарием клона и сроком «сегодня + 1 год»; сохранение —
  через обычный `pkg-save`.
- Форма упаковки ↔ Package: единая точка — `input::PackageForm`
  (`parse` → `into_package(id)` для новой, `apply_to(&mut Package)` для правки).
  Поштучное присвоение полей упаковки в колбэках не допускается; поля
  жизненного цикла (id, вскрытие, списание, фото) формой не трогаются.
- Разделение состояний: `apteka_core::state::DomainState` (db, история отмены,
  unsaved; мутации только методами `commit`/`commit_coalesced`/`step_history`)
  + `UiState` в main.rs (selected_id, search, filter/sort, pkg_edit_id,
  pending_photo, photo_cache, cal_*). Граница — чистые функции ядра
  (`medicine_info`, `initial_selection`, `shopping_list`, `parse_date_input`,
  `visible_medicines` — поиск/фильтр/сортировка списка); ядро не знает про Slint.
- Файл данных несёт `version` (`model::DATA_VERSION`); старые JSON без поля
  читаются (`#[serde(default)]`) и нормализуются в `Database::migrate()` —
  точка будущих миграций. Повторный импорт CSV не создаёт дублей: пары
  (имя, форма) без регистра сверяются с базой и внутри файла.
- Undo/redo: `apteka_core::history::History` — полные снимки `Database`
  (данные — килобайты), стек 50 шагов. Любой мутационный колбэк обязан
  снять `prev = st.domain.db.clone()` ДО мутации и вызвать `commit_history(...)`
  (для правок одного поля — `commit_history_coalesced(...)`, склейка в
  `DomainState`). Хоткеи Ctrl+Z / Ctrl+Y / Ctrl+Shift+Z, кнопки в статусной строке.
- Каркас колбэков — через `with_state!`: макрос захватывает Weak окна и клон
  `Rc<RefCell<UiState>>`, делает upgrade и `borrow()`/`borrow_mut()` (по наличию
  `mut` перед именем состояния) на всё тело. Не применять, где заём нельзя
  держать всё тело (диалог `rfd` в `on_pick_photo` прокачивает message loop)
  или колбэку нужен ещё один Weak/окно (`backups.on_restore`,
  `import_win.on_do_import`). Хвост мутационного колбэка —
  `commit_refresh_persist(ui, st, prev, reset_form)`; undo/redo — общий
  `history_step(ui, st, backwards)`.
- Отложенное автосохранение текста: единственный `save_timer: Rc<slint::Timer>`
  (SingleShot 800 мс, при старте регистрируется один раз с замыканием persist);
  колбэки правок делают `st.unsaved.set(true); timer.restart()`.
- Жизненный цикл упаковки: `Package.opened` (дата вскрытия) +
  `open_period_days` → `effective_expiry()` = min(заводской, вскрытие+период);
  `Package.disposed: Option<DisposedInfo>` — мягкое списание (скрыто из учёта
  и статусов, видно по toggle «Показать списанные», можно «Вернуть»).
  STATUS_DISPOSED=4 передаётся в UI.
- Цена `Package.price: Option<f64>` → чип «Стоимость» (только активные упаковки).
- Журнал: `Database.events` (cap 400, `journal_push`) — только структурные
  события; окно JournalWindow наполняется при каждом открытии (новейшие сверху).

## Производительность (замеры 2026-10, release, SSD)

Комфортный масштаб — до ~1000 лекарств. Узкие места при росте:
- `persist()` = полный fsync JSON на каждую мутацию (~7 мс @200, ~38 мс @5000
  на SSD; на флешке заметно больше). Осознанный trade-off в пользу
  сохранности; текст правок уже дебаунится (800 мс).
- Список/упаковки обновляются на месте (`sync_model`: `set_row_data` только
  по диффу, дополнение/усечение хвоста; новый VecModel — только при первом
  заполнении) — делегаты не пересоздаются, скролл не прыгает. Точечная
  правка одной строки — паттерн `update_row`. Не использовать `VecModel::set_vec`/
  `clear` для этих свойств — они делают полный ресет модели (сброс скролла).
- История = 50 полных снимков `Database` в RAM (~30 МБ @1000 лекарств).
Перф-пробник: `cargo test --release -p apteka-core --test perf_probe -- --nocapture`.

## Backlog рефакторинга (шаги 1-3, по порядку)

1. with_state-хелпер: макрос или `fn persist_state(ui, s)`; убрать копипаст-каркас в 20 колбэках main.rs (borrow/commit/refresh/persist). S. — **выполнен**: `with_state!` + `commit_refresh_persist` + `history_step`, `schedule_save` заменён на `save_timer.restart()`.
2. Разделить State на DomainState (db, history, unsaved — в apteka-core) и UiState (search, filter, selected, pkg_edit_id, pending_photo, cal_*). Граница — чистые функции. M. — **выполнен**: `apteka_core::state::{DomainState, EditKind, initial_selection}` + `UiState` (содержит `domain`); логика истории (склейка, шаг) переехала в ядро с тестами.
3. Единая точка «форма ↔ Package»: parse_package_input уже в core; добавить PackageForm-структуру и собрать Package из неё в pkg_save (убрать 8-контактную правку полей). M. — **выполнен**: `input::PackageForm` (parse/into_package/apply_to), pkg_save разбирает форму одним вызовом; тесты на «apply не трогает жизненный цикл».
Шаги 4 (csv.rs) и 5 (Badge/StatusDot централизация) — выполнены.
