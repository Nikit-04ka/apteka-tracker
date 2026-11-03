// Вторичные окна: журнал, отчёты, покупки, бэкапы, импорт (F6).

use apteka_core::csv;
use apteka_core::i18n::{canonical, fmt, tr, Str};
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use crate::misc::*;
use crate::state::*;
use crate::ui_sync::*;
use crate::with_state;
use crate::*;

/// « (2 без цены)» — упаковки, учтённые в количестве, но не в денежной сумме.
fn unpriced_note(unpriced: usize) -> String {
    if unpriced > 0 {
        fmt(Str::RepUnpriced, &[&unpriced.to_string()])
    } else {
        String::new()
    }
}

/// Набор вторичных окон: держим сильные ссылки, чтобы окна жили до конца main().
pub(crate) struct SecondaryWindows {
    _journal: JournalWindow,
    _reports: ReportWindow,
    _shopping: ShoppingWindow,
    _backups: BackupsWindow,
    _import: ImportWindow,
}

impl SecondaryWindows {
    /// Применяет текущий язык ко всем вторичным окнам: у каждого компонента
    /// свой экземпляр глобала I18n.
    pub(crate) fn apply_i18n(&self) {
        for g in [
            self._journal.global::<I18n>(),
            self._reports.global::<I18n>(),
            self._shopping.global::<I18n>(),
            self._backups.global::<I18n>(),
            self._import.global::<I18n>(),
        ] {
            crate::l10n::apply_i18n_global(&g);
        }
    }
}

