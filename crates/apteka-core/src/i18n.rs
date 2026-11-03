//! Локализация интерфейса: 10 языков, выбор по флажкам (сохраняется в settings.json).
//!
//! Языки («мировые классики»): English, Deutsch, Español, Français, Italiano,
//! Português, Русский, 中文, 日本語, العربية. Русский — исходный язык приложения
//! и язык по умолчанию (тесты и данные рассчитаны на него).
//!
//! Устройство:
//! - `Str` — ключи строк; таблица `strings!` задаёт текст на всех языках сразу,
//!   пропуск языка — ошибка компиляции.
//! - `s(lang, key)` — чистая функция (для тестов), `tr(key)` — на текущем языке.
//! - `fmt(key, args)` подставляет `{0}`, `{1}`, … — набор плейсхолдеров должен
//!   совпадать на всех языках (проверяется тестом).
//! - Текущий язык — глобальный (`set_lang`/`lang`): строковые функции ядра
//!   (`status_text`, `shopping_list`, сообщения валидации) берут его отсюда.
//!   Тесты глобальный язык не меняют и работают на русском по умолчанию.
//!
//! Данные (имена лекарств, формы в базе, события журнала) хранятся в каноническом
//! виде; на экране формы показываются переводом (`form_display`), события журнала
//! переводятся при показе (`journal_kind_display`).

use std::sync::atomic::{AtomicU8, Ordering};

/// Язык интерфейса. Порядок = порядок флажков в UI.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Lang {
    En = 0,
    De,
    Es,
    Fr,
    It,
    Pt,
    Ru,
    Zh,
    Ja,
    Ar,
}

impl Lang {
    pub const ALL: [Lang; 10] = [
        Lang::En,
        Lang::De,
        Lang::Es,
        Lang::Fr,
        Lang::It,
        Lang::Pt,
        Lang::Ru,
        Lang::Zh,
        Lang::Ja,
        Lang::Ar,
    ];

    /// Код языка для settings.json и имён файлов флагов (`assets/flags/<code>.png`).
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::De => "de",
            Lang::Es => "es",
            Lang::Fr => "fr",
            Lang::It => "it",
            Lang::Pt => "pt",
            Lang::Ru => "ru",
            Lang::Zh => "zh",
            Lang::Ja => "ja",
            Lang::Ar => "ar",
        }
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        Lang::ALL.into_iter().find(|l| l.code() == code)
    }

    /// Название языка на нём самом (для подсказок).
    pub fn native_name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::De => "Deutsch",
            Lang::Es => "Español",
            Lang::Fr => "Français",
            Lang::It => "Italiano",
            Lang::Pt => "Português",
            Lang::Ru => "Русский",
            Lang::Zh => "中文",
            Lang::Ja => "日本語",
            Lang::Ar => "العربية",
        }
    }

    /// Язык из системной локали (`ru-RU`, `zh-CN`, …): сравнивается префикс.
    pub fn from_locale(locale: &str) -> Option<Lang> {
        let lower = locale.to_lowercase();
        let prefix = lower.split(['-', '_']).next().unwrap_or(&lower);
        Lang::ALL.into_iter().find(|l| l.code() == prefix)
    }
}

// ---------- Текущий язык (глобальный) ----------

static LANG: AtomicU8 = AtomicU8::new(Lang::Ru as u8);

pub fn set_lang(lang: Lang) {
    LANG.store(lang as u8, Ordering::Relaxed);
}

pub fn lang() -> Lang {
    match LANG.load(Ordering::Relaxed) {
        0 => Lang::En,
        1 => Lang::De,
        2 => Lang::Es,
        3 => Lang::Fr,
        4 => Lang::It,
        5 => Lang::Pt,
        6 => Lang::Ru,
        7 => Lang::Zh,
        8 => Lang::Ja,
        _ => Lang::Ar,
    }
}

// ---------- Таблица строк ----------

macro_rules! strings {
    ($($key:ident: {
        ru: $ru:expr, en: $en:expr, de: $de:expr, fr: $fr:expr, es: $es:expr,
        it: $it:expr, pt: $pt:expr, zh: $zh:expr, ja: $ja:expr, ar: $ar:expr
    },)*) => {
        /// Ключ строки интерфейса.
        #[derive(Copy, Clone, PartialEq, Eq, Debug)]
        #[repr(usize)]
        pub enum Str {
            $($key,)*
        }

        impl Str {
            pub const ALL: &'static [Str] = &[ $(Str::$key,)* ];
            pub const COUNT: usize = Self::ALL.len();
        }

        /// Текст строки на заданном языке (чистая функция).
        pub fn s(lang: Lang, key: Str) -> &'static str {
            match key {
                $(Str::$key => match lang {
                    Lang::Ru => $ru,
                    Lang::En => $en,
                    Lang::De => $de,
                    Lang::Fr => $fr,
                    Lang::Es => $es,
                    Lang::It => $it,
                    Lang::Pt => $pt,
                    Lang::Zh => $zh,
                    Lang::Ja => $ja,
                    Lang::Ar => $ar,
                },)*
            }
        }
    };
}

