// Локализация UI: перенос строк из apteka_core::i18n в свойства Slint (global I18n).
//
// Статические строки живут в .slint-свойствах; при старте и при смене языка
// флажком apply_i18n_global переписывает их на выбранный язык. Динамические
// строки (сводка, статусы, заголовки) пересобираются refresh_all.

use apteka_core::i18n::{fmt, lang, tr, Lang, Str};
use apteka_core::settings::AppSettings;
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::rc::Rc;

use crate::{AppWindow, I18n, SecondaryWindows};

fn text(key: Str) -> SharedString {
    SharedString::from(tr(key))
}

/// Переписывает все строки глобала I18n на текущий язык. Один вызов — для
/// каждого окна (у каждого компонента свой экземпляр глобала).
pub(crate) fn apply_i18n_global(g: &I18n) {
    // шрифт: CJK требует конкретного семейства, иначе системный фолбэк
    // может подобрать шрифт без нужных иероглифов (проверено на zh)
    g.set_font_family(SharedString::from(match lang() {
        Lang::Zh => "Microsoft YaHei",
        Lang::Ja => "Yu Gothic UI",
        _ => "Segoe UI",
    }));

    g.set_t_app_title(text(Str::AppTitle));
    g.set_t_close(text(Str::Close));
    g.set_t_cancel(text(Str::Cancel));
    g.set_t_lang_label(text(Str::LangLabel));

    g.set_t_app_header(text(Str::AppHeader));
    g.set_t_search_placeholder(text(Str::SearchPlaceholder));
    g.set_filter_options(str_model(vec![
        Str::FilterAll,
        Str::FilterSoon,
        Str::FilterExpired,
    ]));
    g.set_sort_options(str_model(vec![Str::SortExpiry, Str::SortName]));
    g.set_t_add_medicine(text(Str::AddMedicine));
    g.set_t_list_header(text(Str::ListHeader));
    g.set_t_all_categories(text(Str::AllCategories));
    g.set_t_list_empty(text(Str::ListEmpty));
    g.set_t_list_no_results(text(Str::ListNoResults));

    g.set_t_detail_pick_hint(text(Str::DetailPickHint));
    g.set_t_detail_hidden(text(Str::DetailHiddenWarning));
    g.set_t_field_name(text(Str::FieldName));
    g.set_t_field_form(text(Str::FieldForm));
    g.set_t_field_actions(text(Str::FieldActions));
    g.set_t_field_notes(text(Str::FieldNotes));
    g.set_t_field_tags(text(Str::FieldTags));
    g.set_t_tags_placeholder(text(Str::TagsPlaceholder));
    g.set_t_delete_medicine(text(Str::DeleteMedicine));
    g.set_t_delete_confirm(text(Str::DeleteConfirm));
    g.set_t_packages_header(text(Str::PackagesHeader));
    g.set_t_packages_hint(text(Str::PackagesHint));
    g.set_t_show_disposed(text(Str::ShowDisposed));
    g.set_t_hide_disposed(text(Str::HideDisposed));
    g.set_t_packages_empty(text(Str::PackagesEmpty));

    g.set_t_photo_missing(text(Str::PhotoMissing));
    g.set_t_pkg_open(text(Str::PkgOpen));
    g.set_t_pkg_open_confirm(text(Str::PkgOpenConfirm));
    g.set_t_pkg_used(text(Str::PkgUsed));
    g.set_t_pkg_discard(text(Str::PkgDiscard));
    g.set_t_pkg_clone(text(Str::PkgClone));
    g.set_t_pkg_edit(text(Str::PkgEdit));
    g.set_t_pkg_restore(text(Str::PkgRestore));
    g.set_t_pkg_delete(text(Str::PkgDelete));
    g.set_t_pkg_delete_forever(text(Str::PkgDeleteForever));

    g.set_t_form_new(text(Str::FormNew));
    g.set_t_form_new_photo(text(Str::FormNewPhoto));
    g.set_t_form_edit(text(Str::FormEdit));
    g.set_t_field_expiry(text(Str::FieldExpiry));
    g.set_t_field_quantity(text(Str::FieldQuantity));
    g.set_t_field_price(text(Str::FieldPrice));
    g.set_t_field_period(text(Str::FieldPeriod));
    g.set_t_field_purchased(text(Str::FieldPurchased));
    g.set_t_field_comment(text(Str::FieldComment));
    g.set_t_expiry_placeholder(text(Str::ExpiryPlaceholder));
    g.set_t_quantity_placeholder(text(Str::QuantityPlaceholder));
    g.set_t_price_placeholder(text(Str::PricePlaceholder));
    g.set_t_period_placeholder(text(Str::PeriodPlaceholder));
    g.set_t_optional_placeholder(text(Str::OptionalPlaceholder));
    g.set_t_quick_label(text(Str::QuickLabel));
    g.set_t_quick_6m(text(Str::Quick6m));
    g.set_t_quick_1y(text(Str::Quick1y));
    g.set_t_quick_2y(text(Str::Quick2y));
    g.set_t_photo_btn(text(Str::PhotoBtn));
    g.set_t_save_pkg(text(Str::SavePkg));
    g.set_t_add_pkg(text(Str::AddPkg));

    g.set_week_days(str_model(vec![
        Str::WeekMon,
        Str::WeekTue,
        Str::WeekWed,
        Str::WeekThu,
        Str::WeekFri,
        Str::WeekSat,
        Str::WeekSun,
    ]));

    g.set_t_warn_prefix(text(Str::WarnPrefix));
    g.set_t_warn_suffix(text(Str::WarnSuffix));
    g.set_t_undo(text(Str::Undo));
    g.set_t_redo(text(Str::Redo));
    g.set_t_journal(text(Str::Journal));
    g.set_t_shopping(text(Str::Shopping));
    g.set_t_reports(text(Str::Reports));
    g.set_t_backups(text(Str::Backups));
    g.set_t_import_csv(text(Str::ImportCsv));
    g.set_t_export_csv(text(Str::ExportCsv));
    g.set_t_theme(text(Str::Theme));

    g.set_t_summary_total(text(Str::SummaryTotal));
    g.set_t_summary_soon(text(Str::SummarySoon));
    g.set_t_summary_expired(text(Str::SummaryExpired));
    g.set_t_summary_cost(text(Str::SummaryCost));

    g.set_t_reports_title(text(Str::ReportsTitle));
    g.set_t_rep_empty(text(Str::RepEmpty));
    g.set_t_journal_title(text(Str::JournalTitle));
    g.set_t_jrn_empty(text(Str::JrnEmpty));
    g.set_t_shopping_title(text(Str::ShoppingTitle));
    g.set_t_shop_empty(text(Str::ShopEmpty));
    g.set_t_shop_copy(text(Str::ShopCopy));
    g.set_t_backups_title(text(Str::BackupsTitle));
    g.set_t_bak_hint(text(Str::BakHint));
    g.set_t_bak_empty(text(Str::BakEmpty));
    g.set_t_bak_restore(text(Str::BakRestore));
    g.set_t_imp_title(text(Str::ImportTitle));
    g.set_t_imp_hint(text(Str::ImportHint));
    g.set_t_imp_empty(text(Str::ImportEmpty));
    g.set_t_imp_import(text(Str::ImportButton));
}