/// Создаёт вторичные окна (журнал, отчёты, покупки, бэкапы, импорт) и
/// связывает их колбэки (F6).
pub(crate) fn build_windows(
    ui: &AppWindow,
    state: &Rc<RefCell<UiState>>,
) -> Result<SecondaryWindows, slint::PlatformError> {
    let journal = JournalWindow::new()?;
    // Отчёты: списания и покупки по годам
    let reports = ReportWindow::new()?;
    {
        let jh = reports.as_weak();
        reports.on_close_clicked(move || {
            let j = jh.unwrap();
            let _ = j.hide();
        });
    }
    {
        let jh = reports.as_weak();
        with_state!(ui, state, on_open_reports, |_ui, st| {
            let j = jh.unwrap();
            let mut lines: Vec<String> = Vec::new();
            let disposals = disposals_by_year(&st.domain.db);
            if disposals.is_empty() {
                lines.push(tr(Str::RepDisposalsNone).to_string());
            } else {
                lines.push(tr(Str::RepDisposals).to_string());
                for r in &disposals {
                    lines.push(fmt(
                        Str::RepYearRow,
                        &[
                            &r.year.to_string(),
                            &r.count.to_string(),
                            &format!("{:.2}", r.sum),
                            &unpriced_note(r.unpriced),
                        ],
                    ));
                }
                let total_c: usize = disposals.iter().map(|r| r.count).sum();
                let total_s: f64 = disposals.iter().map(|r| r.sum).sum();
                let total_u: usize = disposals.iter().map(|r| r.unpriced).sum();
                lines.push(fmt(
                    Str::RepTotal,
                    &[
                        &total_c.to_string(),
                        &format!("{total_s:.2}"),
                        &unpriced_note(total_u),
                    ],
                ));
            }
            lines.push(String::new());
            let purchases = purchases_by_year(&st.domain.db);
            if purchases.is_empty() {
                lines.push(tr(Str::RepPurchasesNone).to_string());
            } else {
                lines.push(tr(Str::RepPurchases).to_string());
                for r in &purchases {
                    lines.push(fmt(
                        Str::RepYearRow,
                        &[
                            &r.year.to_string(),
                            &r.count.to_string(),
                            &format!("{:.2}", r.sum),
                            &unpriced_note(r.unpriced),
                        ],
                    ));
                }
                let total_c: usize = purchases.iter().map(|r| r.count).sum();
                let total_s: f64 = purchases.iter().map(|r| r.sum).sum();
                let total_u: usize = purchases.iter().map(|r| r.unpriced).sum();
                lines.push(fmt(
                    Str::RepTotalPurchases,
                    &[
                        &total_c.to_string(),
                        &format!("{total_s:.2}"),
                        &unpriced_note(total_u),
                    ],
                ));
            }
            lines.push(String::new());
            lines.push(match oldest_active_package(&st.domain.db) {
                Some(p) => fmt(
                    Str::RepOldest,
                    &[&fmt_date_display(
                        p.effective_expiry(),
                        p.month_only && p.effective_expiry() == p.expiry,
                    )],
                ),
                None => tr(Str::RepNoActive).to_string(),
            });
            j.set_lines(ModelRc::from(Rc::new(VecModel::from(
                lines
                    .into_iter()
                    .map(SharedString::from)
                    .collect::<Vec<_>>(),
            ))));
            let _ = j.show();
        });
    }
    {
        let jh = journal.as_weak();
        journal.on_close_clicked(move || {
            let j = jh.unwrap();
            let _ = j.hide();
        });
    }
    {
        let jh = journal.as_weak();
        with_state!(ui, state, on_open_journal, |_ui, st| {
            let j = jh.unwrap();
            let mut rows: Vec<SharedString> = Vec::new();
            if st.domain.db.events.len() >= EVENTS_CAP {
                // журнал хранит ограниченное число событий — говорим об усечении,
                // а не показываем «все» события молча
                rows.push(SharedString::from(fmt(
                    Str::JrnTruncated,
                    &[&EVENTS_CAP.to_string()],
                )));
            }
            rows.extend(st.domain.db.events.iter().rev().map(|e| {
                // вид события хранится каноном — при показе переводим
                SharedString::from(fmt(
                    Str::JrnRow,
                    &[
                        &e.at.format("%d.%m.%Y %H:%M").to_string(),
                        &e.medicine,
                        &apteka_core::i18n::journal_kind_display(&e.kind),
                        &e.package,
                    ],
                ))
            }));
            j.set_events(ModelRc::from(Rc::new(VecModel::from(rows))));
            let _ = j.show();
        });
    }

    let shopping = ShoppingWindow::new()?;
    {
        let jh = shopping.as_weak();
        with_state!(ui, state, on_open_shopping, |_ui, st| {
            let j = jh.unwrap();
            let rows: Vec<SharedString> = shopping_list(&st.domain.db, today())
                .into_iter()
                .map(SharedString::from)
                .collect();
            j.set_lines(ModelRc::from(Rc::new(VecModel::from(rows))));
            j.set_note(SharedString::new());
            let _ = j.show();
        });
    }
    {
        let jh = shopping.as_weak();
        shopping.on_copy_to_clipboard(move || {
            let j = jh.unwrap();
            let model = j.get_lines();
            if model.row_count() == 0 {
                j.set_note(SharedString::from(tr(Str::ShopCopyEmpty)));
                return;
            }
            let text: String = (0..model.row_count())
                .filter_map(|i| model.row_data(i))
                .collect::<Vec<_>>()
                .join("\n");
            let note = match arboard::Clipboard::new() {
                Ok(mut cb) => match cb.set_text(text.clone()) {
                    Ok(()) => fmt(Str::ShopCopied, &[&text.lines().count().to_string()]),
                    Err(e) => fmt(Str::ShopCopyFail, &[&e.to_string()]),
                },
                Err(e) => fmt(Str::ShopCopyFail, &[&e.to_string()]),
            };
            j.set_note(SharedString::from(note));
        });
    }
    {
        let jh = shopping.as_weak();
        shopping.on_close_clicked(move || {
            let _ = jh.unwrap().hide();
        });
    }

    // Резервные копии: окно восстановления
    let backups = BackupsWindow::new()?;
    let backup_files: Rc<RefCell<Vec<std::path::PathBuf>>> = Rc::new(RefCell::new(Vec::new()));
    {
        let files = backup_files.clone();
        let jh = backups.as_weak();
        ui.on_open_backups(move || {
            let j = jh.unwrap();
            let files_now = storage::list_backups();
            let rows: Vec<SharedString> = files_now
                .iter()
                .map(|p| {
                    SharedString::from(
                        p.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                    )
                })
                .collect();
            j.set_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
            j.set_selected(-1);
            j.set_note(SharedString::new());
            *files.borrow_mut() = files_now;
            let _ = j.show();
        });
    }
    {
        let h = ui.as_weak();
        let s = state.clone();
        let files = backup_files.clone();
        let jh = backups.as_weak();
        backups.on_restore(move |idx| {
            let ui = h.unwrap();
            let j = jh.unwrap();
            let Some(path) = files.borrow().get(idx as usize).cloned() else {
                // список набирался при открытии окна: файлы могли исчезнуть —
                // молча ничего не делаем нельзя, пользователь ждёт результат
                j.set_note(SharedString::from(tr(Str::BakStaleList)));
                return;
            };
            // страховка: текущее состояние — в бэкап, и его же можно вернуть Ctrl+Z.
            // Если страховочный бэкап создать не удалось — восстановление отменяем:
            // иначе сбой диска приведёт к безвозвратной потере текущих данных.
            if let Err(e) = storage::backup() {
                j.set_note(SharedString::from(fmt(
                    Str::BakBackupFailed,
                    &[&e.to_string()],
                )));
                return;
            }
            let db = match storage::load_database_from(&path) {
                Ok(db) => db,
                Err(e) => {
                    j.set_note(SharedString::from(fmt(Str::BakRestoreFailed, &[&e])));
                    return;
                }
            };
            let fname = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let mut st = s.borrow_mut();
            let prev = st.domain.db.clone();
            st.domain.db = db;
            // в бэкапе мог быть другой порог — контрол синхронизируем с базой
            ui.set_warn_days(st.domain.db.warn_days);
            st.selected_id = -1;
            ui.set_selected_id(-1);
            clear_photo_cache(&st);
            reset_view_flags(&ui);
            commit_history(&ui, &mut st, prev);
            refresh_all(&ui, &mut st, true);
            st.domain
                .db
                .journal_push(canonical(Str::JrnRestoredBackup), &fname, "");
            persist(&ui, &st);
            j.set_note(SharedString::from(fmt(Str::BakRestored, &[&fname])));
        });
    }
    {
        let jh = backups.as_weak();
        backups.on_close_clicked(move || {
            let _ = jh.unwrap().hide();
        });
    }

    // Импорт CSV: окно выбора файла + разбор
    let import_win = ImportWindow::new()?;
    let import_files: Rc<RefCell<Vec<std::path::PathBuf>>> = Rc::new(RefCell::new(Vec::new()));
    {
        let files = import_files.clone();
        let jh = import_win.as_weak();
        ui.on_open_import(move || {
            let j = jh.unwrap();
            let mut files_now: Vec<std::path::PathBuf> = std::fs::read_dir(storage::exe_dir())
                .into_iter()
                .flatten()
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.is_file()
                        && p.extension()
                            .is_some_and(|ext| ext.eq_ignore_ascii_case("csv"))
                })
                .collect();
            files_now.sort();
            files_now.reverse();
            let rows: Vec<SharedString> = files_now
                .iter()
                .map(|p| {
                    SharedString::from(
                        p.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                    )
                })
                .collect();
            j.set_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
            j.set_selected(-1);
            j.set_note(SharedString::new());
            *files.borrow_mut() = files_now;
            let _ = j.show();
        });
    }
    {
        let h = ui.as_weak();
        let s = state.clone();
        let files = import_files.clone();
        let jh = import_win.as_weak();
        import_win.on_do_import(move |idx| {
            let ui = h.unwrap();
            let j = jh.unwrap();
            let Some(path) = files.borrow().get(idx as usize).cloned() else {
                j.set_note(SharedString::from(tr(Str::ImportStaleList)));
                return;
            };
            let (meds, skipped) = match csv::import_csv(&path) {
                Ok(r) => r,
                Err(e) => {
                    j.set_note(SharedString::from(fmt(
                        Str::ImportReadError,
                        &[&e.to_string()],
                    )));
                    return;
                }
            };
            if meds.is_empty() {
                j.set_note(SharedString::from(tr(Str::ImportNoRows)));
                return;
            }
            let mut st = s.borrow_mut();
            // Дедупликация (ядро csv::dedup_imported): пара (имя, форма) без
            // регистра — против базы, идентичность — внутри файла. Повторный
            // импорт того же CSV не плодит дубли.
            let (kept_meds, dropped) = csv::dedup_imported(&st.domain.db.medicines, meds);
            let duplicates = dropped.len();
            let dropped_names = |limit: usize| {
                let mut s = dropped
                    .iter()
                    .take(limit)
                    .map(|m| m.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                if duplicates > limit {
                    s.push_str(", …");
                }
                s
            };
            let mut meds = kept_meds;
            if meds.is_empty() {
                let note = if duplicates > 0 {
                    fmt(
                        Str::ImportAllDup,
                        &[&duplicates.to_string(), &dropped_names(5)],
                    )
                } else {
                    tr(Str::ImportNoRows).to_string()
                };
                j.set_note(SharedString::from(note));
                return;
            }
            let prev = st.domain.db.clone();
            let mut total_pkgs = 0usize;
            for m in &mut meds {
                m.id = st.domain.db.new_med_id();
                for p in &mut m.packages {
                    p.id = st.domain.db.new_pkg_id();
                    total_pkgs += 1;
                }
                st.domain.db.journal_push(
                    canonical(Str::JrnImport),
                    &m.name,
                    &fmt(Str::JrnImportPkgs, &[&m.packages.len().to_string()]),
                );
            }
            let imported_count = meds.len();
            st.domain
                .db
                .medicines
                .extend(meds.into_iter().map(Arc::new));
            st.selected_id = -1;
            ui.set_selected_id(-1);
            clear_photo_cache(&st);
            reset_view_flags(&ui);
            commit_refresh_persist(&ui, &mut st, prev, true);
            let mut note = fmt(
                Str::ImportDone,
                &[
                    &imported_count.to_string(),
                    &total_pkgs.to_string(),
                    &skipped.to_string(),
                ],
            );
            if duplicates > 0 {
                // имена отброшенных — чтобы правки в файле не пропадали молча
                note.push_str(&fmt(
                    Str::ImportDoneDup,
                    &[&duplicates.to_string(), &dropped_names(5)],
                ));
            }
            note.push(')');
            j.set_note(SharedString::from(note));
        });
    }
    {
        let jh = import_win.as_weak();
        import_win.on_close_clicked(move || {
            let _ = jh.unwrap().hide();
        });
    }

    Ok(SecondaryWindows {
        _journal: journal,
        _reports: reports,
        _shopping: shopping,
        _backups: backups,
        _import: import_win,
    })
}