strings! {
    // ===== Общие =====
    MsgTitle: {
        ru: "Домашняя аптечка", en: "Medicine Cabinet", de: "Hausapotheke",
        fr: "Trousse de pharmacie", es: "Botiquín casero", it: "Farmacia di casa",
        pt: "Kit de medicamentos", zh: "家庭药箱", ja: "家庭の薬箱", ar: "الصيدلية المنزلية"
    },
    AppTitle: {
        ru: "Домашняя аптечка — учёт сроков годности", en: "Home Medicine Cabinet — Expiry Tracker",
        de: "Hausapotheke – Haltbarkeits-Tracker", fr: "Trousse de pharmacie — suivi des péremptions",
        es: "Botiquín casero — control de caducidad", it: "Farmacia di casa — scadenze",
        pt: "Kit de medicamentos — controle de validades", zh: "家庭药箱 — 有效期管理",
        ja: "家庭の薬箱 — 有効期限管理", ar: "الصيدلية المنزلية — تتبع تواريخ الصلاحية"
    },
    Close: {
        ru: "Закрыть", en: "Close", de: "Schließen", fr: "Fermer", es: "Cerrar",
        it: "Chiudi", pt: "Fechar", zh: "关闭", ja: "閉じる", ar: "إغلاق"
    },
    Cancel: {
        ru: "Отмена", en: "Cancel", de: "Abbrechen", fr: "Annuler", es: "Cancelar",
        it: "Annulla", pt: "Cancelar", zh: "取消", ja: "キャンセル", ar: "إلغاء"
    },
    LangLabel: {
        ru: "Язык:", en: "Language:", de: "Sprache:", fr: "Langue :", es: "Idioma:",
        it: "Lingua:", pt: "Idioma:", zh: "语言：", ja: "言語：", ar: "اللغة:"
    },

    // ===== Панель инструментов и список =====
    AppHeader: {
        ru: "Аптечка", en: "Medicines", de: "Medikamente", fr: "Médicaments",
        es: "Medicamentos", it: "Farmaci", pt: "Medicamentos", zh: "药箱", ja: "お薬", ar: "الأدوية"
    },
    SearchPlaceholder: {
        ru: "Поиск: название, форма, заметки, категория…",
        en: "Search: name, form, notes, category…",
        de: "Suche: Name, Form, Notizen, Kategorie…",
        fr: "Recherche : nom, forme, notes, catégorie…",
        es: "Buscar: nombre, forma, notas, categoría…",
        it: "Cerca: nome, forma, note, categoria…",
        pt: "Buscar: nome, forma, notas, categoria…",
        zh: "搜索：名称、剂型、备注、分类…",
        ja: "検索：名前、剤型、メモ、カテゴリ…",
        ar: "بحث: الاسم، الصيغة، الملاحظات، الفئة…"
    },
    FilterAll: {
        ru: "Все лекарства", en: "All medicines", de: "Alle Medikamente", fr: "Tous les médicaments",
        es: "Todos los medicamentos", it: "Tutti i farmaci", pt: "Todos os medicamentos",
        zh: "全部药品", ja: "すべての薬", ar: "كل الأدوية"
    },
    FilterSoon: {
        ru: "Истекают скоро", en: "Expiring soon", de: "Läuft bald ab", fr: "Expirant bientôt",
        es: "Caducan pronto", it: "In scadenza", pt: "Expiram em breve", zh: "即将过期",
        ja: "期限切れ間近", ar: "قارب على الانتهاء"
    },
    FilterExpired: {
        ru: "Просроченные", en: "Expired", de: "Abgelaufen", fr: "Périmés", es: "Caducados",
        it: "Scaduti", pt: "Expirados", zh: "已过期", ja: "期限切れ", ar: "منتهية الصلاحية"
    },
    SortExpiry: {
        ru: "По ближайшему сроку", en: "By nearest expiry", de: "Nach Ablaufdatum",
        fr: "Par péremption prochaine", es: "Por caducidad más próxima", it: "Per scadenza più vicina",
        pt: "Por validade mais próxima", zh: "按最近有效期", ja: "期限が近い順", ar: "حسب أقرب صلاحية"
    },
    SortName: {
        ru: "По названию", en: "By name", de: "Nach Name", fr: "Par nom", es: "Por nombre",
        it: "Per nome", pt: "Por nome", zh: "按名称", ja: "名前順", ar: "حسب الاسم"
    },
    AddMedicine: {
        ru: "+ Добавить лекарство", en: "+ Add medicine", de: "+ Medikament hinzufügen",
        fr: "+ Ajouter un médicament", es: "+ Añadir medicamento", it: "+ Aggiungi farmaco",
        pt: "+ Adicionar medicamento", zh: "+ 添加药品", ja: "+ 薬を追加", ar: "+ إضافة دواء"
    },
    ListHeader: {
        ru: "Лекарства · ", en: "Medicines · ", de: "Medikamente · ", fr: "Médicaments · ",
        es: "Medicamentos · ", it: "Farmaci · ", pt: "Medicamentos · ", zh: "药品 · ", ja: "薬 · ",
        ar: "الأدوية · "
    },
    AllCategories: {
        ru: "Все категории", en: "All categories", de: "Alle Kategorien", fr: "Toutes les catégories",
        es: "Todas las categorías", it: "Tutte le categorie", pt: "Todas as categorias",
        zh: "全部分类", ja: "すべてのカテゴリ", ar: "كل الفئات"
    },
    ListEmpty: {
        ru: "Аптечка пуста.\nНажмите «+ Добавить лекарство».",
        en: "The cabinet is empty.\nClick “+ Add medicine”.",
        de: "Die Apotheke ist leer.\nKlicken Sie auf „+ Medikament hinzufügen“.",
        fr: "La trousse est vide.\nCliquez sur « + Ajouter un médicament ».",
        es: "El botiquín está vacío.\nPulse «+ Añadir medicamento».",
        it: "La farmacia è vuota.\nFai clic su «+ Aggiungi farmaco».",
        pt: "O kit está vazio.\nClique em “+ Adicionar medicamento”.",
        zh: "药箱为空。\n点击“+ 添加药品”。",
        ja: "薬箱は空です。\n「+ 薬を追加」を押してください。",
        ar: "الصيدلية فارغة.\nانقر فوق «+ إضافة دواء»."
    },
    ListNoResults: {
        ru: "Ничего не найдено.\nИзмените поиск или фильтр.",
        en: "Nothing found.\nChange the search or filter.",
        de: "Nichts gefunden.\nSuche oder Filter ändern.",
        fr: "Rien trouvé.\nModifiez la recherche ou le filtre.",
        es: "No se encontró nada.\nCambie la búsqueda o el filtro.",
        it: "Nessun risultato.\nModifica la ricerca o il filtro.",
        pt: "Nada encontrado.\nAltere a busca ou o filtro.",
        zh: "未找到结果。\n请修改搜索或筛选条件。",
        ja: "見つかりませんでした。\n検索またはフィルターを変更してください。",
        ar: "لم يتم العثور على نتائج.\nغيّر البحث أو المرشح."
    },

    // ===== Панель деталей =====
    DetailPickHint: {
        ru: "Выберите лекарство в списке\nили нажмите «+ Добавить лекарство»",
        en: "Select a medicine in the list\nor click “+ Add medicine”",
        de: "Wählen Sie ein Medikament in der Liste\noder klicken Sie auf „+ Medikament hinzufügen“",
        fr: "Sélectionnez un médicament dans la liste\nou cliquez sur « + Ajouter un médicament »",
        es: "Seleccione un medicamento de la lista\no pulse «+ Añadir medicamento»",
        it: "Seleziona un farmaco nell'elenco\no fai clic su «+ Aggiungi farmaco»",
        pt: "Selecione um medicamento na lista\nou clique em “+ Adicionar medicamento”",
        zh: "请在列表中选择药品\n或点击“+ 添加药品”",
        ja: "リストから薬を選択するか\n「+ 薬を追加」を押してください",
        ar: "اختر دواءً من القائمة\nأو انقر فوق «+ إضافة دواء»"
    },
    DetailHiddenWarning: {
        ru: "Это лекарство скрыто текущим поиском или фильтром — в списке слева его не видно",
        en: "This medicine is hidden by the current search or filter — it is not visible in the list on the left",
        de: "Dieses Medikament ist durch die aktuelle Suche oder den Filter ausgeblendet – es ist in der Liste links nicht sichtbar",
        fr: "Ce médicament est masqué par la recherche ou le filtre actuel — il n'est pas visible dans la liste à gauche",
        es: "Este medicamento está oculto por la búsqueda o el filtro actual: no se ve en la lista de la izquierda",
        it: "Questo farmaco è nascosto dalla ricerca o dal filtro attuale: non è visibile nell'elenco a sinistra",
        pt: "Este medicamento está oculto pela busca ou filtro atual — não aparece na lista à esquerda",
        zh: "该药品被当前的搜索或筛选条件隐藏，左侧列表中看不到",
        ja: "この薬は現在の検索またはフィルターで非表示になっています。左のリストには表示されません",
        ar: "هذا الدواء مخفي بسبب البحث أو المرشح الحالي — غير ظاهر في القائمة على اليسار"
    },
    FieldName: {
        ru: "Название", en: "Name", de: "Name", fr: "Nom", es: "Nombre",
        it: "Nome", pt: "Nome", zh: "名称", ja: "名前", ar: "الاسم"
    },
    FieldForm: {
        ru: "Форма", en: "Form", de: "Darreichungsform", fr: "Forme", es: "Forma",
        it: "Forma", pt: "Forma", zh: "剂型", ja: "剤型", ar: "الصيغة"
    },
    FieldActions: {
        ru: "Действия", en: "Actions", de: "Aktionen", fr: "Actions", es: "Acciones",
        it: "Azioni", pt: "Ações", zh: "操作", ja: "操作", ar: "إجراءات"
    },
    FieldNotes: {
        ru: "Заметки", en: "Notes", de: "Notizen", fr: "Notes", es: "Notas",
        it: "Note", pt: "Notas", zh: "备注", ja: "メモ", ar: "ملاحظات"
    },
    FieldTags: {
        ru: "Категории (через запятую) — попадают в поиск",
        en: "Categories (comma-separated) — included in search",
        de: "Kategorien (kommagetrennt) — werden durchsucht",
        fr: "Catégories (séparées par des virgules) — incluses dans la recherche",
        es: "Categorías (separadas por comas) — incluidas en la búsqueda",
        it: "Categorie (separate da virgola) — incluse nella ricerca",
        pt: "Categorias (separadas por vírgula) — incluídas na busca",
        zh: "分类（用逗号分隔）— 会加入搜索",
        ja: "カテゴリ（カンマ区切り）— 検索対象になります",
        ar: "الفئات (مفصولة بفواصل) — تُشمل في البحث"
    },
    TagsPlaceholder: {
        ru: "напр. жаропонижающее, детское", en: "e.g. fever relief, for children",
        de: "z. B. fiebersenkend, für Kinder", fr: "p. ex. antipyrétique, enfant",
        es: "p. ej. antipirético, infantil", it: "es. antipiretico, per bambini",
        pt: "ex.: antitérmico, infantil", zh: "例如：退烧、儿童", ja: "例：解熱剤、子供用",
        ar: "مثال: خافض حرارة، للأطفال"
    },
    DeleteMedicine: {
        ru: "Удалить лекарство", en: "Delete medicine", de: "Medikament löschen",
        fr: "Supprimer le médicament", es: "Eliminar medicamento", it: "Elimina farmaco",
        pt: "Excluir medicamento", zh: "删除药品", ja: "薬を削除", ar: "حذف الدواء"
    },
    DeleteConfirm: {
        ru: "Точно удалить?", en: "Really delete?", de: "Wirklich löschen?", fr: "Vraiment supprimer ?",
        es: "¿Eliminar de verdad?", it: "Eliminare davvero?", pt: "Excluir mesmo?",
        zh: "确定删除？", ja: "本当に削除しますか？", ar: "هل الحذف نهائي؟"
    },

    // ===== Упаковки =====
    PackagesHeader: {
        ru: "Упаковки", en: "Packages", de: "Packungen", fr: "Boîtes", es: "Envases",
        it: "Confezioni", pt: "Embalagens", zh: "包装", ja: "パッケージ", ar: "العبوات"
    },
    PackagesHint: {
        ru: "у одного лекарства может быть несколько упаковок с разными сроками",
        en: "one medicine can have several packages with different expiry dates",
        de: "ein Medikament kann mehrere Packungen mit unterschiedlichen Ablaufdaten haben",
        fr: "un médicament peut avoir plusieurs boîtes avec des dates de péremption différentes",
        es: "un medicamento puede tener varios envases con fechas de caducidad distintas",
        it: "un farmaco può avere più confezioni con scadenze diverse",
        pt: "um medicamento pode ter várias embalagens com validades diferentes",
        zh: "同一种药品可以有多个不同有效期的包装",
        ja: "同じ薬に有効期限の異なるパッケージを複数登録できます",
        ar: "قد يكون للدواء عدة عبوات بتواريخ صلاحية مختلفة"
    },
    ShowDisposed: {
        ru: "Показать списанные", en: "Show disposed", de: "Zeige entsorgte", fr: "Afficher les écartés",
        es: "Mostrar descartados", it: "Mostra scartati", pt: "Mostrar descartados", zh: "显示已丢弃",
        ja: "廃棄済みを表示", ar: "إظهار المستبعدة"
    },
    HideDisposed: {
        ru: "Скрыть списанные", en: "Hide disposed", de: "Verstecke entsorgte", fr: "Masquer les écartés",
        es: "Ocultar descartados", it: "Nascondi scartati", pt: "Ocultar descartados", zh: "隐藏已丢弃",
        ja: "廃棄済みを非表示", ar: "إخفاء المستبعدة"
    },
    PhotoMissing: {
        ru: "фото\nнедоступно.\nКлик —\nзаменить", en: "photo\nunavailable.\nClick to\nreplace",
        de: "Foto\nnicht\nverfügbar.\nKlicken zum\nErsetzen", fr: "photo\nindisponible.\nCliquez pour\nremplacer",
        es: "foto no\ndisponible.\nClic para\nreemplazar", it: "foto non\ndisponibile.\nClicca per\nsostituire",
        pt: "foto\nindisponível.\nClique para\nsubstituir", zh: "照片\n不可用。\n点击\n替换",
        ja: "写真は\n利用できません。\nクリックで\n差し替え", ar: "الصورة\nغير متاحة.\nانقر\nللاستبدال"
    },
    PkgOpen: {
        ru: "Открыть", en: "Open", de: "Öffnen", fr: "Ouvrir", es: "Abrir",
        it: "Apri", pt: "Abrir", zh: "开封", ja: "開封", ar: "فتح"
    },
    PkgOpenConfirm: {
        ru: "Точно открыть?", en: "Really open?", de: "Wirklich öffnen?", fr: "Vraiment ouvrir ?",
        es: "¿Abrir de verdad?", it: "Aprire davvero?", pt: "Abrir mesmo?", zh: "确定开封？",
        ja: "本当に開封しますか？", ar: "هل الفتح نهائي؟"
    },
    PkgUsed: {
        ru: "Израсходовано", en: "Used up", de: "Verbraucht", fr: "Épuisé", es: "Agotado",
        it: "Esaurito", pt: "Esgotado", zh: "已用完", ja: "使い切り", ar: "مستنفد"
    },
    PkgDiscard: {
        ru: "Выбросить", en: "Discard", de: "Entsorgen", fr: "Écarter", es: "Descartar",
        it: "Scarta", pt: "Descartar", zh: "丢弃", ja: "廃棄", ar: "استبعاد"
    },
    PkgClone: {
        ru: "Клонировать", en: "Clone", de: "Klonen", fr: "Dupliquer", es: "Duplicar",
        it: "Duplica", pt: "Duplicar", zh: "克隆", ja: "複製", ar: "نسخ"
    },
    PkgEdit: {
        ru: "Изменить", en: "Edit", de: "Bearbeiten", fr: "Modifier", es: "Editar",
        it: "Modifica", pt: "Editar", zh: "编辑", ja: "編集", ar: "تعديل"
    },
    PkgRestore: {
        ru: "Вернуть", en: "Restore", de: "Zurückholen", fr: "Rétablir", es: "Restaurar",
        it: "Ripristina", pt: "Restaurar", zh: "恢复", ja: "元に戻す", ar: "استعادة"
    },
    PkgDelete: {
        ru: "Удалить", en: "Delete", de: "Löschen", fr: "Supprimer", es: "Eliminar",
        it: "Elimina", pt: "Excluir", zh: "删除", ja: "削除", ar: "حذف"
    },
    PkgDeleteForever: {
        ru: "Удалить навсегда", en: "Delete forever", de: "Endgültig löschen",
        fr: "Supprimer définitivement", es: "Eliminar para siempre", it: "Elimina per sempre",
        pt: "Excluir para sempre", zh: "永久删除", ja: "完全に削除", ar: "حذف نهائيًا"
    },

    // ===== Форма упаковки =====
    FormNew: {
        ru: "Новая упаковка", en: "New package", de: "Neue Packung", fr: "Nouvelle boîte",
        es: "Nuevo envase", it: "Nuova confezione", pt: "Nova embalagem", zh: "新包装",
        ja: "新規パッケージ", ar: "عبوة جديدة"
    },
    FormNewPhoto: {
        ru: "Новая упаковка · фото прикрепится при сохранении",
        en: "New package · photo will be attached on save",
        de: "Neue Packung · Foto wird beim Speichern angehängt",
        fr: "Nouvelle boîte · la photo sera jointe à l'enregistrement",
        es: "Nuevo envase · la foto se adjuntará al guardar",
        it: "Nuova confezione · la foto sarà allegata al salvataggio",
        pt: "Nova embalagem · a foto será anexada ao salvar",
        zh: "新包装 · 保存时将附加照片",
        ja: "新規パッケージ · 保存時に写真が添付されます",
        ar: "عبوة جديدة · ستُرفق الصورة عند الحفظ"
    },
    FormEdit: {
        ru: "Изменение упаковки", en: "Edit package", de: "Packung bearbeiten", fr: "Modifier la boîte",
        es: "Editar envase", it: "Modifica confezione", pt: "Editar embalagem", zh: "编辑包装",
        ja: "パッケージを編集", ar: "تعديل العبوة"
    },
    FieldExpiry: {
        ru: "Срок годности", en: "Expiry date", de: "Haltbarkeitsdatum", fr: "Date de péremption",
        es: "Fecha de caducidad", it: "Data di scadenza", pt: "Validade", zh: "有效期",
        ja: "有効期限", ar: "تاريخ الصلاحية"
    },
    FieldQuantity: {
        ru: "Количество", en: "Quantity", de: "Menge", fr: "Quantité", es: "Cantidad",
        it: "Quantità", pt: "Quantidade", zh: "数量", ja: "数量", ar: "الكمية"
    },
    FieldPrice: {
        ru: "Цена, руб.", en: "Price", de: "Preis", fr: "Prix", es: "Precio",
        it: "Prezzo", pt: "Preço", zh: "价格", ja: "価格", ar: "السعر"
    },
    FieldPeriod: {
        ru: "После вскрытия, дн.", en: "After opening, days", de: "Nach Öffnen, Tage",
        fr: "Après ouverture, jours", es: "Tras abrir, días", it: "Dopo l'apertura, giorni",
        pt: "Após abrir, dias", zh: "开封后天数", ja: "開封後の日数", ar: "بعد الفتح، أيام"
    },
    FieldPurchased: {
        ru: "Куплена", en: "Purchased", de: "Gekauft", fr: "Acheté", es: "Comprado",
        it: "Acquistato", pt: "Comprado", zh: "购入日期", ja: "購入日", ar: "تاريخ الشراء"
    },
    FieldComment: {
        ru: "Комментарий", en: "Comment", de: "Kommentar", fr: "Commentaire", es: "Comentario",
        it: "Commento", pt: "Comentário", zh: "备注", ja: "コメント", ar: "تعليق"
    },
    ExpiryPlaceholder: {
        ru: "ГГГГ-ММ-ДД, ДД.ММ.ГГГГ или ММ.ГГГГ", en: "YYYY-MM-DD, DD.MM.YYYY or MM.YYYY",
        de: "JJJJ-MM-TT, TT.MM.JJJJ oder MM.JJJJ", fr: "AAAA-MM-JJ, JJ.MM.AAAA ou MM.AAAA",
        es: "AAAA-MM-DD, DD.MM.AAAA o MM.AAAA", it: "AAAA-MM-GG, GG.MM.AAAA o MM.AAAA",
        pt: "AAAA-MM-DD, DD.MM.AAAA ou MM.AAAA", zh: "YYYY-MM-DD、DD.MM.YYYY 或 MM.YYYY",
        ja: "YYYY-MM-DD、DD.MM.YYYY、または MM.YYYY", ar: "YYYY-MM-DD أو DD.MM.YYYY أو MM.YYYY"
    },
    QuantityPlaceholder: {
        ru: "напр. 2 уп.", en: "e.g. 2 packs", de: "z. B. 2 Pack.", fr: "p. ex. 2 boîtes",
        es: "p. ej. 2 envases", it: "es. 2 confezioni", pt: "ex.: 2 emb.", zh: "例如 2 盒",
        ja: "例：2箱", ar: "مثال: 2 عبوة"
    },
    PricePlaceholder: {
        ru: "напр. 350", en: "e.g. 350", de: "z. B. 350", fr: "p. ex. 350", es: "p. ej. 350",
        it: "es. 350", pt: "ex.: 350", zh: "例如 350", ja: "例：350", ar: "مثال: 350"
    },
    PeriodPlaceholder: {
        ru: "напр. 30", en: "e.g. 30", de: "z. B. 30", fr: "p. ex. 30", es: "p. ej. 30",
        it: "es. 30", pt: "ex.: 30", zh: "例如 30", ja: "例：30", ar: "مثال: 30"
    },
    OptionalPlaceholder: {
        ru: "необязательно", en: "optional", de: "optional", fr: "facultatif", es: "opcional",
        it: "facoltativo", pt: "opcional", zh: "可选", ja: "任意", ar: "اختياري"
    },
    QuickLabel: {
        ru: "Быстро:", en: "Quick:", de: "Schnell:", fr: "Rapide :", es: "Rápido:",
        it: "Veloce:", pt: "Rápido:", zh: "快捷：", ja: "クイック：", ar: "سريع:"
    },
    Quick6m: {
        ru: "+6 мес.", en: "+6 mo.", de: "+6 Mon.", fr: "+6 mois", es: "+6 mes.",
        it: "+6 mes.", pt: "+6 meses", zh: "+6 个月", ja: "+6か月", ar: "+6 أشهر"
    },
    Quick1y: {
        ru: "+1 год", en: "+1 yr.", de: "+1 Jahr", fr: "+1 an", es: "+1 año",
        it: "+1 anno", pt: "+1 ano", zh: "+1 年", ja: "+1年", ar: "+1 سنة"
    },
    Quick2y: {
        ru: "+2 года", en: "+2 yrs.", de: "+2 Jahre", fr: "+2 ans", es: "+2 años",
        it: "+2 anni", pt: "+2 anos", zh: "+2 年", ja: "+2年", ar: "+2 سنة"
    },
    PhotoBtn: {
        ru: "Фото…", en: "Photo…", de: "Foto…", fr: "Photo…", es: "Foto…",
        it: "Foto…", pt: "Foto…", zh: "照片…", ja: "写真…", ar: "صورة…"
    },
    SavePkg: {
        ru: "Сохранить упаковку", en: "Save package", de: "Packung speichern",
        fr: "Enregistrer la boîte", es: "Guardar envase", it: "Salva confezione",
        pt: "Salvar embalagem", zh: "保存包装", ja: "パッケージを保存", ar: "حفظ العبوة"
    },
    AddPkg: {
        ru: "Добавить упаковку", en: "Add package", de: "Packung hinzufügen", fr: "Ajouter la boîte",
        es: "Añadir envase", it: "Aggiungi confezione", pt: "Adicionar embalagem", zh: "添加包装",
        ja: "パッケージを追加", ar: "إضافة عبوة"
    },

    // ===== Статусная строка =====
    WarnPrefix: {
        ru: "«Истекает», если осталось ≤", en: "“Expiring” if ≤", de: "»Läuft ab« bei ≤",
        fr: "«Expire bientôt» si ≤", es: "«Caduca pronto» si ≤", it: "«In scadenza» se ≤",
        pt: "«Expira em breve» se ≤", zh: "“即将过期”阈值 ≤", ja: "「期限切れ間近」のしきい値 ≤",
        ar: "«قارب على الانتهاء» عند ≤"
    },
    WarnSuffix: {
        ru: "дн.", en: "days", de: "Tage", fr: "jours", es: "días",
        it: "giorni", pt: "dias", zh: "天", ja: "日", ar: "يوم"
    },
    Undo: {
        ru: "Отменить", en: "Undo", de: "Rückgängig", fr: "Annuler", es: "Deshacer",
        it: "Annulla", pt: "Desfazer", zh: "撤销", ja: "元に戻す", ar: "تراجع"
    },
    Redo: {
        ru: "Вернуть", en: "Redo", de: "Wiederherstellen", fr: "Rétablir", es: "Rehacer",
        it: "Ripeti", pt: "Refazer", zh: "重做", ja: "やり直す", ar: "إعادة"
    },
    Journal: {
        ru: "Журнал", en: "Journal", de: "Journal", fr: "Journal", es: "Registro",
        it: "Registro", pt: "Diário", zh: "日志", ja: "履歴", ar: "السجل"
    },
    Shopping: {
        ru: "Что купить", en: "Shopping list", de: "Einkaufsliste", fr: "Liste de courses",
        es: "Lista de la compra", it: "Lista della spesa", pt: "Lista de compras", zh: "购物清单",
        ja: "買い物リスト", ar: "قائمة التسوق"
    },
    Reports: {
        ru: "Отчёты", en: "Reports", de: "Berichte", fr: "Rapports", es: "Informes",
        it: "Report", pt: "Relatórios", zh: "报表", ja: "レポート", ar: "التقارير"
    },
    Backups: {
        ru: "Бэкапы", en: "Backups", de: "Backups", fr: "Sauvegardes", es: "Copias de seguridad",
        it: "Backup", pt: "Backups", zh: "备份", ja: "バックアップ", ar: "النسخ الاحتياطية"
    },
    ImportCsv: {
        ru: "Импорт CSV", en: "Import CSV", de: "CSV importieren", fr: "Importer CSV",
        es: "Importar CSV", it: "Importa CSV", pt: "Importar CSV", zh: "导入 CSV",
        ja: "CSV インポート", ar: "استيراد CSV"
    },
    ExportCsv: {
        ru: "Экспорт CSV", en: "Export CSV", de: "CSV exportieren", fr: "Exporter CSV",
        es: "Exportar CSV", it: "Esporta CSV", pt: "Exportar CSV", zh: "导出 CSV",
        ja: "CSV エクスポート", ar: "تصدير CSV"
    },
    Theme: {
        ru: "Тема", en: "Theme", de: "Theme", fr: "Thème", es: "Tema",
        it: "Tema", pt: "Tema", zh: "主题", ja: "テーマ", ar: "المظهر"
    },

    // ===== Сводка и строки списка =====
    SummaryTotal: {
        ru: "Лекарств: {0} · Упаковок: {1}", en: "Medicines: {0} · Packages: {1}",
        de: "Medikamente: {0} · Packungen: {1}", fr: "Médicaments : {0} · Boîtes : {1}",
        es: "Medicamentos: {0} · Envases: {1}", it: "Farmaci: {0} · Confezioni: {1}",
        pt: "Medicamentos: {0} · Embalagens: {1}", zh: "药品：{0} · 包装：{1}",
        ja: "薬：{0} · パッケージ：{1}", ar: "الأدوية: {0} · العبوات: {1}"
    },
    SummarySoon: {
        ru: "Истекают (≤{0} дн.): {1}", en: "Expiring (≤{0} d.): {1}", de: "Läuft ab (≤{0} T.): {1}",
        fr: "Expirant (≤{0} j.) : {1}", es: "Caducan (≤{0} d.): {1}", it: "In scadenza (≤{0} g.): {1}",
        pt: "Expiram (≤{0} d.): {1}", zh: "即将过期（≤{0} 天）：{1}", ja: "期限切れ間近（≤{0}日）：{1}",
        ar: "قارب على الانتهاء (≤{0} يوم): {1}"
    },
    SummaryExpired: {
        ru: "Просрочено: {0}", en: "Expired: {0}", de: "Abgelaufen: {0}", fr: "Périmés : {0}",
        es: "Caducados: {0}", it: "Scaduti: {0}", pt: "Expirados: {0}", zh: "已过期：{0}",
        ja: "期限切れ：{0}", ar: "منتهية الصلاحية: {0}"
    },
    SummaryCost: {
        ru: "Стоимость: {0} руб.", en: "Value: {0} ₽", de: "Wert: {0} ₽", fr: "Valeur : {0} ₽",
        es: "Valor: {0} ₽", it: "Valore: {0} ₽", pt: "Valor: {0} ₽", zh: "总价值：{0} ₽",
        ja: "総額：{0} ₽", ar: "القيمة: {0} ₽"
    },
    SummaryCostNone: {
        ru: "Стоимость: —", en: "Value: —", de: "Wert: —", fr: "Valeur : —", es: "Valor: —",
        it: "Valore: —", pt: "Valor: —", zh: "总价值：—", ja: "総額：—", ar: "القيمة: —"
    },
    PriceValue: {
        ru: "{0} руб.", en: "{0} ₽", de: "{0} ₽", fr: "{0} ₽", es: "{0} ₽",
        it: "{0} ₽", pt: "{0} ₽", zh: "{0} ₽", ja: "{0} ₽", ar: "{0} ₽"
    },
    MedNoPackages: {
        ru: "нет упаковок — добавьте первую", en: "no packages — add the first one",
        de: "keine Packungen – fügen Sie die erste hinzu", fr: "aucune boîte — ajoutez la première",
        es: "sin envases: añada el primero", it: "nessuna confezione: aggiungi la prima",
        pt: "sem embalagens — adicione a primeira", zh: "暂无包装 — 请添加第一个",
        ja: "パッケージなし — 最初のものを追加してください", ar: "لا توجد عبوات — أضف الأولى"
    },
    MedAllDisposed: {
        ru: "активных упаковок нет (списано: {0})", en: "no active packages (disposed: {0})",
        de: "keine aktiven Packungen (entsorgt: {0})", fr: "aucune boîte active (écartées : {0})",
        es: "sin envases activos (descartados: {0})", it: "nessuna confezione attiva (scartate: {0})",
        pt: "sem embalagens ativas (descartadas: {0})", zh: "无有效包装（已丢弃：{0}）",
        ja: "有効なパッケージなし（廃棄済み：{0}）", ar: "لا توجد عبوات نشطة (مستبعدة: {0})"
    },
    MedNearest: {
        ru: "ближайший срок: {0} · упаковок: {1} · {2}", en: "nearest expiry: {0} · packages: {1} · {2}",
        de: "nächstes Ablaufdatum: {0} · Packungen: {1} · {2}",
        fr: "péremption la plus proche : {0} · boîtes : {1} · {2}",
        es: "caducidad más próxima: {0} · envases: {1} · {2}",
        it: "scadenza più vicina: {0} · confezioni: {1} · {2}",
        pt: "validade mais próxima: {0} · embalagens: {1} · {2}",
        zh: "最近有效期：{0} · 包装：{1} · {2}", ja: "最寄りの期限：{0} · パッケージ：{1} · {2}",
        ar: "أقرب صلاحية: {0} · العبوات: {1} · {2}"
    },

    // ===== Статусы, дни, причины списания =====
    StExpired: {
        ru: "просрочено {0} дн. назад", en: "expired {0} d. ago", de: "vor {0} T. abgelaufen",
        fr: "périmé depuis {0} j.", es: "caducó hace {0} d.", it: "scaduto {0} g. fa",
        pt: "expirado há {0} d.", zh: "已过期 {0} 天", ja: "{0}日前に期限切れ",
        ar: "انتهت الصلاحية منذ {0} يوم"
    },
    StToday: {
        ru: "истекает сегодня", en: "expires today", de: "läuft heute ab", fr: "expire aujourd'hui",
        es: "caduca hoy", it: "scade oggi", pt: "expira hoje", zh: "今天过期", ja: "本日期限切れ",
        ar: "تنتهي اليوم"
    },
    StSoon: {
        ru: "истекает через {0} дн.", en: "expires in {0} d.", de: "läuft in {0} T. ab",
        fr: "expire dans {0} j.", es: "caduca en {0} d.", it: "scade tra {0} g.",
        pt: "expira em {0} d.", zh: "{0} 天后过期", ja: "{0}日後に期限切れ", ar: "تنتهي خلال {0} يوم"
    },
    StOk: {
        ru: "годен ещё {0} дн.", en: "good for {0} d.", de: "noch {0} T. haltbar",
        fr: "bon pour {0} j.", es: "válido {0} d. más", it: "buono per altri {0} g.",
        pt: "válido por mais {0} d.", zh: "还有 {0} 天有效", ja: "あと{0}日有効",
        ar: "صالح لمدة {0} يوم أخرى"
    },
    StDisposed: {
        ru: "списано", en: "disposed", de: "entsorgt", fr: "écarté", es: "descartado",
        it: "scartato", pt: "descartado", zh: "已丢弃", ja: "廃棄済み", ar: "مستبعد"
    },
    StEmpty: {
        ru: "нет упаковок", en: "no packages", de: "keine Packungen", fr: "aucune boîte",
        es: "sin envases", it: "nessuna confezione", pt: "sem embalagens", zh: "暂无包装",
        ja: "パッケージなし", ar: "لا توجد عبوات"
    },
    DaysExpired: {
        ru: "-{0} дн.", en: "-{0} d.", de: "-{0} T.", fr: "-{0} j.", es: "-{0} d.",
        it: "-{0} g.", pt: "-{0} d.", zh: "-{0} 天", ja: "-{0}日", ar: "-{0} يوم"
    },
    DaysToday: {
        ru: "сегодня", en: "today", de: "heute", fr: "aujourd'hui", es: "hoy",
        it: "oggi", pt: "hoje", zh: "今天", ja: "今日", ar: "اليوم"
    },
    DaysN: {
        ru: "{0} дн.", en: "{0} d.", de: "{0} T.", fr: "{0} j.", es: "{0} d.",
        it: "{0} g.", pt: "{0} d.", zh: "{0} 天", ja: "{0}日", ar: "{0} يوم"
    },
    DaysNone: {
        ru: "—", en: "—", de: "—", fr: "—", es: "—", it: "—", pt: "—", zh: "—", ja: "—", ar: "—"
    },
    ReasonUsed: {
        ru: "израсходовано", en: "used up", de: "verbraucht", fr: "épuisé", es: "agotado",
        it: "esaurito", pt: "esgotado", zh: "已用完", ja: "使い切り", ar: "مستنفد"
    },
    ReasonDiscarded: {
        ru: "выброшено", en: "discarded", de: "entsorgt", fr: "écarté", es: "descartado",
        it: "scartato", pt: "descartado", zh: "已丢弃", ja: "廃棄", ar: "مستبعد"
    },

    // ===== Список покупок =====
    ShopExpired: {
        ru: "ПРОСРОЧЕНО на {0} дн.", en: "EXPIRED {0} d. ago", de: "ABGELAUFEN vor {0} T.",
        fr: "PÉRIMÉ depuis {0} j.", es: "CADUCADO hace {0} d.", it: "SCADUTO {0} g. fa",
        pt: "EXPIRADO há {0} d.", zh: "已过期 {0} 天", ja: "期限切れ（{0}日前）",
        ar: "منتهية منذ {0} يوم"
    },
    ShopSoon: {
        ru: "осталось {0} дн.", en: "{0} d. left", de: "noch {0} T.", fr: "reste {0} j.",
        es: "quedan {0} d.", it: "mancano {0} g.", pt: "faltam {0} d.", zh: "剩余 {0} 天",
        ja: "残り{0}日", ar: "تبقّى {0} يوم"
    },
    ShopRow: {
        ru: "{0} ({1}): {2} — {3}, годен до {4}", en: "{0} ({1}): {2} — {3}, good until {4}",
        de: "{0} ({1}): {2} — {3}, haltbar bis {4}", fr: "{0} ({1}) : {2} — {3}, bon jusqu'au {4}",
        es: "{0} ({1}): {2} — {3}, válido hasta {4}", it: "{0} ({1}): {2} — {3}, buono fino al {4}",
        pt: "{0} ({1}): {2} — {3}, válido até {4}", zh: "{0}（{1}）：{2} — {3}，有效期至 {4}",
        ja: "{0}（{1}）：{2} — {3}、{4}まで有効", ar: "{0} ({1}): {2} — {3}، صالح حتى {4}"
    },
    ShopNoForm: {
        ru: "без формы", en: "no form", de: "ohne Form", fr: "sans forme", es: "sin forma",
        it: "senza forma", pt: "sem forma", zh: "无剂型", ja: "剤型なし", ar: "بدون صيغة"
    },
    ShopPkgCount: {
        ru: "упаковка", en: "package", de: "Packung", fr: "boîte", es: "envase",
        it: "confezione", pt: "embalagem", zh: "包装", ja: "パッケージ", ar: "عبوة"
    },

    // ===== Таблица упаковок =====
    DisposedCell: {
        ru: "списано {0} ({1})", en: "disposed {0} ({1})", de: "entsorgt {0} ({1})",
        fr: "écarté {0} ({1})", es: "descartado {0} ({1})", it: "scartato {0} ({1})",
        pt: "descartado {0} ({1})", zh: "已丢弃 {0}（{1}）", ja: "廃棄済み {0}（{1}）",
        ar: "مستبعد {0} ({1})"
    },
    OpenedWithPeriod: {
        ru: "открыта {0} · годен до {1}", en: "opened {0} · good until {1}",
        de: "geöffnet {0} · haltbar bis {1}", fr: "ouverte le {0} · bon jusqu'au {1}",
        es: "abierta el {0} · válido hasta {1}", it: "aperta il {0} · buona fino al {1}",
        pt: "aberta em {0} · válido até {1}", zh: "开封 {0} · 有效期至 {1}", ja: "開封 {0} · {1}まで有効",
        ar: "فُتحت في {0} · صالحة حتى {1}"
    },
    OpenedNoPeriod: {
        ru: "открыта {0} · без периода после вскрытия",
        en: "opened {0} · no period after opening",
        de: "geöffnet {0} · keine Frist nach Öffnen",
        fr: "ouverte le {0} · sans période après ouverture",
        es: "abierta el {0} · sin período tras abrir",
        it: "aperta il {0} · senza periodo dopo l'apertura",
        pt: "aberta em {0} · sem período após abrir",
        zh: "开封 {0} · 无开封后期限", ja: "開封 {0} · 開封後の期限なし", ar: "فُتحت في {0} · بدون فترة بعد الفتح"
    },
    PackagesCount: {
        ru: "Упаковки ({0})", en: "Packages ({0})", de: "Packungen ({0})", fr: "Boîtes ({0})",
        es: "Envases ({0})", it: "Confezioni ({0})", pt: "Embalagens ({0})", zh: "包装（{0}）",
        ja: "パッケージ（{0}）", ar: "العبوات ({0})"
    },
    PackagesCountDisposed: {
        ru: "Упаковки ({0}, списано: {1})", en: "Packages ({0}, disposed: {1})",
        de: "Packungen ({0}, entsorgt: {1})", fr: "Boîtes ({0}, écartées : {1})",
        es: "Envases ({0}, descartados: {1})", it: "Confezioni ({0}, scartate: {1})",
        pt: "Embalagens ({0}, descartadas: {1})", zh: "包装（{0}，已丢弃：{1}）",
        ja: "パッケージ（{0}、廃棄済み：{1}）", ar: "العبوات ({0}، مستبعدة: {1})"
    },
    PackagesEmpty: {
        ru: "Упаковок пока нет. Заполните форму ниже и нажмите «Добавить упаковку».",
        en: "No packages yet. Fill in the form below and click “Add package”.",
        de: "Noch keine Packungen. Füllen Sie das Formular unten aus und klicken Sie auf „Packung hinzufügen“.",
        fr: "Pas encore de boîtes. Remplissez le formulaire ci-dessous et cliquez sur « Ajouter la boîte ».",
        es: "Aún no hay envases. Rellene el formulario de abajo y pulse «Añadir envase».",
        it: "Nessuna confezione. Compila il modulo qui sotto e fai clic su «Aggiungi confezione».",
        pt: "Ainda não há embalagens. Preencha o formulário abaixo e clique em “Adicionar embalagem”.",
        zh: "暂无包装。请填写下方表单并点击“添加包装”。",
        ja: "パッケージはまだありません。下のフォームに入力して「パッケージを追加」を押してください。",
        ar: "لا توجد عبوات بعد. املأ النموذج أدناه وانقر فوق «إضافة عبوة»."
    },
    PackagesAllDisposed: {
        ru: "Все упаковки списаны ({0}). Нажмите «Показать списанные», чтобы увидеть их, или добавьте новую — форма ниже.",
        en: "All packages are disposed ({0}). Click “Show disposed” to see them, or add a new one — the form is below.",
        de: "Alle Packungen sind entsorgt ({0}). Klicken Sie auf „Zeige entsorgte“, um sie zu sehen, oder fügen Sie eine neue hinzu – das Formular ist unten.",
        fr: "Toutes les boîtes ont été écartées ({0}). Cliquez sur « Afficher les écartés » pour les voir, ou ajoutez-en une nouvelle — le formulaire est ci-dessous.",
        es: "Todos los envases están descartados ({0}). Pulse «Mostrar descartados» para verlos, o añada uno nuevo: el formulario está abajo.",
        it: "Tutte le confezioni sono state scartate ({0}). Fai clic su «Mostra scartati» per vederle, o aggiungine una nuova: il modulo è qui sotto.",
        pt: "Todas as embalagens foram descartadas ({0}). Clique em “Mostrar descartados” para vê-las ou adicione uma nova — o formulário está abaixo.",
        zh: "所有包装均已丢弃（{0}）。点击“显示已丢弃”查看，或添加新包装——表单在下方。",
        ja: "すべてのパッケージは廃棄済みです（{0}）。「廃棄済みを表示」で確認するか、新しいものを追加してください。フォームは下にあります。",
        ar: "تم استبعاد جميع العبوات ({0}). انقر فوق «إظهار المستبعدة» لمشاهدتها، أو أضف عبوة جديدة — النموذج أدناه."
    },
    PkgDesc: {
        ru: "срок {0}", en: "expires {0}", de: "Ablauf {0}", fr: "péremption {0}", es: "caduca {0}",
        it: "scadenza {0}", pt: "validade {0}", zh: "有效期 {0}", ja: "期限 {0}", ar: "الصلاحية {0}"
    },
    PkgDescExpires: {
        ru: "{0} · годен до {1}", en: "{0} · good until {1}", de: "{0} · haltbar bis {1}",
        fr: "{0} · bon jusqu'au {1}", es: "{0} · válido hasta {1}", it: "{0} · buono fino al {1}",
        pt: "{0} · válido até {1}", zh: "{0} · 有效期至 {1}", ja: "{0} · {1}まで有効", ar: "{0} · صالح حتى {1}"
    },

    // ===== Сообщения о сохранении, фото =====
    Saved: {
        ru: "Сохранено {0} · {1}", en: "Saved {0} · {1}", de: "Gespeichert {0} · {1}",
        fr: "Enregistré {0} · {1}", es: "Guardado {0} · {1}", it: "Salvato {0} · {1}",
        pt: "Salvo {0} · {1}", zh: "已保存 {0} · {1}", ja: "保存しました {0} · {1}",
        ar: "تم الحفظ {0} · {1}"
    },
    SaveError: {
        ru: "ОШИБКА сохранения: {0} — проверьте, что диск и папка доступны для записи",
        en: "SAVE ERROR: {0} — check that the disk and folder are writable",
        de: "SPEICHERFEHLER: {0} – prüfen Sie, ob Datenträger und Ordner beschreibbar sind",
        fr: "ERREUR D'ENREGISTREMENT : {0} — vérifiez que le disque et le dossier sont accessibles en écriture",
        es: "ERROR AL GUARDAR: {0} — compruebe que el disco y la carpeta se pueden escribir",
        it: "ERRORE DI SALVATAGGIO: {0} — verifica che disco e cartella siano scrivibili",
        pt: "ERRO AO SALVAR: {0} — verifique se o disco e a pasta têm permissão de escrita",
        zh: "保存错误：{0} — 请检查磁盘和文件夹是否可写",
        ja: "保存エラー：{0} — ディスクとフォルダーが書き込み可能か確認してください",
        ar: "خطأ في الحفظ: {0} — تحقق من إمكانية الكتابة على القرص والمجلد"
    },
    FormCleared: {
        ru: "Несохранённая форма упаковки очищена", en: "Unsaved package form cleared",
        de: "Ungespeichertes Packungsformular geleert", fr: "Formulaire de boîte non enregistré effacé",
        es: "Formulario de envase sin guardar borrado", it: "Modulo confezione non salvato azzerato",
        pt: "Formulário de embalagem não salvo limpo", zh: "未保存的包装表单已清空",
        ja: "未保存のパッケージフォームをクリアしました", ar: "تم مسح نموذج العبوة غير المحفوظ"
    },
    EditUndoneFormCleared: {
        ru: "Правка отменена — несохранённая форма упаковки очищена",
        en: "Edit undone — unsaved package form cleared",
        de: "Änderung rückgängig – ungescpeichertes Formular geleert",
        fr: "Modification annulée — formulaire de boîte non enregistré effacé",
        es: "Cambio deshecho — formulario de envase sin guardar borrado",
        it: "Modifica annullata — modulo confezione non salvato azzerato",
        pt: "Edição desfeita — formulário de embalagem não salvo limpo",
        zh: "已撤销编辑 — 未保存的包装表单已清空",
        ja: "編集を元に戻しました — 未保存のパッケージフォームをクリアしました",
        ar: "تم التراجع عن التعديل — تم مسح نموذج العبوة غير المحفوظ"
    },
    PhotoDialogTitle: {
        ru: "Выберите фото упаковки", en: "Select a package photo", de: "Packungsfoto auswählen",
        fr: "Choisir une photo de la boîte", es: "Seleccione una foto del envase",
        it: "Seleziona una foto della confezione", pt: "Selecione uma foto da embalagem",
        zh: "选择包装照片", ja: "パッケージ写真を選択", ar: "اختر صورة العبوة"
    },
    PhotoFilter: {
        ru: "Изображения", en: "Images", de: "Bilder", fr: "Images", es: "Imágenes",
        it: "Immagini", pt: "Imagens", zh: "图片", ja: "画像", ar: "الصور"
    },
    PhotoPkgNotFound: {
        ru: "Упаковка не найдена — фото не прикреплено", en: "Package not found — photo not attached",
        de: "Packung nicht gefunden – Foto nicht angehängt", fr: "Boîte introuvable — photo non jointe",
        es: "Envase no encontrado: foto no adjunta", it: "Confezione non trovata: foto non allegata",
        pt: "Embalagem não encontrada — foto não anexada", zh: "未找到包装 — 照片未附加",
        ja: "パッケージが見つかりません — 写真は添付されませんでした", ar: "لم يتم العثور على العبوة — لم تُرفق الصورة"
    },
    PhotoUpdated: {
        ru: "Фото упаковки обновлено", en: "Package photo updated", de: "Packungsfoto aktualisiert",
        fr: "Photo de la boîte mise à jour", es: "Foto del envase actualizada",
        it: "Foto della confezione aggiornata", pt: "Foto da embalagem atualizada", zh: "包装照片已更新",
        ja: "パッケージ写真を更新しました", ar: "تم تحديث صورة العبوة"
    },
    PhotoSame: {
        ru: "Фото упаковки уже было прикреплено", en: "This package photo was already attached",
        de: "Dieses Packungsfoto war bereits angehängt", fr: "Cette photo était déjà jointe",
        es: "Esta foto ya estaba adjunta", it: "Questa foto era già allegata",
        pt: "Esta foto já estava anexada", zh: "该包装照片此前已附加", ja: "この写真はすでに添付されています",
        ar: "كانت هذه الصورة مرفقة بالفعل"
    },
    PhotoDirError: {
        ru: "Не удалось создать папку photos: {0}", en: "Could not create the photos folder: {0}",
        de: "Ordner photos konnte nicht erstellt werden: {0}", fr: "Impossible de créer le dossier photos : {0}",
        es: "No se pudo crear la carpeta photos: {0}", it: "Impossibile creare la cartella photos: {0}",
        pt: "Não foi possível criar a pasta photos: {0}", zh: "无法创建 photos 文件夹：{0}",
        ja: "photos フォルダーを作成できませんでした：{0}", ar: "تعذّر إنشاء مجلد photos: {0}"
    },
    PhotoReadError: {
        ru: "файл не читается: {0}", en: "file cannot be read: {0}", de: "Datei kann nicht gelesen werden: {0}",
        fr: "fichier illisible : {0}", es: "el archivo no se puede leer: {0}", it: "impossibile leggere il file: {0}",
        pt: "o arquivo não pode ser lido: {0}", zh: "文件无法读取：{0}", ja: "ファイルを読み取れません：{0}",
        ar: "تعذّرت قراءة الملف: {0}"
    },
    PhotoDecodeError: {
        ru: "изображение не декодируется: {0}", en: "image cannot be decoded: {0}",
        de: "Bild kann nicht dekodiert werden: {0}", fr: "image indéchiffrable : {0}",
        es: "la imagen no se puede decodificar: {0}", it: "impossibile decodificare l'immagine: {0}",
        pt: "a imagem não pode ser decodificada: {0}", zh: "图片无法解码：{0}",
        ja: "画像をデコードできません：{0}", ar: "تعذّر فك ترميز الصورة: {0}"
    },
    PhotoSaveError: {
        ru: "Не удалось сохранить миниатюру фото: {0}", en: "Could not save the photo thumbnail: {0}",
        de: "Foto-Vorschaubild konnte nicht gespeichert werden: {0}",
        fr: "Impossible d'enregistrer la miniature de la photo : {0}",
        es: "No se pudo guardar la miniatura de la foto: {0}", it: "Impossibile salvare la miniatura della foto: {0}",
        pt: "Não foi possível salvar a miniatura da foto: {0}", zh: "无法保存照片缩略图：{0}",
        ja: "写真のサムネイルを保存できませんでした：{0}", ar: "تعذّر حفظ الصورة المصغّرة: {0}"
    },

    // ===== Лекарства: добавление/удаление =====
    NewMedicineName: {
        ru: "Новое лекарство", en: "New medicine", de: "Neues Medikament", fr: "Nouveau médicament",
        es: "Nuevo medicamento", it: "Nuovo farmaco", pt: "Novo medicamento", zh: "新药品",
        ja: "新しい薬", ar: "دواء جديد"
    },
    PlaceholderExists: {
        ru: "Уже есть пустое «Новое лекарство» — открываем его",
        en: "An empty “New medicine” already exists — opening it",
        de: "Ein leeres „Neues Medikament“ existiert bereits – wir öffnen es",
        fr: "Un « Nouveau médicament » vide existe déjà — ouverture",
        es: "Ya existe un «Nuevo medicamento» vacío: abriéndolo",
        it: "Esiste già un «Nuovo farmaco» vuoto: lo apriamo",
        pt: "Já existe um “Novo medicamento” vazio: abrindo-o",
        zh: "已存在空白的“新药品” — 正在打开",
        ja: "空の「新しい薬」が既にあります — 開きます",
        ar: "يوجد «دواء جديد» فارغ بالفعل — جارٍ فتحه"
    },
    DeletedMedicine: {
        ru: "Удалено: «{0}» — вернуть: Ctrl+Z", en: "Deleted: “{0}” — undo: Ctrl+Z",
        de: "Gelöscht: „{0}“ – rückgängig: Strg+Z", fr: "Supprimé : « {0} » — annuler : Ctrl+Z",
        es: "Eliminado: «{0}» — deshacer: Ctrl+Z", it: "Eliminato: «{0}» — annulla: Ctrl+Z",
        pt: "Excluído: “{0}” — desfazer: Ctrl+Z", zh: "已删除：“{0}” — 撤销：Ctrl+Z",
        ja: "削除しました：「{0}」— 元に戻す：Ctrl+Z", ar: "تم الحذف: «{0}» — تراجع: Ctrl+Z"
    },
    DeleteArmHint: {
        ru: "Удалить «{0}»? Нажмите Delete ещё раз (Esc — отмена)",
        en: "Delete “{0}”? Press Delete again (Esc to cancel)",
        de: "„{0}“ löschen? Drücken Sie Entf erneut (Esc zum Abbrechen)",
        fr: "Supprimer « {0} » ? Appuyez à nouveau sur Suppr (Échap pour annuler)",
        es: "¿Eliminar «{0}»? Pulse Supr de nuevo (Esc para cancelar)",
        it: "Eliminare «{0}»? Premi Canc di nuovo (Esc per annullare)",
        pt: "Excluir “{0}”? Pressione Delete novamente (Esc para cancelar)",
        zh: "删除“{0}”？再次按 Delete（Esc 取消）",
        ja: "「{0}」を削除しますか？もう一度 Delete を押してください（Esc でキャンセル）",
        ar: "حذف «{0}»؟ اضغط Delete مرة أخرى (Esc للإلغاء)"
    },
    NoPkgChanges: {
        ru: "Изменений нет — упаковка осталась прежней", en: "Nothing changed — the package is unchanged",
        de: "Keine Änderungen – die Packung bleibt unverändert", fr: "Rien n'a changé — la boîte est inchangée",
        es: "No hay cambios: el envase sigue igual", it: "Nessuna modifica: la confezione è invariata",
        pt: "Nada mudou — a embalagem continua igual", zh: "无变更 — 包装保持不变",
        ja: "変更はありません — パッケージはそのままです", ar: "لا تغييرات — العبوة بقيت كما كانت"
    },
    PkgDeleted: {
        ru: "Удалено: {0} ({1}) — вернуть: Ctrl+Z", en: "Deleted: {0} ({1}) — undo: Ctrl+Z",
        de: "Gelöscht: {0} ({1}) – rückgängig: Strg+Z", fr: "Supprimé : {0} ({1}) — annuler : Ctrl+Z",
        es: "Eliminado: {0} ({1}) — deshacer: Ctrl+Z", it: "Eliminato: {0} ({1}) — annulla: Ctrl+Z",
        pt: "Excluído: {0} ({1}) — desfazer: Ctrl+Z", zh: "已删除：{0}（{1}）— 撤销：Ctrl+Z",
        ja: "削除しました：{0}（{1}）— 元に戻す：Ctrl+Z", ar: "تم الحذف: {0} ({1}) — تراجع: Ctrl+Z"
    },
    PkgOpened: {
        ru: "Упаковка отмечена открытой сегодня — отменить: Ctrl+Z",
        en: "Package marked as opened today — undo: Ctrl+Z",
        de: "Packung als heute geöffnet markiert – rückgängig: Strg+Z",
        fr: "Boîte marquée comme ouverte aujourd'hui — annuler : Ctrl+Z",
        es: "Envase marcado como abierto hoy — deshacer: Ctrl+Z",
        it: "Confezione segnata come aperta oggi — annulla: Ctrl+Z",
        pt: "Embalagem marcada como aberta hoje — desfazer: Ctrl+Z",
        zh: "已标记为今日开封 — 撤销：Ctrl+Z",
        ja: "本日開封としてマークしました — 元に戻す：Ctrl+Z",
        ar: "تم وضع علامة «فُتحت اليوم» — تراجع: Ctrl+Z"
    },
    PkgSavedPhotoFailed: {
        ru: "Упаковка сохранена, но {0}. Прикрепить фото позже: кнопка «Изменить» → «Фото…»",
        en: "Package saved, but {0}. Attach a photo later: “Edit” → “Photo…”",
        de: "Packung gespeichert, aber {0}. Foto später anhängen: „Bearbeiten“ → „Foto…“",
        fr: "Boîte enregistrée, mais {0}. Joindre la photo plus tard : « Modifier » → « Photo… »",
        es: "Envase guardado, pero {0}. Adjunte la foto más tarde: «Editar» → «Foto…»",
        it: "Confezione salvata, ma {0}. Allega la foto più tardi: «Modifica» → «Foto…»",
        pt: "Embalagem salva, mas {0}. Anexe a foto depois: “Editar” → “Foto…”",
        zh: "包装已保存，但{0}。稍后可附加照片：“编辑” → “照片…”",
        ja: "パッケージは保存されましたが、{0}。写真は後で添付できます：「編集」→「写真…」",
        ar: "تم حفظ العبوة، لكن {0}. يمكنك إرفاق الصورة لاحقًا: «تعديل» → «صورة…»"
    },
    Exported: {
        ru: "Экспортировано: {0}", en: "Exported: {0}", de: "Exportiert: {0}", fr: "Exporté : {0}",
        es: "Exportado: {0}", it: "Esportato: {0}", pt: "Exportado: {0}", zh: "已导出：{0}",
        ja: "エクスポートしました：{0}", ar: "تم التصدير: {0}"
    },
    ExportError: {
        ru: "Ошибка экспорта: {0}. Проверьте, что файл не открыт в Excel, и что папка программы доступна для записи",
        en: "Export error: {0}. Make sure the file is not open in Excel and the program folder is writable",
        de: "Exportfehler: {0}. Prüfen Sie, ob die Datei in Excel geöffnet ist und der Programmordner beschreibbar ist",
        fr: "Erreur d'exportation : {0}. Vérifiez que le fichier n'est pas ouvert dans Excel et que le dossier du programme est accessible en écriture",
        es: "Error de exportación: {0}. Compruebe que el archivo no esté abierto en Excel y que la carpeta del programa se pueda escribir",
        it: "Errore di esportazione: {0}. Verifica che il file non sia aperto in Excel e che la cartella del programma sia scrivibile",
        pt: "Erro na exportação: {0}. Verifique se o arquivo não está aberto no Excel e se a pasta do programa é gravável",
        zh: "导出错误：{0}。请确认文件未在 Excel 中打开，且程序文件夹可写",
        ja: "エクスポートエラー：{0}。ファイルが Excel で開かれていないか、プログラムフォルダーが書き込み可能か確認してください",
        ar: "خطأ في التصدير: {0}. تأكد من أن الملف ليس مفتوحًا في Excel وأن مجلد البرنامج قابل للكتابة"
    },

    // ===== Журнал событий =====
    JournalTitle: {
        ru: "Журнал событий", en: "Event Journal", de: "Ereignisjournal", fr: "Journal des événements",
        es: "Registro de eventos", it: "Registro eventi", pt: "Diário de eventos", zh: "事件日志",
        ja: "イベント履歴", ar: "سجل الأحداث"
    },
    JrnMedAdded: {
        ru: "лекарство добавлено", en: "medicine added", de: "Medikament hinzugefügt",
        fr: "médicament ajouté", es: "medicamento añadido", it: "farmaco aggiunto",
        pt: "medicamento adicionado", zh: "已添加药品", ja: "薬を追加", ar: "تمت إضافة دواء"
    },
    JrnMedDeleted: {
        ru: "лекарство удалено", en: "medicine deleted", de: "Medikament gelöscht",
        fr: "médicament supprimé", es: "medicamento eliminado", it: "farmaco eliminato",
        pt: "medicamento excluído", zh: "已删除药品", ja: "薬を削除", ar: "تم حذف دواء"
    },
    JrnPkgAdded: {
        ru: "добавлена упаковка", en: "package added", de: "Packung hinzugefügt", fr: "boîte ajoutée",
        es: "envase añadido", it: "confezione aggiunta", pt: "embalagem adicionada", zh: "已添加包装",
        ja: "パッケージを追加", ar: "تمت إضافة عبوة"
    },
    JrnPkgChanged: {
        ru: "изменена упаковка", en: "package changed", de: "Packung geändert", fr: "boîte modifiée",
        es: "envase modificado", it: "confezione modificata", pt: "embalagem alterada", zh: "已修改包装",
        ja: "パッケージを変更", ar: "تم تعديل عبوة"
    },
    JrnPkgDeleted: {
        ru: "упаковка удалена", en: "package deleted", de: "Packung gelöscht", fr: "boîte supprimée",
        es: "envase eliminado", it: "confezione eliminata", pt: "embalagem excluída", zh: "已删除包装",
        ja: "パッケージを削除", ar: "تم حذف عبوة"
    },
    JrnPkgOpened: {
        ru: "упаковка открыта", en: "package opened", de: "Packung geöffnet", fr: "boîte ouverte",
        es: "envase abierto", it: "confezione aperta", pt: "embalagem aberta", zh: "包装已开封",
        ja: "パッケージを開封", ar: "تم فتح عبوة"
    },
    JrnPkgDisposed: {
        ru: "упаковка списана", en: "package disposed", de: "Packung entsorgt", fr: "boîte écartée",
        es: "envase descartado", it: "confezione scartata", pt: "embalagem descartada", zh: "包装已丢弃",
        ja: "パッケージを廃棄", ar: "تم استبعاد عبوة"
    },
    JrnPkgRestored: {
        ru: "упаковка возвращена в работу", en: "package restored", de: "Packung zurückgeholt",
        fr: "boîte rétablie", es: "envase restaurado", it: "confezione ripristinata",
        pt: "embalagem restaurada", zh: "包装已恢复", ja: "パッケージを復元", ar: "تمت استعادة عبوة"
    },
    JrnRestoredBackup: {
        ru: "восстановлено из бэкапа", en: "restored from backup", de: "aus Backup wiederhergestellt",
        fr: "restauré depuis la sauvegarde", es: "restaurado desde la copia de seguridad",
        it: "ripristinato dal backup", pt: "restaurado do backup", zh: "从备份恢复",
        ja: "バックアップから復元", ar: "تمت الاستعادة من النسخة الاحتياطية"
    },
    JrnImport: {
        ru: "импорт из CSV", en: "imported from CSV", de: "aus CSV importiert", fr: "importé depuis CSV",
        es: "importado desde CSV", it: "importato da CSV", pt: "importado de CSV", zh: "从 CSV 导入",
        ja: "CSV からインポート", ar: "استيراد من CSV"
    },
    JrnImportPkgs: {
        ru: "упаковок: {0}", en: "packages: {0}", de: "Packungen: {0}", fr: "boîtes : {0}",
        es: "envases: {0}", it: "confezioni: {0}", pt: "embalagens: {0}", zh: "包装：{0}",
        ja: "パッケージ：{0}", ar: "العبوات: {0}"
    },
    JrnRow: {
        ru: "{0} · {1} — {2} {3}", en: "{0} · {1} — {2} {3}", de: "{0} · {1} — {2} {3}",
        fr: "{0} · {1} — {2} {3}", es: "{0} · {1} — {2} {3}", it: "{0} · {1} — {2} {3}",
        pt: "{0} · {1} — {2} {3}", zh: "{0} · {1} — {2} {3}", ja: "{0} · {1} — {2} {3}",
        ar: "{0} · {1} — {2} {3}"
    },
    JrnTruncated: {
        ru: "Показаны последние {0} событий — более старые вытеснены.",
        en: "Showing the latest {0} events — older ones were evicted.",
        de: "Die letzten {0} Ereignisse werden angezeigt – ältere wurden verdrängt.",
        fr: "Affichage des {0} derniers événements — les plus anciens ont été évincés.",
        es: "Se muestran los últimos {0} eventos: los más antiguos se descartaron.",
        it: "Mostrati gli ultimi {0} eventi — i più vecchi sono stati eliminati.",
        pt: "Mostrando os últimos {0} eventos — os mais antigos foram descartados.",
        zh: "仅显示最近 {0} 条事件，更早的已被移除。",
        ja: "最新の{0}件のイベントを表示しています（古いものは削除済み）。",
        ar: "يتم عرض آخر {0} حدثًا — الأقدم أُزيل."
    },
    JrnEmpty: {
        ru: "Журнал пуст — события появятся по мере работы.",
        en: "The journal is empty — events will appear as you work.",
        de: "Das Journal ist leer – Ereignisse erscheinen beim Arbeiten.",
        fr: "Le journal est vide — les événements apparaîtront au fil du travail.",
        es: "El registro está vacío: los eventos aparecerán según trabaje.",
        it: "Il registro è vuoto: gli eventi appariranno man mano che lavori.",
        pt: "O diário está vazio — os eventos aparecerão conforme você trabalhar.",
        zh: "日志为空 — 事件将在使用过程中出现。",
        ja: "履歴は空です — 作業に応じてイベントが表示されます。",
        ar: "السجل فارغ — ستظهر الأحداث مع الاستخدام."
    },

    // ===== Отчёты =====
    ReportsTitle: {
        ru: "Отчёты", en: "Reports", de: "Berichte", fr: "Rapports", es: "Informes",
        it: "Report", pt: "Relatórios", zh: "报表", ja: "レポート", ar: "التقارير"
    },
    RepEmpty: {
        ru: "Данных пока нет — отметьте даты покупки и списывайте упаковки кнопкой «Выбросить».",
        en: "No data yet — record purchase dates and dispose of packages with “Discard”.",
        de: "Noch keine Daten – erfassen Sie Kaufdaten und entsorgen Sie Packungen mit „Entsorgen“.",
        fr: "Pas encore de données — indiquez les dates d'achat et écartez les boîtes avec « Écarter ».",
        es: "Aún no hay datos: registre las fechas de compra y descarte los envases con «Descartar».",
        it: "Nessun dato: registra le date di acquisto e scarta le confezioni con «Scarta».",
        pt: "Ainda não há dados — registre as datas de compra e descarte as embalagens com “Descartar”.",
        zh: "暂无数据 — 请填写购入日期，并用“丢弃”处理包装。",
        ja: "データはまだありません — 購入日を記録し、「廃棄」でパッケージを処分してください。",
        ar: "لا توجد بيانات بعد — سجّل تواريخ الشراء واستبعد العبوات بزر «استبعاد»."
    },
    RepDisposalsNone: {
        ru: "Списания: пока ни одной упаковки не списано.", en: "Disposals: no packages disposed yet.",
        de: "Entsorgungen: noch keine Packungen entsorgt.", fr: "Écartements : aucune boîte écartée pour l'instant.",
        es: "Descartes: todavía no se ha descartado ningún envase.", it: "Scarti: nessuna confezione scartata finora.",
        pt: "Descartes: nenhuma embalagem descartada ainda.", zh: "丢弃：尚未丢弃任何包装。",
        ja: "廃棄：まだパッケージの廃棄はありません。", ar: "الاستبعادات: لم تُستبعد أي عبوة بعد."
    },
    RepDisposals: {
        ru: "Списания по годам:", en: "Disposals by year:", de: "Entsorgungen nach Jahr:",
        fr: "Écartements par année :", es: "Descartes por año:", it: "Scarti per anno:",
        pt: "Descartes por ano:", zh: "按年度丢弃：", ja: "年度別廃棄：", ar: "الاستبعادات حسب السنة:"
    },
    RepPurchasesNone: {
        ru: "Покупки: даты не отмечены — заполните поле «Куплена» у упаковки.",
        en: "Purchases: no dates recorded — fill in the “Purchased” field of a package.",
        de: "Käufe: keine Daten erfasst – füllen Sie das Feld „Gekauft“ einer Packung aus.",
        fr: "Achats : dates non renseignées — remplissez le champ « Acheté » d'une boîte.",
        es: "Compras: fechas sin registrar: rellene el campo «Comprado» de un envase.",
        it: "Acquisti: date non registrate — compila il campo «Acquistato» di una confezione.",
        pt: "Compras: datas não registradas — preencha o campo “Comprado” de uma embalagem.",
        zh: "购入：尚未记录日期 — 请填写包装的“购入日期”字段。",
        ja: "購入：日付が未記録です — パッケージの「購入日」を入力してください。",
        ar: "المشتريات: التواريخ غير مسجلة — املأ حقل «تاريخ الشراء» في العبوة."
    },
    RepPurchases: {
        ru: "Покупки по годам:", en: "Purchases by year:", de: "Käufe nach Jahr:",
        fr: "Achats par année :", es: "Compras por año:", it: "Acquisti per anno:",
        pt: "Compras por ano:", zh: "按年度购入：", ja: "年度別購入：", ar: "المشتريات حسب السنة:"
    },
    RepYearRow: {
        ru: "  {0} — {1} уп. · {2} руб.{3}", en: "  {0} — {1} pcs · {2} ₽{3}",
        de: "  {0} — {1} Stk. · {2} ₽{3}", fr: "  {0} — {1} u. · {2} ₽{3}",
        es: "  {0} — {1} uds. · {2} ₽{3}", it: "  {0} — {1} pz. · {2} ₽{3}",
        pt: "  {0} — {1} un. · {2} ₽{3}", zh: "  {0} — {1} 件 · {2} ₽{3}",
        ja: "  {0} — {1}個 · {2} ₽{3}", ar: "  {0} — {1} قطعة · {2} ₽{3}"
    },
    RepTotal: {
        ru: "  Всего: {0} уп. · {1} руб.{2}", en: "  Total: {0} pcs · {1} ₽{2}",
        de: "  Gesamt: {0} Stk. · {1} ₽{2}", fr: "  Total : {0} u. · {1} ₽{2}",
        es: "  Total: {0} uds. · {1} ₽{2}", it: "  Totale: {0} pz. · {1} ₽{2}",
        pt: "  Total: {0} un. · {1} ₽{2}", zh: "  合计：{0} 件 · {1} ₽{2}",
        ja: "  合計：{0}個 · {1} ₽{2}", ar: "  الإجمالي: {0} قطعة · {1} ₽{2}"
    },
    RepTotalPurchases: {
        ru: "  Всего с датой покупки: {0} уп. · {1} руб.{2}",
        en: "  Total with purchase date: {0} pcs · {1} ₽{2}",
        de: "  Gesamt mit Kaufdatum: {0} Stk. · {1} ₽{2}",
        fr: "  Total avec date d'achat : {0} u. · {1} ₽{2}",
        es: "  Total con fecha de compra: {0} uds. · {1} ₽{2}",
        it: "  Totale con data di acquisto: {0} pz. · {1} ₽{2}",
        pt: "  Total com data de compra: {0} un. · {1} ₽{2}",
        zh: "  有购入日期合计：{0} 件 · {1} ₽{2}",
        ja: "  購入日あり合計：{0}個 · {1} ₽{2}",
        ar: "  الإجمالي بتاريخ الشراء: {0} قطعة · {1} ₽{2}"
    },
    RepUnpriced: {
        ru: " ({0} без цены)", en: " ({0} without price)", de: " ({0} ohne Preis)",
        fr: " ({0} sans prix)", es: " ({0} sin precio)", it: " ({0} senza prezzo)",
        pt: " ({0} sem preço)", zh: "（{0} 无价格）", ja: "（{0}件は価格なし）", ar: " ({0} بدون سعر)"
    },
    RepOldest: {
        ru: "Самая старая активная упаковка годна до {0}",
        en: "The oldest active package is good until {0}",
        de: "Die älteste aktive Packung ist haltbar bis {0}",
        fr: "La plus ancienne boîte active est bonne jusqu'au {0}",
        es: "El envase activo más antiguo es válido hasta {0}",
        it: "La confezione attiva più vecchia è buona fino al {0}",
        pt: "A embalagem ativa mais antiga é válida até {0}",
        zh: "最旧的有效包装有效期至 {0}", ja: "最も古い有効パッケージは{0}まで有効です",
        ar: "أقدم عبوة نشطة صالحة حتى {0}"
    },
    RepNoActive: {
        ru: "Активных упаковок нет.", en: "No active packages.", de: "Keine aktiven Packungen.",
        fr: "Aucune boîte active.", es: "No hay envases activos.", it: "Nessuna confezione attiva.",
        pt: "Sem embalagens ativas.", zh: "无有效包装。", ja: "有効なパッケージはありません。",
        ar: "لا توجد عبوات نشطة."
    },

    // ===== Окно «Что купить» =====
    ShoppingTitle: {
        ru: "Что купить", en: "Shopping list", de: "Einkaufsliste", fr: "Liste de courses",
        es: "Lista de la compra", it: "Lista della spesa", pt: "Lista de compras", zh: "购物清单",
        ja: "買い物リスト", ar: "قائمة التسوق"
    },
    ShopEmpty: {
        ru: "Покупать нечего — все сроки в порядке.", en: "Nothing to buy — all expiry dates are fine.",
        de: "Nichts zu kaufen – alle Ablaufdaten sind in Ordnung.",
        fr: "Rien à acheter — toutes les dates sont correctes.",
        es: "Nada que comprar: todas las fechas están bien.",
        it: "Niente da comprare: tutte le scadenze sono a posto.",
        pt: "Nada a comprar — todos os prazos estão ok.",
        zh: "无需购买 — 所有有效期正常。", ja: "買うものはありません — 期限はすべて良好です。",
        ar: "لا شيء للشراء — جميع التواريخ سليمة."
    },
    ShopCopy: {
        ru: "Скопировать", en: "Copy", de: "Kopieren", fr: "Copier", es: "Copiar",
        it: "Copia", pt: "Copiar", zh: "复制", ja: "コピー", ar: "نسخ"
    },
    ShopCopyEmpty: {
        ru: "Список пуст — нечего копировать.", en: "The list is empty — nothing to copy.",
        de: "Die Liste ist leer – nichts zu kopieren.", fr: "La liste est vide — rien à copier.",
        es: "La lista está vacía: no hay nada que copiar.", it: "L'elenco è vuoto: niente da copiare.",
        pt: "A lista está vazia — nada para copiar.", zh: "列表为空 — 无可复制内容。",
        ja: "リストは空です — コピーするものはありません。", ar: "القائمة فارغة — لا شيء للنسخ."
    },
    ShopCopied: {
        ru: "Скопировано в буфер обмена ({0} поз.)", en: "Copied to clipboard ({0} items)",
        de: "In die Zwischenablage kopiert ({0} Einträge)", fr: "Copié dans le presse-papiers ({0} éléments)",
        es: "Copiado al portapapeles ({0} elementos)", it: "Copiato negli appunti ({0} elementi)",
        pt: "Copiado para a área de transferência ({0} itens)", zh: "已复制到剪贴板（{0} 项）",
        ja: "クリップボードにコピーしました（{0}件）", ar: "تم النسخ إلى الحافظة ({0} عنصر)"
    },
    ShopCopyFail: {
        ru: "Не удалось скопировать: {0}", en: "Could not copy: {0}", de: "Kopieren fehlgeschlagen: {0}",
        fr: "Impossible de copier : {0}", es: "No se pudo copiar: {0}", it: "Impossibile copiare: {0}",
        pt: "Não foi possível copiar: {0}", zh: "无法复制：{0}", ja: "コピーできませんでした：{0}",
        ar: "تعذّر النسخ: {0}"
    },

    // ===== Бэкапы =====
    BackupsTitle: {
        ru: "Резервные копии", en: "Backups", de: "Backups", fr: "Sauvegardes",
        es: "Copias de seguridad", it: "Backup", pt: "Backups", zh: "备份", ja: "バックアップ",
        ar: "النسخ الاحتياطية"
    },
    BakHint: {
        ru: "Выберите копию и нажмите «Восстановить». Текущие данные перед этим будут сохранены в новый бэкап (и их можно вернуть через Ctrl+Z).",
        en: "Select a copy and click “Restore”. Current data is saved to a new backup first (undo with Ctrl+Z).",
        de: "Wählen Sie eine Kopie und klicken Sie auf „Wiederherstellen“. Die aktuellen Daten werden zuvor in einem neuen Backup gespeichert (Strg+Z macht es rückgängig).",
        fr: "Sélectionnez une copie et cliquez sur « Rétablir ». Les données actuelles sont d'abord enregistrées dans une nouvelle sauvegarde (annuler avec Ctrl+Z).",
        es: "Seleccione una copia y pulse «Restaurar». Los datos actuales se guardan antes en una nueva copia (deshacer con Ctrl+Z).",
        it: "Seleziona una copia e fai clic su «Ripristina». I dati attuali vengono prima salvati in un nuovo backup (Ctrl+Z per annullare).",
        pt: "Selecione uma cópia e clique em “Restaurar”. Os dados atuais são salvos antes em um novo backup (desfazer com Ctrl+Z).",
        zh: "选择一个备份并点击“恢复”。当前数据会先保存到新的备份（可用 Ctrl+Z 撤销）。",
        ja: "コピーを選んで「復元」を押してください。現在のデータは先に新しいバックアップへ保存されます（Ctrl+Z で元に戻せます）。",
        ar: "اختر نسخة وانقر فوق «استعادة». تُحفظ البيانات الحالية أولًا في نسخة احتياطية جديدة (تراجع بـ Ctrl+Z)."
    },
    BakEmpty: {
        ru: "Бэкапов пока нет — они создаются автоматически при каждом запуске программы.",
        en: "No backups yet — they are created automatically on every program start.",
        de: "Noch keine Backups – sie werden bei jedem Programmstart automatisch erstellt.",
        fr: "Pas encore de sauvegardes — elles sont créées automatiquement à chaque démarrage.",
        es: "Aún no hay copias: se crean automáticamente en cada inicio del programa.",
        it: "Nessun backup: vengono creati automaticamente a ogni avvio del programma.",
        pt: "Ainda não há backups — eles são criados automaticamente a cada início do programa.",
        zh: "暂无备份 — 每次启动程序时会自动创建。",
        ja: "バックアップはまだありません — プログラムの起動時に自動作成されます。",
        ar: "لا توجد نسخ احتياطية بعد — تُنشأ تلقائيًا عند كل تشغيل للبرنامج."
    },
    BakRestore: {
        ru: "Восстановить", en: "Restore", de: "Wiederherstellen", fr: "Rétablir", es: "Restaurar",
        it: "Ripristina", pt: "Restaurar", zh: "恢复", ja: "復元", ar: "استعادة"
    },
    BakStaleList: {
        ru: "Список копий устарел — закройте и заново откройте окно «Бэкапы»",
        en: "The copy list is stale — close and reopen the “Backups” window",
        de: "Die Kopienliste ist veraltet – schließen Sie das Fenster „Backups“ und öffnen Sie es erneut",
        fr: "La liste des copies est obsolète — fermez et rouvrez la fenêtre « Sauvegardes »",
        es: "La lista de copias está desactualizada: cierre y vuelva a abrir la ventana «Copias de seguridad»",
        it: "L'elenco delle copie non è aggiornato: chiudi e riapri la finestra «Backup»",
        pt: "A lista de cópias está desatualizada — feche e reabra a janela “Backups”",
        zh: "备份列表已过期 — 请关闭并重新打开“备份”窗口",
        ja: "コピー一覧が古くなっています — 「バックアップ」ウィンドウを閉じて開き直してください",
        ar: "قائمة النسخ قديمة — أغلق نافذة «النسخ الاحتياطية» وأعد فتحها"
    },
    BakBackupFailed: {
        ru: "Не удалось сохранить бэкап текущих данных ({0}) — восстановление отменено.",
        en: "Could not back up the current data ({0}) — restore cancelled.",
        de: "Aktuelle Daten konnten nicht gesichert werden ({0}) – Wiederherstellung abgebrochen.",
        fr: "Impossible de sauvegarder les données actuelles ({0}) — restauration annulée.",
        es: "No se pudo respaldar los datos actuales ({0}): restauración cancelada.",
        it: "Impossibile salvare un backup dei dati attuali ({0}): ripristino annullato.",
        pt: "Não foi possível fazer backup dos dados atuais ({0}) — restauração cancelada.",
        zh: "无法备份当前数据（{0}）— 已取消恢复。",
        ja: "現在のデータをバックアップできませんでした（{0}）— 復元を中止しました。",
        ar: "تعذّر إنشاء نسخة احتياطية للبيانات الحالية ({0}) — أُلغيت الاستعادة."
    },
    BakRestoreFailed: {
        ru: "Не удалось восстановить: {0}. Текущие данные не тронуты.",
        en: "Could not restore: {0}. Current data is untouched.",
        de: "Wiederherstellung fehlgeschlagen: {0}. Aktuelle Daten sind unverändert.",
        fr: "Impossible de restaurer : {0}. Les données actuelles n'ont pas été modifiées.",
        es: "No se pudo restaurar: {0}. Los datos actuales no se han tocado.",
        it: "Impossibile ripristinare: {0}. I dati attuali sono invariati.",
        pt: "Não foi possível restaurar: {0}. Os dados atuais não foram alterados.",
        zh: "恢复失败：{0}。当前数据未受影响。",
        ja: "復元できませんでした：{0}。現在のデータは変更されていません。",
        ar: "تعذّرت الاستعادة: {0}. البيانات الحالية لم تُمس."
    },
    BakRestored: {
        ru: "Восстановлено: {0} (отменить — Ctrl+Z)", en: "Restored: {0} (undo — Ctrl+Z)",
        de: "Wiederhergestellt: {0} (rückgängig – Strg+Z)", fr: "Restauré : {0} (annuler — Ctrl+Z)",
        es: "Restaurado: {0} (deshacer — Ctrl+Z)", it: "Ripristinato: {0} (annulla — Ctrl+Z)",
        pt: "Restaurado: {0} (desfazer — Ctrl+Z)", zh: "已恢复：{0}（撤销 — Ctrl+Z）",
        ja: "復元しました：{0}（元に戻す — Ctrl+Z）", ar: "تمت الاستعادة: {0} (تراجع — Ctrl+Z)"
    },

    // ===== Импорт/экспорт CSV =====
    ImportTitle: {
        ru: "Импорт CSV", en: "Import CSV", de: "CSV importieren", fr: "Importer CSV",
        es: "Importar CSV", it: "Importa CSV", pt: "Importar CSV", zh: "导入 CSV",
        ja: "CSV インポート", ar: "استيراد CSV"
    },
    ImportHint: {
        ru: "Импорт понимает файлы экспорта этой программы (разделитель «;»). Выберите CSV-файл рядом с программой. Данные добавятся к существующим; лекарства с такой парой «название + форма» пропускаются, поэтому повторный импорт не создаёт дублей. Жизненный цикл упаковок (вскрытие, период после вскрытия, списание) переносится; фото не переносятся.",
        en: "Import understands this program's export files (separator “;”). Select a CSV file next to the program. Data is added to the existing one; medicines with the same “name + form” pair are skipped, so re-importing does not create duplicates. Package lifecycle (opening, period after opening, disposal) is carried over; photos are not.",
        de: "Der Import versteht die Exportdateien dieses Programms (Trennzeichen „;“). Wählen Sie eine CSV-Datei neben dem Programm. Die Daten werden zu den bestehenden hinzugefügt; Medikamente mit derselben Kombination „Name + Form“ werden übersprungen, sodass ein erneuter Import keine Duplikate erzeugt. Der Lebenszyklus der Packungen (Öffnen, Frist nach Öffnen, Entsorgung) wird übernommen; Fotos nicht.",
        fr: "L'importation comprend les fichiers d'exportation de ce programme (séparateur « ; »). Choisissez un fichier CSV à côté du programme. Les données sont ajoutées aux existantes ; les médicaments avec la même paire « nom + forme » sont ignorés, donc une réimportation ne crée pas de doublons. Le cycle de vie des boîtes (ouverture, période après ouverture, écartement) est repris ; les photos ne le sont pas.",
        es: "La importación entiende los archivos de exportación de este programa (separador «;»). Seleccione un archivo CSV junto al programa. Los datos se añaden a los existentes; los medicamentos con la misma pareja «nombre + forma» se omiten, por lo que una reimportación no crea duplicados. El ciclo de vida de los envases (apertura, período tras abrir, descarte) se conserva; las fotos no.",
        it: "L'importazione comprende i file di esportazione di questo programma (separatore «;»). Scegli un file CSV accanto al programma. I dati vengono aggiunti a quelli esistenti; i farmaci con la stessa coppia «nome + forma» vengono saltati, quindi reimportare non crea duplicati. Il ciclo di vita delle confezioni (apertura, periodo dopo l'apertura, scarto) viene conservato; le foto no.",
        pt: "A importação entende os arquivos de exportação deste programa (separador “;”). Selecione um arquivo CSV ao lado do programa. Os dados são adicionados aos existentes; medicamentos com o mesmo par “nome + forma” são ignorados, então reimportar não cria duplicatas. O ciclo de vida das embalagens (abertura, período após abrir, descarte) é mantido; as fotos não.",
        zh: "导入支持本程序的导出文件（分隔符为“;”）。请选择程序目录旁的 CSV 文件。数据将合并到现有数据中；“名称 + 剂型”相同的药品会被跳过，因此重复导入不会产生重复记录。包装的生命周期（开封、开封后期限、丢弃）会保留；照片不会。",
        ja: "インポートは本プログラムのエクスポートファイル（区切り文字「;」）を読み込みます。プログラムの横にある CSV ファイルを選択してください。データは既存のものに追加され、「名前 + 剤型」が同じ薬はスキップされるため、再インポートで重複は増えません。パッケージのライフサイクル（開封、開封後の期限、廃棄）は引き継がれますが、写真は引き継がれません。",
        ar: "يستورد البرنامج ملفات التصدير الخاصة به (الفاصل «؛»). اختر ملف CSV بجانب البرنامج. ستُضاف البيانات إلى الحالية؛ وتُتخطى الأدوية ذات الزوج «الاسم + الصيغة»، لذا لا يُنشئ الاستيراد المتكرر تكرارات. تُنقل دورة حياة العبوات (الفتح، فترة ما بعد الفتح، الاستبعاد)؛ أما الصور فلا."
    },
    ImportEmpty: {
        ru: "CSV-файлы рядом с программой не найдены. Сделайте экспорт на другом компьютере и скопируйте файл сюда.",
        en: "No CSV files next to the program. Export on another computer and copy the file here.",
        de: "Keine CSV-Dateien neben dem Programm. Exportieren Sie auf einem anderen Computer und kopieren Sie die Datei hierher.",
        fr: "Aucun fichier CSV à côté du programme. Faites une exportation sur un autre ordinateur et copiez le fichier ici.",
        es: "No se encontraron archivos CSV junto al programa. Exporte en otro equipo y copie el archivo aquí.",
        it: "Nessun file CSV accanto al programma. Esporta su un altro computer e copia il file qui.",
        pt: "Nenhum arquivo CSV ao lado do programa. Faça a exportação em outro computador e copie o arquivo para cá.",
        zh: "程序旁未找到 CSV 文件。请在其他电脑上导出后将文件复制到这里。",
        ja: "プログラムの横に CSV ファイルが見つかりません。他のパソコンでエクスポートして、ファイルをここにコピーしてください。",
        ar: "لم يتم العثور على ملفات CSV بجانب البرنامج. صدّر على جهاز آخر وانسخ الملف إلى هنا."
    },
    ImportButton: {
        ru: "Импортировать", en: "Import", de: "Importieren", fr: "Importer", es: "Importar",
        it: "Importa", pt: "Importar", zh: "导入", ja: "インポート", ar: "استيراد"
    },
    ImportStaleList: {
        ru: "Список файлов устарел — закройте и заново откройте окно импорта",
        en: "The file list is stale — close and reopen the import window",
        de: "Die Dateiliste ist veraltet – schließen Sie das Importfenster und öffnen Sie es erneut",
        fr: "La liste des fichiers est obsolète — fermez et rouvrez la fenêtre d'importation",
        es: "La lista de archivos está desactualizada: cierre y vuelva a abrir la ventana de importación",
        it: "L'elenco dei file non è aggiornato: chiudi e riapri la finestra di importazione",
        pt: "A lista de arquivos está desatualizada — feche e reabra a janela de importação",
        zh: "文件列表已过期 — 请关闭并重新打开导入窗口",
        ja: "ファイル一覧が古くなっています — インポートウィンドウを閉じて開き直してください",
        ar: "قائمة الملفات قديمة — أغلق نافذة الاستيراد وأعد فتحها"
    },
    ImportReadError: {
        ru: "Ошибка чтения: {0}", en: "Read error: {0}", de: "Lesefehler: {0}",
        fr: "Erreur de lecture : {0}", es: "Error de lectura: {0}", it: "Errore di lettura: {0}",
        pt: "Erro de leitura: {0}", zh: "读取错误：{0}", ja: "読み取りエラー：{0}", ar: "خطأ في القراءة: {0}"
    },
    ImportNoRows: {
        ru: "В файле нет корректных строк.", en: "The file has no valid rows.",
        de: "Die Datei enthält keine gültigen Zeilen.", fr: "Le fichier ne contient pas de lignes valides.",
        es: "El archivo no contiene filas válidas.", it: "Il file non contiene righe valide.",
        pt: "O arquivo não contém linhas válidas.", zh: "文件中没有有效数据行。",
        ja: "ファイルに有効な行がありません。", ar: "لا يحتوي الملف على صفوف صالحة."
    },
    ImportAllDup: {
        ru: "Все лекарства из файла уже есть в аптечке (дублей: {0}): {1}.",
        en: "All medicines from the file are already in the cabinet (duplicates: {0}): {1}.",
        de: "Alle Medikamente aus der Datei sind bereits in der Apotheke (Duplikate: {0}): {1}.",
        fr: "Tous les médicaments du fichier sont déjà dans la trousse (doublons : {0}) : {1}.",
        es: "Todos los medicamentos del archivo ya están en el botiquín (duplicados: {0}): {1}.",
        it: "Tutti i farmaci del file sono già nella farmacia (duplicati: {0}): {1}.",
        pt: "Todos os medicamentos do arquivo já estão no kit (duplicatas: {0}): {1}.",
        zh: "文件中的药品均已在药箱中（重复：{0}）：{1}。",
        ja: "ファイル内の薬はすべて薬箱にあります（重複：{0}）：{1}。",
        ar: "كل أدوية الملف موجودة بالفعل في الصيدلية (مكرر: {0}): {1}."
    },
    ImportDone: {
        ru: "Импортировано лекарств: {0} (упаковок: {1}, пропущено строк: {2}",
        en: "Medicines imported: {0} (packages: {1}, rows skipped: {2}",
        de: "Medikamente importiert: {0} (Packungen: {1}, übersprungene Zeilen: {2}",
        fr: "Médicaments importés : {0} (boîtes : {1}, lignes ignorées : {2}",
        es: "Medicamentos importados: {0} (envases: {1}, filas omitidas: {2}",
        it: "Farmaci importati: {0} (confezioni: {1}, righe saltate: {2}",
        pt: "Medicamentos importados: {0} (embalagens: {1}, linhas ignoradas: {2}",
        zh: "已导入药品：{0}（包装：{1}，跳过行：{2}",
        ja: "インポートした薬：{0}（パッケージ：{1}、スキップ行：{2}",
        ar: "الأدوية المستوردة: {0} (العبوات: {1}، الصفوف المتخطاة: {2}"
    },
    ImportDoneDup: {
        ru: ", дублей пропущено: {0} ({1})", en: ", duplicates skipped: {0} ({1})",
        de: ", Duplikate übersprungen: {0} ({1})", fr: ", doublons ignorés : {0} ({1})",
        es: ", duplicados omitidos: {0} ({1})", it: ", duplicati saltati: {0} ({1})",
        pt: ", duplicatas ignoradas: {0} ({1})", zh: "，跳过重复：{0}（{1}）",
        ja: "、重複をスキップ：{0}（{1}）", ar: "، التكرارات المتخطاة: {0} ({1})"
    },

    // ===== Запуск и ошибки =====
    AlreadyRunning: {
        ru: "Домашняя аптечка уже запущена.\n\nЕсли окна не видно — посмотрите на панели задач: возможно, программа свёрнута или её окно на другом мониторе.",
        en: "Medicine Cabinet is already running.\n\nIf you don't see the window, check the taskbar: the program may be minimized or on another monitor.",
        de: "Die Hausapotheke läuft bereits.\n\nWenn Sie das Fenster nicht sehen, prüfen Sie die Taskleiste: Das Programm ist möglicherweise minimiert oder auf einem anderen Monitor.",
        fr: "La trousse de pharmacie est déjà en cours d'exécution.\n\nSi vous ne voyez pas la fenêtre, regardez dans la barre des tâches : le programme est peut-être réduit ou sur un autre écran.",
        es: "El botiquín ya se está ejecutando.\n\nSi no ve la ventana, mire la barra de tareas: el programa puede estar minimizado o en otro monitor.",
        it: "Farmacia di casa è già in esecuzione.\n\nSe non vedi la finestra, controlla la barra delle applicazioni: il programma potrebbe essere ridotto a icona o su un altro monitor.",
        pt: "O kit de medicamentos já está em execução.\n\nSe você não vê a janela, veja a barra de tarefas: o programa pode estar minimizado ou em outro monitor.",
        zh: "家庭药箱已在运行。\n\n如果看不到窗口，请查看任务栏：程序可能已最小化或在另一个显示器上。",
        ja: "家庭の薬箱はすでに起動しています。\n\nウィンドウが見えない場合はタスクバーを確認してください。最小化されているか、別のモニターにあります。",
        ar: "الصيدلية المنزلية قيد التشغيل بالفعل.\n\nإذا لم ترَ النافذة، افحص شريط المهام: قد تكون النافذة مصغّرة أو على شاشة أخرى."
    },
    CrashMsg: {
        ru: "Программа аварийно завершается.\nПодробности записаны в apteka_crash.log рядом с программой — пришлите его вместе с описанием действий, которые к этому привели.",
        en: "The program is crashing.\nDetails were written to apteka_crash.log next to the program — please send it along with a description of what led to this.",
        de: "Das Programm stürzt ab.\nDetails wurden in apteka_crash.log neben dem Programm gespeichert – bitte senden Sie diese Datei zusammen mit einer Beschreibung der Vorgeschichte.",
        fr: "Le programme se termine de manière anormale.\nLes détails ont été écrits dans apteka_crash.log à côté du programme — envoyez-le avec une description des actions qui ont conduit à cela.",
        es: "El programa se cierra de forma inesperada.\nLos detalles se han escrito en apteka_crash.log junto al programa: envíelo con una descripción de las acciones que llevaron a esto.",
        it: "Il programma si sta chiudendo in modo anomalo.\nI dettagli sono stati scritti in apteka_crash.log accanto al programma: invialo insieme a una descrizione delle azioni che hanno portato a questo.",
        pt: "O programa está travando.\nOs detalhes foram gravados em apteka_crash.log ao lado do programa — envie-o com uma descrição do que levou a isso.",
        zh: "程序异常崩溃。\n详细信息已写入程序旁的 apteka_crash.log — 请连同导致此问题的操作描述一起发送给我们。",
        ja: "プログラムが異常終了します。\n詳細はプログラムの横の apteka_crash.log に記録されました。問題に至った操作の説明とともに送信してください。",
        ar: "البرنامج يتوقف بشكل غير متوقع.\nتم تسجيل التفاصيل في apteka_crash.log بجانب البرنامج — يرجى إرساله مع وصف الإجراءات التي أدت إلى ذلك."
    },
    PanicUnknown: {
        ru: "неизвестная паника", en: "unknown panic", de: "unbekannte Panik", fr: "panique inconnue",
        es: "pánico desconocido", it: "panic sconosciuto", pt: "pânico desconhecido", zh: "未知错误",
        ja: "不明なパニック", ar: "خطأ غير معروف"
    },
    PanicPlaceUnknown: {
        ru: "место неизвестно", en: "location unknown", de: "Ort unbekannt", fr: "emplacement inconnu",
        es: "ubicación desconocida", it: "posizione sconosciuta", pt: "local desconhecido", zh: "位置未知",
        ja: "場所不明", ar: "الموقع غير معروف"
    },
    CorruptWarning: {
        ru: "Файл данных был повреждён и сохранён как {0}. Работаем с пустой базой ({1}).\n\nЕсли раньше делали «Бэкапы» — откройте их кнопкой «Бэкапы» и восстановите последнюю копию.",
        en: "The data file was corrupted and saved as {0}. Working with an empty database ({1}).\n\nIf you have used “Backups” before, open them with the “Backups” button and restore the latest copy.",
        de: "Die Datendatei war beschädigt und wurde als {0} gespeichert. Wir arbeiten mit einer leeren Datenbank ({1}).\n\nWenn Sie früher Backups erstellt haben, öffnen Sie diese über die Schaltfläche „Backups“ und stellen Sie die neueste Kopie wieder her.",
        fr: "Le fichier de données était corrompu et a été enregistré sous {0}. Nous travaillons avec une base vide ({1}).\n\nSi vous avez déjà utilisé les sauvegardes, ouvrez-les avec le bouton « Sauvegardes » et restaurez la dernière copie.",
        es: "El archivo de datos estaba dañado y se guardó como {0}. Trabajamos con una base vacía ({1}).\n\nSi antes usaba «Copias de seguridad», ábralas con el botón correspondiente y restaure la última copia.",
        it: "Il file dati era danneggiato ed è stato salvato come {0}. Lavoriamo con un database vuoto ({1}).\n\nSe in precedenza usavi i backup, aprili con il pulsante «Backup» e ripristina l'ultima copia.",
        pt: "O arquivo de dados estava corrompido e foi salvo como {0}. Estamos trabalhando com um banco vazio ({1}).\n\nSe você já usou “Backups”, abra-os pelo botão correspondente e restaure a cópia mais recente.",
        zh: "数据文件已损坏，并已另存为 {0}。当前使用空数据库（{1}）。\n\n如果之前做过“备份”，请点击“备份”按钮并恢复最近的一份。",
        ja: "データファイルが破損していたため、{0} として保存しました。空のデータベースで動作します（{1}）。\n\n以前「バックアップ」を作成した場合は、そのボタンから開いて最新のコピーを復元してください。",
        ar: "كان ملف البيانات تالفًا وحُفظ باسم {0}. نعمل بقاعدة بيانات فارغة ({1}).\n\nإذا كنت تستخدم «النسخ الاحتياطية» من قبل، افتحها بزر «النسخ الاحتياطية» واستعد آخر نسخة."
    },
    IoError: {
        ru: "Не удалось прочитать файл данных:\n{0}\n\nПрограмма закроется, чтобы не перезаписать ваши данные.\nЕсли файл занят другой программой (Excel, редактор) — закройте её и запустите аптечку снова.",
        en: "Could not read the data file:\n{0}\n\nThe program will close to avoid overwriting your data.\nIf the file is locked by another program (Excel, an editor), close it and start the cabinet again.",
        de: "Die Datendatei konnte nicht gelesen werden:\n{0}\n\nDas Programm wird beendet, um Ihre Daten nicht zu überschreiben.\nWenn die Datei von einem anderen Programm (Excel, Editor) belegt ist, schließen Sie es und starten Sie die Apotheke erneut.",
        fr: "Impossible de lire le fichier de données :\n{0}\n\nLe programme va se fermer pour ne pas écraser vos données.\nSi le fichier est utilisé par un autre programme (Excel, un éditeur), fermez-le et relancez l'application.",
        es: "No se pudo leer el archivo de datos:\n{0}\n\nEl programa se cerrará para no sobrescribir sus datos.\nSi el archivo está ocupado por otro programa (Excel, un editor), ciérrelo y vuelva a iniciar el botiquín.",
        it: "Impossibile leggere il file dati:\n{0}\n\nIl programma verrà chiuso per non sovrascrivere i tuoi dati.\nSe il file è occupato da un altro programma (Excel, un editor), chiudilo e riavvia la farmacia.",
        pt: "Não foi possível ler o arquivo de dados:\n{0}\n\nO programa será encerrado para não sobrescrever seus dados.\nSe o arquivo estiver aberto em outro programa (Excel, um editor), feche-o e inicie o kit novamente.",
        zh: "无法读取数据文件：\n{0}\n\n程序将退出以避免覆盖您的数据。\n如果文件被其他程序（Excel、编辑器）占用，请关闭它后重新启动药箱。",
        ja: "データファイルを読み取れませんでした：\n{0}\n\nデータを上書きしないよう、プログラムは終了します。\nファイルが他のプログラム（Excel やエディター）で開かれている場合は閉じてから、もう一度起動してください。",
        ar: "تعذّرت قراءة ملف البيانات:\n{0}\n\nسيُغلق البرنامج لتجنب الكتابة فوق بياناتك.\nإذا كان الملف مستخدمًا في برنامج آخر (Excel أو محرر)، أغلقه وأعد تشغيل الصيدلية."
    },
    CreateFileError: {
        ru: "Не удалось создать файл данных:\n{0}\n\nФайл: {1}",
        en: "Could not create the data file:\n{0}\n\nFile: {1}",
        de: "Die Datendatei konnte nicht erstellt werden:\n{0}\n\nDatei: {1}",
        fr: "Impossible de créer le fichier de données :\n{0}\n\nFichier : {1}",
        es: "No se pudo crear el archivo de datos:\n{0}\n\nArchivo: {1}",
        it: "Impossibile creare il file dati:\n{0}\n\nFile: {1}",
        pt: "Não foi possível criar o arquivo de dados:\n{0}\n\nArquivo: {1}",
        zh: "无法创建数据文件：\n{0}\n\n文件：{1}",
        ja: "データファイルを作成できませんでした：\n{0}\n\nファイル：{1}",
        ar: "تعذّر إنشاء ملف البيانات:\n{0}\n\nالملف: {1}"
    },
    SaveExitError: {
        ru: "Не удалось сохранить данные при выходе:\n{0}\n\nФайл: {1}",
        en: "Could not save data on exit:\n{0}\n\nFile: {1}",
        de: "Daten beim Beenden konnten nicht gespeichert werden:\n{0}\n\nDatei: {1}",
        fr: "Impossible d'enregistrer les données à la sortie :\n{0}\n\nFichier : {1}",
        es: "No se pudieron guardar los datos al salir:\n{0}\n\nArchivo: {1}",
        it: "Impossibile salvare i dati all'uscita:\n{0}\n\nFile: {1}",
        pt: "Não foi possível salvar os dados ao sair:\n{0}\n\nArquivo: {1}",
        zh: "退出时无法保存数据：\n{0}\n\n文件：{1}",
        ja: "終了時にデータを保存できませんでした：\n{0}\n\nファイル：{1}",
        ar: "تعذّر حفظ البيانات عند الخروج:\n{0}\n\nالملف: {1}"
    },
    StatusData: {
        ru: "Данные: {0}", en: "Data: {0}", de: "Daten: {0}", fr: "Données : {0}",
        es: "Datos: {0}", it: "Dati: {0}", pt: "Dados: {0}", zh: "数据：{0}", ja: "データ：{0}",
        ar: "البيانات: {0}"
    },
    StatusBackupOk: {
        ru: " · Бэкап: {0}", en: " · Backup: {0}", de: " · Backup: {0}", fr: " · Sauvegarde : {0}",
        es: " · Copia: {0}", it: " · Backup: {0}", pt: " · Backup: {0}", zh: " · 备份：{0}",
        ja: " · バックアップ：{0}", ar: " · نسخة احتياطية: {0}"
    },
    StatusBackupFail: {
        ru: " · Бэкап не удался: {0}", en: " · Backup failed: {0}", de: " · Backup fehlgeschlagen: {0}",
        fr: " · Échec de la sauvegarde : {0}", es: " · Error al respaldar: {0}",
        it: " · Backup non riuscito: {0}", pt: " · Falha no backup: {0}", zh: " · 备份失败：{0}",
        ja: " · バックアップ失敗：{0}", ar: " · فشلت النسخة الاحتياطية: {0}"
    },
    StatusStartupWarn: {
        ru: "ВНИМАНИЕ: {0} · Данные: {1}{2}", en: "ATTENTION: {0} · Data: {1}{2}",
        de: "ACHTUNG: {0} · Daten: {1}{2}", fr: "ATTENTION : {0} · Données : {1}{2}",
        es: "ATENCIÓN: {0} · Datos: {1}{2}", it: "ATTENZIONE: {0} · Dati: {1}{2}",
        pt: "ATENÇÃO: {0} · Dados: {1}{2}", zh: "注意：{0} · 数据：{1}{2}",
        ja: "警告：{0} · データ：{1}{2}", ar: "تنبيه: {0} · البيانات: {1}{2}"
    },

    // ===== Хранение =====
    FileNotFound: {
        ru: "Файл не найден: {0}", en: "File not found: {0}", de: "Datei nicht gefunden: {0}",
        fr: "Fichier introuvable : {0}", es: "Archivo no encontrado: {0}", it: "File non trovato: {0}",
        pt: "Arquivo não encontrado: {0}", zh: "文件未找到：{0}", ja: "ファイルが見つかりません：{0}",
        ar: "لم يتم العثور على الملف: {0}"
    },
    CorruptJson: {
        ru: "Повреждённый JSON: {0}", en: "Corrupted JSON: {0}", de: "Beschädigtes JSON: {0}",
        fr: "JSON corrompu : {0}", es: "JSON dañado: {0}", it: "JSON danneggiato: {0}",
        pt: "JSON corrompido: {0}", zh: "JSON 损坏：{0}", ja: "破損した JSON：{0}", ar: "JSON تالف: {0}"
    },
    CorruptQuarantineFailed: {
        ru: "файл повреждён ({0}) и его не удалось сохранить как {1} — данные НЕ тронуты",
        en: "the file is corrupted ({0}) and could not be preserved as {1} — the data is NOT touched",
        de: "die Datei ist beschädigt ({0}) und konnte nicht als {1} aufbewahrt werden – die Daten sind NICHT verändert",
        fr: "le fichier est corrompu ({0}) et n'a pas pu être préservé sous {1} — les données n'ont PAS été modifiées",
        es: "el archivo está dañado ({0}) y no se pudo preservar como {1}: los datos NO se han tocado",
        it: "il file è danneggiato ({0}) e non è stato possibile preservarlo come {1}: i dati NON sono stati toccati",
        pt: "o arquivo está corrompido ({0}) e não pôde ser preservado como {1} — os dados NÃO foram alterados",
        zh: "文件已损坏（{0}），且无法另存为 {1} — 数据未被改动",
        ja: "ファイルが破損しており（{0}）、{1} として保存できませんでした — データは変更されていません",
        ar: "الملف تالف ({0}) وتعذّر حفظه باسم {1} — البيانات لم تُمس"
    },

    // ===== Ввод формы упаковки =====
    FmtHint: {
        ru: "Формат: ГГГГ-ММ-ДД, ДД.ММ.ГГГГ или ММ.ГГГГ (годен до конца месяца)",
        en: "Format: YYYY-MM-DD, DD.MM.YYYY or MM.YYYY (good until the end of the month)",
        de: "Format: JJJJ-MM-TT, TT.MM.JJJJ oder MM.JJJJ (gültig bis Monatsende)",
        fr: "Format : AAAA-MM-JJ, JJ.MM.AAAA ou MM.AAAA (bon jusqu'à la fin du mois)",
        es: "Formato: AAAA-MM-DD, DD.MM.AAAA o MM.AAAA (válido hasta fin de mes)",
        it: "Formato: AAAA-MM-GG, GG.MM.AAAA o MM.AAAA (buono fino alla fine del mese)",
        pt: "Formato: AAAA-MM-DD, DD.MM.AAAA ou MM.AAAA (válido até o fim do mês)",
        zh: "格式：YYYY-MM-DD、DD.MM.YYYY 或 MM.YYYY（有效期至当月底）",
        ja: "形式：YYYY-MM-DD、DD.MM.YYYY、または MM.YYYY（月末まで有効）",
        ar: "الصيغة: YYYY-MM-DD أو DD.MM.YYYY أو MM.YYYY (صالح حتى نهاية الشهر)"
    },
    BadDate: {
        ru: "Неверная дата", en: "Invalid date", de: "Ungültiges Datum", fr: "Date invalide",
        es: "Fecha no válida", it: "Data non valida", pt: "Data inválida", zh: "日期无效",
        ja: "日付が無効です", ar: "تاريخ غير صالح"
    },
    PeriodError: {
        ru: "Период после вскрытия: целое число дней от 1 до 3650",
        en: "Period after opening: an integer number of days from 1 to 3650",
        de: "Frist nach Öffnen: ganze Anzahl von Tagen von 1 bis 3650",
        fr: "Période après ouverture : un nombre entier de jours de 1 à 3650",
        es: "Período tras abrir: número entero de días de 1 a 3650",
        it: "Periodo dopo l'apertura: numero intero di giorni da 1 a 3650",
        pt: "Período após abrir: número inteiro de dias de 1 a 3650",
        zh: "开封后期限：1 到 3650 之间的整数天数",
        ja: "開封後の期限：1〜3650 の整数日",
        ar: "فترة ما بعد الفتح: عدد أيام صحيح من 1 إلى 3650"
    },
    PriceError: {
        ru: "Цена: число, например 350 или 249.99", en: "Price: a number, e.g. 350 or 249.99",
        de: "Preis: eine Zahl, z. B. 350 oder 249,99", fr: "Prix : un nombre, p. ex. 350 ou 249,99",
        es: "Precio: un número, p. ej. 350 o 249,99", it: "Prezzo: un numero, ad es. 350 o 249,99",
        pt: "Preço: um número, ex.: 350 ou 249,99", zh: "价格：请输入数字，例如 350 或 249.99",
        ja: "価格：数値を入力してください（例：350 または 249.99）", ar: "السعر: رقم، مثل 350 أو 249.99"
    },
    BuyDateError: {
        ru: "Дата покупки: ГГГГ-ММ-ДД или ДД.ММ.ГГГГ (можно оставить пустым)",
        en: "Purchase date: YYYY-MM-DD or DD.MM.YYYY (can be left empty)",
        de: "Kaufdatum: JJJJ-MM-TT oder TT.MM.JJJJ (kann leer bleiben)",
        fr: "Date d'achat : AAAA-MM-JJ ou JJ.MM.AAAA (peut rester vide)",
        es: "Fecha de compra: AAAA-MM-DD o DD.MM.AAAA (puede quedar vacía)",
        it: "Data di acquisto: AAAA-MM-GG o GG.MM.AAAA (può restare vuota)",
        pt: "Data de compra: AAAA-MM-DD ou DD.MM.AAAA (pode ficar vazia)",
        zh: "购入日期：YYYY-MM-DD 或 DD.MM.YYYY（可留空）",
        ja: "購入日：YYYY-MM-DD または DD.MM.YYYY（空欄可）",
        ar: "تاريخ الشراء: YYYY-MM-DD أو DD.MM.YYYY (يمكن تركه فارغًا)"
    },
    MonthValidation: {
        ru: "месяц: годен до {0} · {1}", en: "month: good until {0} · {1}", de: "Monat: haltbar bis {0} · {1}",
        fr: "mois : bon jusqu'au {0} · {1}", es: "mes: válido hasta {0} · {1}", it: "mese: buono fino al {0} · {1}",
        pt: "mês: válido até {0} · {1}", zh: "按月：有效期至 {0} · {1}", ja: "月単位：{0}まで有効 · {1}",
        ar: "الشهر: صالح حتى {0} · {1}"
    },
    PeriodApplies: {
        ru: " · период применится после вскрытия", en: " · the period applies after opening",
        de: " · die Frist gilt nach dem Öffnen", fr: " · la période s'applique après ouverture",
        es: " · el período se aplica tras abrir", it: " · il periodo si applica dopo l'apertura",
        pt: " · o período se aplica após abrir", zh: " · 开封后才会应用期限",
        ja: " · 期限は開封後に適用されます", ar: " · تُطبَّق الفترة بعد الفتح"
    },

    // ===== CSV =====
    CsvColMedicine: {
        ru: "Лекарство", en: "Medicine", de: "Medikament", fr: "Médicament", es: "Medicamento",
        it: "Farmaco", pt: "Medicamento", zh: "药品", ja: "薬", ar: "الدواء"
    },
    CsvColForm: {
        ru: "Форма", en: "Form", de: "Form", fr: "Forme", es: "Forma", it: "Forma", pt: "Forma",
        zh: "剂型", ja: "剤型", ar: "الصيغة"
    },
    CsvColNotes: {
        ru: "Заметки", en: "Notes", de: "Notizen", fr: "Notes", es: "Notas", it: "Note", pt: "Notas",
        zh: "备注", ja: "メモ", ar: "ملاحظات"
    },
    CsvColExpiry: {
        ru: "Срок годности", en: "Expiry date", de: "Haltbarkeitsdatum", fr: "Date de péremption",
        es: "Fecha de caducidad", it: "Data di scadenza", pt: "Validade", zh: "有效期", ja: "有効期限",
        ar: "تاريخ الصلاحية"
    },
    CsvColDaysLeft: {
        ru: "Дней до конца", en: "Days left", de: "Tage übrig", fr: "Jours restants",
        es: "Días restantes", it: "Giorni rimanenti", pt: "Dias restantes", zh: "剩余天数",
        ja: "残り日数", ar: "الأيام المتبقية"
    },
    CsvColStatus: {
        ru: "Статус", en: "Status", de: "Status", fr: "Statut", es: "Estado", it: "Stato",
        pt: "Status", zh: "状态", ja: "状態", ar: "الحالة"
    },
    CsvColQuantity: {
        ru: "Количество", en: "Quantity", de: "Menge", fr: "Quantité", es: "Cantidad", it: "Quantità",
        pt: "Quantidade", zh: "数量", ja: "数量", ar: "الكمية"
    },
    CsvColPrice: {
        ru: "Цена", en: "Price", de: "Preis", fr: "Prix", es: "Precio", it: "Prezzo", pt: "Preço",
        zh: "价格", ja: "価格", ar: "السعر"
    },
    CsvColOpened: {
        ru: "Открыта", en: "Opened", de: "Geöffnet", fr: "Ouverte", es: "Abierta", it: "Aperta",
        pt: "Aberta", zh: "开封", ja: "開封", ar: "فُتحت"
    },
    CsvColDisposed: {
        ru: "Списано", en: "Disposed", de: "Entsorgt", fr: "Écarté", es: "Descartado", it: "Scartato",
        pt: "Descartado", zh: "已丢弃", ja: "廃棄済み", ar: "مستبعد"
    },
    CsvColComment: {
        ru: "Комментарий", en: "Comment", de: "Kommentar", fr: "Commentaire", es: "Comentario",
        it: "Commento", pt: "Comentário", zh: "备注", ja: "コメント", ar: "تعليق"
    },
    CsvColCategories: {
        ru: "Категории", en: "Categories", de: "Kategorien", fr: "Catégories", es: "Categorías",
        it: "Categorie", pt: "Categorias", zh: "分类", ja: "カテゴリ", ar: "الفئات"
    },
    CsvColPurchased: {
        ru: "Куплена", en: "Purchased", de: "Gekauft", fr: "Acheté", es: "Comprado", it: "Acquistato",
        pt: "Comprado", zh: "购入", ja: "購入", ar: "تاريخ الشراء"
    },
    CsvColPeriod: {
        ru: "Период после вскрытия", en: "Period after opening", de: "Frist nach Öffnen",
        fr: "Période après ouverture", es: "Período tras abrir", it: "Periodo dopo l'apertura",
        pt: "Período após abrir", zh: "开封后期限", ja: "開封後の期限", ar: "فترة ما بعد الفتح"
    },
    CsvColId: {
        ru: "ID", en: "ID", de: "ID", fr: "ID", es: "ID", it: "ID", pt: "ID", zh: "ID", ja: "ID", ar: "ID"
    },
    CsvOpenedCell: {
        ru: "открыта {0}", en: "opened {0}", de: "geöffnet {0}", fr: "ouverte le {0}",
        es: "abierta el {0}", it: "aperta il {0}", pt: "aberta em {0}", zh: "打开于 {0}",
        ja: "開封 {0}", ar: "فُتحت في {0}"
    },
    CsvMonthSuffix: {
        ru: " (до конца мес.)", en: " (until end of month)", de: " (bis Monatsende)",
        fr: " (jusqu'à la fin du mois)", es: " (hasta fin de mes)", it: " (fino a fine mese)",
        pt: " (até o fim do mês)", zh: " (至月底)", ja: " (月末まで)", ar: " (حتى نهاية الشهر)"
    },
    CsvDisposedStatus: {
        ru: "списано ({0})", en: "disposed ({0})", de: "entsorgt ({0})", fr: "écarté ({0})",
        es: "descartado ({0})", it: "scartato ({0})", pt: "descartado ({0})", zh: "已丢弃（{0}）",
        ja: "廃棄済み（{0}）", ar: "مستبعد ({0})"
    },

    // ===== Календарь =====
    CalLabel: {
        ru: "{0} {1}", en: "{0} {1}", de: "{0} {1}", fr: "{0} {1}", es: "{0} {1}",
        it: "{0} {1}", pt: "{0} {1}", zh: "{1}年{0}", ja: "{1}年{0}", ar: "{0} {1}"
    },
    Month1: {
        ru: "Январь", en: "January", de: "Januar", fr: "janvier", es: "enero", it: "gennaio",
        pt: "janeiro", zh: "1月", ja: "1月", ar: "يناير"
    },
    Month2: {
        ru: "Февраль", en: "February", de: "Februar", fr: "février", es: "febrero", it: "febbraio",
        pt: "fevereiro", zh: "2月", ja: "2月", ar: "فبراير"
    },
    Month3: {
        ru: "Март", en: "March", de: "März", fr: "mars", es: "marzo", it: "marzo",
        pt: "março", zh: "3月", ja: "3月", ar: "مارس"
    },
    Month4: {
        ru: "Апрель", en: "April", de: "April", fr: "avril", es: "abril", it: "aprile",
        pt: "abril", zh: "4月", ja: "4月", ar: "أبريل"
    },
    Month5: {
        ru: "Май", en: "May", de: "Mai", fr: "mai", es: "mayo", it: "maggio",
        pt: "maio", zh: "5月", ja: "5月", ar: "مايو"
    },
    Month6: {
        ru: "Июнь", en: "June", de: "Juni", fr: "juin", es: "junio", it: "giugno",
        pt: "junho", zh: "6月", ja: "6月", ar: "يونيو"
    },
    Month7: {
        ru: "Июль", en: "July", de: "Juli", fr: "juillet", es: "julio", it: "luglio",
        pt: "julho", zh: "7月", ja: "7月", ar: "يوليو"
    },
    Month8: {
        ru: "Август", en: "August", de: "August", fr: "août", es: "agosto", it: "agosto",
        pt: "agosto", zh: "8月", ja: "8月", ar: "أغسطس"
    },
    Month9: {
        ru: "Сентябрь", en: "September", de: "September", fr: "septembre", es: "septiembre",
        it: "settembre", pt: "setembro", zh: "9月", ja: "9月", ar: "سبتمبر"
    },
    Month10: {
        ru: "Октябрь", en: "October", de: "Oktober", fr: "octobre", es: "octubre", it: "ottobre",
        pt: "outubro", zh: "10月", ja: "10月", ar: "أكتوبر"
    },
    Month11: {
        ru: "Ноябрь", en: "November", de: "November", fr: "novembre", es: "noviembre",
        it: "novembre", pt: "novembro", zh: "11月", ja: "11月", ar: "نوفمبر"
    },
    Month12: {
        ru: "Декабрь", en: "December", de: "Dezember", fr: "décembre", es: "diciembre",
        it: "dicembre", pt: "dezembro", zh: "12月", ja: "12月", ar: "ديسمبر"
    },
    WeekMon: {
        ru: "Пн", en: "Mon", de: "Mo", fr: "lun.", es: "lun", it: "lun", pt: "seg",
        zh: "周一", ja: "月", ar: "إثن"
    },
    WeekTue: {
        ru: "Вт", en: "Tue", de: "Di", fr: "mar.", es: "mar", it: "mar", pt: "ter",
        zh: "周二", ja: "火", ar: "ثلا"
    },
    WeekWed: {
        ru: "Ср", en: "Wed", de: "Mi", fr: "mer.", es: "mié", it: "mer", pt: "qua",
        zh: "周三", ja: "水", ar: "أرب"
    },
    WeekThu: {
        ru: "Чт", en: "Thu", de: "Do", fr: "jeu.", es: "jue", it: "gio", pt: "qui",
        zh: "周四", ja: "木", ar: "خمي"
    },
    WeekFri: {
        ru: "Пт", en: "Fri", de: "Fr", fr: "ven.", es: "vie", it: "ven", pt: "sex",
        zh: "周五", ja: "金", ar: "جمع"
    },
    WeekSat: {
        ru: "Сб", en: "Sat", de: "Sa", fr: "sam.", es: "sáb", it: "sab", pt: "sáb",
        zh: "周六", ja: "土", ar: "سبت"
    },
    WeekSun: {
        ru: "Вс", en: "Sun", de: "So", fr: "dim.", es: "dom", it: "dom", pt: "dom",
        zh: "周日", ja: "日", ar: "أحد"
    },

    // ===== Формы лекарств (значения из FORMS, канон — русский) =====
    FormTablets: {
        ru: "Таблетки", en: "Tablets", de: "Tabletten", fr: "Comprimés", es: "Comprimidos",
        it: "Compresse", pt: "Comprimidos", zh: "片剂", ja: "錠剤", ar: "أقراص"
    },
    FormCapsules: {
        ru: "Капсулы", en: "Capsules", de: "Kapseln", fr: "Gélules", es: "Cápsulas",
        it: "Capsule", pt: "Cápsulas", zh: "胶囊", ja: "カプセル", ar: "كبسولات"
    },
    FormOintment: {
        ru: "Мазь", en: "Ointment", de: "Salbe", fr: "Pommade", es: "Pomada",
        it: "Unguento", pt: "Pomada", zh: "软膏", ja: "軟膏", ar: "مرهم"
    },
    FormGel: {
        ru: "Гель", en: "Gel", de: "Gel", fr: "Gel", es: "Gel", it: "Gel", pt: "Gel",
        zh: "凝胶", ja: "ゲル", ar: "جل"
    },
    FormSyrup: {
        ru: "Сироп", en: "Syrup", de: "Sirup", fr: "Sirop", es: "Jarabe", it: "Sciroppo",
        pt: "Xarope", zh: "糖浆", ja: "シロップ", ar: "شراب"
    },
    FormDrops: {
        ru: "Капли", en: "Drops", de: "Tropfen", fr: "Gouttes", es: "Gotas", it: "Gocce",
        pt: "Gotas", zh: "滴剂", ja: "滴剤", ar: "قطرات"
    },
    FormSpray: {
        ru: "Спрей", en: "Spray", de: "Spray", fr: "Spray", es: "Spray", it: "Spray",
        pt: "Spray", zh: "喷雾", ja: "スプレー", ar: "بخاخ"
    },
    FormInjections: {
        ru: "Инъекции", en: "Injections", de: "Injektionen", fr: "Injections", es: "Inyecciones",
        it: "Iniezioni", pt: "Injeções", zh: "注射剂", ja: "注射剤", ar: "حقن"
    },
    FormSuppositories: {
        ru: "Свечи", en: "Suppositories", de: "Zäpfchen", fr: "Suppositoires", es: "Supositorios",
        it: "Supposte", pt: "Supositórios", zh: "栓剂", ja: "坐剤", ar: "تحاميل"
    },
    FormPowder: {
        ru: "Порошок", en: "Powder", de: "Pulver", fr: "Poudre", es: "Polvo", it: "Polvere",
        pt: "Pó", zh: "粉剂", ja: "散剤", ar: "مسحوق"
    },
    FormOther: {
        ru: "Прочее", en: "Other", de: "Sonstiges", fr: "Autre", es: "Otro", it: "Altro",
        pt: "Outro", zh: "其他", ja: "その他", ar: "أخرى"
    },
}

