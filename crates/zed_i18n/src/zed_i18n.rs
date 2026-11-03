//! UI localization adapted from ReachForStar/zed-ZH_CN (GPL-3.0-or-later).

rust_i18n::i18n!("locales", fallback = "en");

use std::sync::{
    Once,
    atomic::{AtomicBool, Ordering},
};

include!(concat!(env!("OUT_DIR"), "/catalog_indexes.rs"));

static PSEUDO: AtomicBool = AtomicBool::new(false);

/// Fallback for unsupported system locales.
pub const DEFAULT_LOCALE: &str = "en";

static INIT: Once = Once::new();

/// 应在应用启动早期(任何 UI 构建之前)调用;重复调用安全,仅首次生效。
/// Initializes the system locale. Safe to call multiple times; only the
/// first call takes effect.
pub fn init() {
    INIT.call_once(|| {
        set_ui_locale("system");
    });
}

/// Resolves the system locale once when applying the language setting.
pub fn set_ui_locale(locale: &str) {
    let pseudo = locale == "pseudo" || std::env::var_os("ZED_LITE_PSEUDO_TRANSLATION").is_some();
    PSEUDO.store(pseudo, Ordering::Relaxed);
    if pseudo {
        rust_i18n::set_locale("en");
        return;
    }
    if locale == "system" {
        let system_locale = sys_locale::get_locale();
        rust_i18n::set_locale(
            system_locale
                .as_deref()
                .map(system_locale_name)
                .unwrap_or(DEFAULT_LOCALE),
        );
    } else {
        rust_i18n::set_locale(locale);
    }
}

fn system_locale_name(locale: &str) -> &'static str {
    let locale = locale.replace('_', "-").to_ascii_lowercase();
    if locale == "zh"
        || locale.starts_with("zh-cn")
        || locale.starts_with("zh-sg")
        || locale.starts_with("zh-hans")
    {
        "zh-CN"
    } else {
        DEFAULT_LOCALE
    }
}

/// 运行时查询指定 key 的翻译文本。
/// 按 zh-CN -> en 回退链查找;若仍缺失,rust-i18n 返回 key 本身。
/// 返回 `Cow` 而非 `String`:命中时借用编译期内嵌的 `&'static str`(零分配),
/// 仅在 key 未命中需回退为 key 本身时才借用入参 key。
/// 调用方需要拥有所有权时再自行 `into_owned()`/`to_string()`。
/// Runtime translation lookup with the fallback chain zh-CN -> en -> key.
/// Returns a `Cow`: borrowed (zero-allocation) when the key resolves, and only
/// borrows from the `key` argument on the missing-key fallback.
pub fn lookup<'a>(key: &'a str) -> std::borrow::Cow<'a, str> {
    let text = rust_i18n::t!(key);
    if is_pseudo() {
        format!("⟦{text}⟧").into()
    } else {
        text
    }
}

pub fn is_pseudo() -> bool {
    PSEUDO.load(Ordering::Relaxed)
}

pub fn action_name(action: &str) -> Option<String> {
    translated_key(&format!("commands.{action}"))
}

pub fn action_description(action: &str) -> Option<String> {
    translated_key(&format!("command_descriptions.{action}"))
}

fn translated_key(key: &str) -> Option<String> {
    let text = lookup(key);
    (text != key && text != format!("⟦{key}⟧")).then(|| text.into_owned())
}

pub fn schema_description(english: &str) -> Option<String> {
    if is_pseudo() {
        return Some(format!("⟦{english}⟧"));
    }
    let index = SCHEMA_DESCRIPTIONS
        .binary_search_by_key(&english, |(text, _)| *text)
        .ok()?;
    translated_key(SCHEMA_DESCRIPTIONS.get(index)?.1)
}

pub fn settings_page_title_matches(title: &str, requested: &str) -> bool {
    if title.eq_ignore_ascii_case(requested) {
        return true;
    }
    SETTINGS_PAGE_TITLE_KEYS.iter().any(|key| {
        let mut titles = rust_i18n::available_locales!()
            .into_iter()
            .map(|locale| rust_i18n::t!(*key, locale = locale))
            .collect::<Vec<_>>();
        titles.push(format!("⟦{}⟧", rust_i18n::t!(*key, locale = "en")).into());
        titles.iter().any(|text| text.eq_ignore_ascii_case(title))
            && titles
                .iter()
                .any(|text| text.eq_ignore_ascii_case(requested))
    })
}

/// 带变量插值的翻译查询;locale 模板中的占位符使用 `%{name}` 格式。
/// 复用 rust-i18n 的 `replace_patterns` 做单次扫描替换,避免逐个
/// `str::replace` 时插值值自身含 `%{other}` 文本被二次误伤。
/// 插值必然产生新文本,故返回拥有所有权的 `String`。
/// Translation lookup with interpolation; templates use `%{name}` placeholders.
/// Delegates to rust-i18n's single-pass `replace_patterns`. Interpolation always
/// produces new text, so this returns an owned `String`.
pub fn lookup_with(key: &str, args: &[(&str, &str)]) -> String {
    let template = lookup(key);
    let (keys, values): (Vec<&str>, Vec<String>) = args
        .iter()
        .map(|(name, value)| (*name, (*value).to_string()))
        .unzip();
    rust_i18n::replace_patterns(&template, &keys, &values)
}