fn str_model(keys: Vec<Str>) -> ModelRc<SharedString> {
    ModelRc::from(Rc::new(VecModel::from(
        keys.into_iter()
            .map(|k| SharedString::from(tr(k)))
            .collect::<Vec<_>>(),
    )))
}

/// Применяет язык ко всем окнам приложения (главному и вторичным).
pub(crate) fn apply_i18n(ui: &AppWindow, windows: &SecondaryWindows) {
    apply_i18n_global(&ui.global::<I18n>());
    windows.apply_i18n();
}

/// Язык при старте: явный выбор из settings.json → системная локаль → русский.
pub(crate) fn initial_lang(settings: &AppSettings) -> Lang {
    if let Some(l) = Lang::from_code(settings.language.trim()) {
        return l;
    }
    detect_system_lang().unwrap_or(Lang::Ru)
}

/// Язык системной локали (для первого запуска). `APTEKA_LANG` — переопределение
/// для отладки и скриншотов. На Windows — локаль пользователя, иначе $LANG.
pub(crate) fn detect_system_lang() -> Option<Lang> {
    if let Ok(code) = std::env::var("APTEKA_LANG") {
        if let Some(l) = Lang::from_code(code.trim()) {
            return Some(l);
        }
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;
        let mut buf = [0u16; 85];
        let len = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), buf.len() as i32) };
        if len > 0 {
            let locale = String::from_utf16_lossy(&buf[..(len as usize - 1)]);
            return Lang::from_locale(&locale);
        }
        None
    }
    #[cfg(not(windows))]
    {
        std::env::var("LANG")
            .ok()
            .and_then(|v| Lang::from_locale(&v))
    }
}

/// Служебная строка «данные + бэкап» статусной строки при старте.
pub(crate) fn startup_status(data_path: &str, backup_note: &str) -> String {
    fmt(Str::StatusData, &[data_path]) + backup_note
}

/// То же при предупреждении о повреждённом файле.
pub(crate) fn startup_status_warn(warning: &str, data_path: &str, backup_note: &str) -> String {
    fmt(Str::StatusStartupWarn, &[warning, data_path, backup_note])
}

/// Имя для message box (заголовок).
pub(crate) fn msg_title() -> String {
    tr(Str::MsgTitle).to_string()
}