/// Канонический (русский) текст строки — для журналов и данных, которые
/// хранятся в базе в одном виде и переводятся при показе.
pub fn canonical(key: Str) -> &'static str {
    s(Lang::Ru, key)
}

/// Текст строки на текущем языке.
pub fn tr(key: Str) -> &'static str {
    s(lang(), key)
}

/// Подстановка `{0}`, `{1}`, … в шаблон текущего языка.
pub fn fmt(key: Str, args: &[&str]) -> String {
    fmt_in(lang(), key, args)
}

/// Подстановка аргументов в шаблон заданного языка (чистая функция).
pub fn fmt_in(lang: Lang, key: Str, args: &[&str]) -> String {
    let template = s(lang, key);
    let mut out = String::with_capacity(template.len() + 16);
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            out.push_str(&rest[start..]);
            return out;
        };
        let idx = &after[..end];
        match idx.parse::<usize>().ok().and_then(|i| args.get(i)) {
            Some(value) => out.push_str(value),
            None => {
                out.push('{');
                out.push_str(idx);
                out.push('}');
            }
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Плейсхолдеры шаблона (`{0}`, `{1}`, …) — для теста паритета между языками.
#[cfg(test)]
fn placeholders(template: &str) -> Vec<usize> {
    let mut out = Vec::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('}') else {
            break;
        };
        if let Ok(i) = after[..end].parse::<usize>() {
            out.push(i);
        }
        rest = &after[end + 1..];
    }
    out.sort_unstable();
    out.dedup();
    out
}