/// i18n 翻译宏。
/// The i18n translation macro.
///
/// 简单查询返回 `Cow<'a, str>`:字面量 key 得到 `Cow<'static, str>`,
/// 命中翻译时零分配,可直接传给接受 `Into<SharedString>` 的 UI 组件。
/// 插值查询返回拥有所有权的 `String`。
/// The simple form returns a `Cow` (zero-allocation for literal keys that
/// resolve); the interpolation form returns an owned `String`.
///
/// ```ignore
/// use zed_i18n::t;
///
/// // 简单查询 / simple lookup
/// let s = t!("app_menus.file.save");
///
/// // 变量插值 / interpolation(模板: "Line %{line} of %{total}")
/// let s = t!("editor.line_of", line = 42, total = 100);
/// ```
#[macro_export]
macro_rules! t {
    ($key:expr $(,)?) => {
        $crate::lookup($key)
    };
    ($key:expr, $($arg_key:ident = $arg_value:expr),+ $(,)?) => {
        $crate::lookup_with($key, &[$((stringify!($arg_key), &($arg_value).to_string())),+])
    };
}

pub use rust_i18n::{available_locales, locale, set_locale};

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    /// locale 是进程级全局状态,rust-i18n 的 set_locale 影响所有线程;
    /// cargo test 默认并行执行,涉及 locale 切换的测试必须串行,
    /// 否则会因竞态非确定性地失败。
    /// The locale is process-global state: `set_locale` affects every thread,
    /// and cargo test runs tests in parallel by default, so tests that switch
    /// the locale must be serialized or they fail non-deterministically.
    static LOCALE_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn settings_page_arguments_work_across_display_languages() {
        assert!(!"外观".eq_ignore_ascii_case("Appearance"));
        assert!(crate::settings_page_title_matches("外观", "Appearance"));
        assert!(crate::settings_page_title_matches("外观", "appearance"));
        assert!(crate::settings_page_title_matches("Appearance", "外观"));
        assert!(crate::settings_page_title_matches(
            "⟦Appearance⟧",
            "Appearance"
        ));
        assert!(!crate::settings_page_title_matches(
            "Terminal",
            "Appearance"
        ));
    }

    #[test]
    fn pseudo_marks_ui_commands_and_schema_without_changing_values() {
        let _guard = LOCALE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        crate::set_ui_locale("pseudo");
        assert_eq!(crate::lookup("app_menus.file.save"), "⟦Save⟧");
        assert_eq!(
            crate::schema_description("A schema description"),
            Some("⟦A schema description⟧".to_owned())
        );
        let command = crate::action_name("editor::Copy").expect("Copy has a translation");
        assert!(command.starts_with('⟦') && command.ends_with('⟧'));
        assert_eq!(
            crate::lookup_with(
                "activity_indicator.status.downloading",
                &[("names", "%{count}")]
            ),
            "⟦Downloading %{count}...⟧"
        );
        crate::set_ui_locale("en");
        assert_eq!(crate::lookup("app_menus.file.save"), "Save");
    }

    #[test]
    fn command_names_and_descriptions_use_stable_action_ids() {
        let _guard = LOCALE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        crate::set_ui_locale("zh-CN");
        assert_eq!(
            crate::action_name("editor::DuplicateLineDown"),
            Some("向下复制行".to_owned())
        );
        assert!(crate::action_description("editor::DuplicateLineDown").is_some());
        assert_eq!(crate::action_name("test::NoSuchAction"), None);
        assert_eq!(
            crate::schema_description("An unknown external schema description"),
            None
        );
        crate::set_ui_locale("en");
    }

    #[test]
    fn supported_system_locales_and_english_fallback() {
        for locale in ["zh", "zh_CN", "zh-Hans-CN", "zh-SG"] {
            assert_eq!(super::system_locale_name(locale), "zh-CN");
        }
        for locale in ["en-US", "fr-FR", "zh-TW", "zh-Hant-HK"] {
            assert_eq!(super::system_locale_name(locale), "en");
        }
    }

    #[test]
    fn interpolation_preserves_values_as_text() {
        let _guard = LOCALE_LOCK
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        crate::set_locale("en");
        assert_eq!(
            crate::lookup_with(
                "activity_indicator.status.downloading",
                &[("names", "%{count}")]
            ),
            "Downloading %{count}..."
        );
        crate::set_locale("zh-CN");
        assert_eq!(crate::lookup("app_menus.file.save"), "保存");
        crate::set_locale("en");
        assert_eq!(crate::lookup("missing_translation"), "missing_translation");
    }

    /// 验证 locale 子目录分片文件的后缀命名机制:
    /// rust-i18n 3.1.5 以文件主干的最后一段(如 `foo.en.toml` -> `en`)
    /// 判定 locale,故 `locales/en/foo.en.toml` 会并入 en locale,
    /// `locales/zh-CN/foo.zh-CN.toml` 会并入 zh-CN locale。
    /// Verifies the dotted-suffix naming mechanism for split locale files:
    /// rust-i18n derives the locale from the last segment of the file stem,
    /// so `foo.en.toml` merges into the `en` locale.
    #[test]
    fn suffix_named_files_merge_into_locale() {
        let _guard = LOCALE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        crate::set_locale("en");
        assert_eq!(
            crate::lookup("suffix_test"),
            "Suffix Works",
            "en 后缀分片文件未并入 en locale"
        );
        crate::set_locale("zh-CN");
        assert_eq!(
            crate::lookup("suffix_test"),
            "后缀生效",
            "zh-CN 后缀分片文件未并入 zh-CN locale"
        );
    }

    /// 根 locale 文件的 key 必须仍可正常查询(回归保护)。
    /// Root locale file keys must remain queryable (regression guard).
    #[test]
    fn root_file_keys_resolve() {
        let _guard = LOCALE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        crate::set_locale("zh-CN");
        assert_eq!(crate::lookup("app_menus.file.save"), "保存");
        crate::set_locale("en");
        assert_eq!(crate::lookup("app_menus.file.save"), "Save");
    }
}