// ---------- Календарь ----------

/// Ключи названий месяцев (1..=12).
pub const MONTH_KEYS: [Str; 12] = [
    Str::Month1,
    Str::Month2,
    Str::Month3,
    Str::Month4,
    Str::Month5,
    Str::Month6,
    Str::Month7,
    Str::Month8,
    Str::Month9,
    Str::Month10,
    Str::Month11,
    Str::Month12,
];

/// Название месяца (1..=12) на текущем языке.
pub fn month_name(month: u32) -> &'static str {
    tr(MONTH_KEYS[(month.clamp(1, 12) - 1) as usize])
}

// ---------- Канонические значения: формы, журнал ----------

/// Ключи строк форм в порядке `model::FORMS`.
pub const FORM_KEYS: [Str; 11] = [
    Str::FormTablets,
    Str::FormCapsules,
    Str::FormOintment,
    Str::FormGel,
    Str::FormSyrup,
    Str::FormDrops,
    Str::FormSpray,
    Str::FormInjections,
    Str::FormSuppositories,
    Str::FormPowder,
    Str::FormOther,
];

/// Название формы для экрана: справочник `FORMS` переводится, чужие значения
/// (импорт, легаси-данные) показываются как есть.
pub fn form_display(form: &str) -> String {
    match crate::model::FORMS.iter().position(|f| *f == form) {
        Some(i) => tr(FORM_KEYS[i]).to_string(),
        None => form.to_string(),
    }
}

/// Обратное сопоставление: локализованное название формы → канонический
/// (для импорта CSV, экспортированного на другом языке). Неизвестное — как есть.
pub fn form_canonical(form: &str) -> String {
    for (i, key) in FORM_KEYS.iter().enumerate() {
        for lang in Lang::ALL {
            if s(lang, *key) == form {
                return crate::model::FORMS[i].to_string();
            }
        }
    }
    form.to_string()
}

/// Плейсхолдер «Новое лекарство»: имя совпадает с переводом на любом языке —
/// данные переезжают между языками вместе с файлом.
pub fn is_new_medicine_placeholder(name: &str) -> bool {
    Lang::ALL
        .into_iter()
        .any(|l| s(l, Str::NewMedicineName) == name)
}

/// Ключи событий журнала: хранятся в каноническом (русском) виде,
/// при показе переводятся на текущий язык.
pub const JOURNAL_KIND_KEYS: [Str; 10] = [
    Str::JrnMedAdded,
    Str::JrnMedDeleted,
    Str::JrnPkgAdded,
    Str::JrnPkgChanged,
    Str::JrnPkgDeleted,
    Str::JrnPkgOpened,
    Str::JrnPkgDisposed,
    Str::JrnPkgRestored,
    Str::JrnRestoredBackup,
    Str::JrnImport,
];

/// Вид события журнала для экрана: неизвестные (ручные, легаси) — как есть.
pub fn journal_kind_display(kind: &str) -> String {
    journal_kind_display_in(lang(), kind)
}

/// То же на заданном языке (чистая функция).
pub fn journal_kind_display_in(lang: Lang, kind: &str) -> String {
    for key in JOURNAL_KIND_KEYS {
        if s(Lang::Ru, key) == kind {
            return s(lang, key).to_string();
        }
    }
    kind.to_string()
}

// ---------- CSV: локализуемые ячейки ----------

/// Заголовок CSV-экспорта: 15 колонок на текущем языке.
pub fn csv_headers() -> Vec<&'static str> {
    [
        Str::CsvColMedicine,
        Str::CsvColForm,
        Str::CsvColNotes,
        Str::CsvColExpiry,
        Str::CsvColDaysLeft,
        Str::CsvColStatus,
        Str::CsvColQuantity,
        Str::CsvColPrice,
        Str::CsvColOpened,
        Str::CsvColDisposed,
        Str::CsvColComment,
        Str::CsvColCategories,
        Str::CsvColPurchased,
        Str::CsvColPeriod,
        Str::CsvColId,
    ]
    .iter()
    .map(|k| tr(*k))
    .collect()
}

/// Первая колонка заголовка на любом языке — признак строки заголовка
/// (импорт должен понимать файлы, экспортированные на другом языке).
pub fn is_csv_header_first_cell(cell: &str) -> bool {
    let cell = cell.trim();
    Lang::ALL
        .into_iter()
        .any(|l| s(l, Str::CsvColMedicine) == cell)
}

/// Префиксы ячейки «Открыта» на всех языках (для импорта).
pub fn csv_opened_prefixes() -> Vec<&'static str> {
    Lang::ALL
        .into_iter()
        .map(|l| {
            let t = s(l, Str::CsvOpenedCell);
            t.split("{0}").next().unwrap_or(t)
        })
        .collect()
}

/// Причина списания из ячейки CSV на любом языке: «израсходовано» → REASON_USED,
/// всё остальное (включая неизвестное) → REASON_DISCARDED (как и раньше).
pub fn csv_dispose_reason(reason_text: &str) -> i32 {
    let reason_text = reason_text.trim();
    if Lang::ALL
        .into_iter()
        .any(|l| s(l, Str::ReasonUsed) == reason_text)
    {
        crate::model::REASON_USED
    } else {
        crate::model::REASON_DISCARDED
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_string_is_translated_in_all_languages() {
        for &key in Str::ALL {
            for lang in Lang::ALL {
                let text = s(lang, key);
                assert!(!text.is_empty(), "пустая строка {key:?} на {lang:?}");
            }
        }
    }

    #[test]
    fn placeholders_are_the_same_in_all_languages() {
        // иначе fmt молча потеряет аргумент на каком-то языке
        for &key in Str::ALL {
            let expected = placeholders(s(Lang::Ru, key));
            for lang in Lang::ALL {
                assert_eq!(
                    placeholders(s(lang, key)),
                    expected,
                    "плейсхолдеры {key:?} различаются на {lang:?}"
                );
            }
        }
    }

    #[test]
    fn fmt_substitutes_arguments() {
        assert_eq!(
            fmt_in(Lang::Ru, Str::StSoon, &["12"]),
            "истекает через 12 дн."
        );
        assert_eq!(fmt_in(Lang::En, Str::StSoon, &["12"]), "expires in 12 d.");
        assert_eq!(
            fmt_in(Lang::Ru, Str::SummaryTotal, &["2", "3"]),
            "Лекарств: 2 · Упаковок: 3"
        );
        // неизвестный индекс не съедается молча
        assert_eq!(fmt_in(Lang::En, Str::DaysN, &[]), "{0} d.");
    }

    #[test]
    fn lang_codes_and_locales_roundtrip() {
        for lang in Lang::ALL {
            assert_eq!(Lang::from_code(lang.code()), Some(lang));
            assert_eq!(
                Lang::from_locale(&format!("{}-XX", lang.code())),
                Some(lang)
            );
            assert!(!lang.native_name().is_empty());
        }
        assert_eq!(Lang::from_locale("xx-YY"), None);
        assert_eq!(Lang::from_code("xx"), None);
    }

    #[test]
    fn forms_map_both_ways() {
        use crate::model::FORMS;
        for (i, form) in FORMS.iter().enumerate() {
            let shown = form_display(form);
            assert_eq!(shown, s(Lang::Ru, FORM_KEYS[i]), "русский — канон");
            for lang in Lang::ALL {
                let localized = s(lang, FORM_KEYS[i]);
                assert_eq!(
                    form_canonical(localized),
                    *form,
                    "обратное сопоставление формы на {lang:?}"
                );
            }
        }
        // чужая форма показывается и сохраняется как есть
        assert_eq!(form_display("Шипучий"), "Шипучий");
        assert_eq!(form_canonical("Шипучий"), "Шипучий");
    }

    #[test]
    fn placeholder_name_recognized_in_every_language() {
        for lang in Lang::ALL {
            assert!(is_new_medicine_placeholder(s(lang, Str::NewMedicineName)));
        }
        assert!(!is_new_medicine_placeholder("Парацетамол"));
    }

    #[test]
    fn journal_kinds_translate_only_known_values() {
        // глобальный язык не трогаем: тесты ядра работают параллельно на русском
        for key in JOURNAL_KIND_KEYS {
            let canonical = s(Lang::Ru, key);
            for lang in Lang::ALL {
                assert_eq!(journal_kind_display_in(lang, canonical), s(lang, key));
            }
        }
        assert_eq!(
            journal_kind_display_in(Lang::En, "тест события"),
            "тест события"
        );
    }

    #[test]
    fn csv_helpers_cover_all_languages() {
        assert_eq!(csv_headers().len(), 15);
        for lang in Lang::ALL {
            assert!(is_csv_header_first_cell(s(lang, Str::CsvColMedicine)));
        }
        assert!(!is_csv_header_first_cell("Просто имя"));
        assert_eq!(csv_opened_prefixes().len(), Lang::ALL.len());
        for lang in Lang::ALL {
            let prefix = s(lang, Str::CsvOpenedCell).split("{0}").next().unwrap();
            assert_eq!(
                csv_dispose_reason(s(lang, Str::ReasonUsed)),
                crate::model::REASON_USED
            );
            assert_eq!(
                csv_dispose_reason(s(lang, Str::ReasonDiscarded)),
                crate::model::REASON_DISCARDED
            );
            let _ = prefix;
        }
    }
}
