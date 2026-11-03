use crate::{self as settings, settings_content::UiLanguageContent};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use settings::{RegisterSetting, Settings};

/// Display language of the Zed user interface (runtime setting).
/// Zed 用户界面的显示语言(运行时设置)。
///
/// Default: system
#[derive(
    Copy, Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Default, RegisterSetting,
)]
pub enum UiLanguage {
    #[default]
    System,
    /// 简体中文(Simplified Chinese)
    Chinese,
    /// English
    English,
}

impl UiLanguage {
    /// 对应的 rust-i18n locale 标识。
    /// The corresponding rust-i18n locale identifier.
    pub fn locale_str(self) -> &'static str {
        match self {
            UiLanguage::System => "system",
            UiLanguage::Chinese => "zh-CN",
            UiLanguage::English => "en",
        }
    }

    /// 从 locale 标识解析;未知值使用系统语言。
    /// Parse from a locale identifier; unknown values fall back to the
    /// system language.
    pub fn from_locale_str(locale: &str) -> Self {
        match locale {
            "en" => UiLanguage::English,
            "zh-CN" => UiLanguage::Chinese,
            _ => UiLanguage::System,
        }
    }
}

impl From<UiLanguageContent> for UiLanguage {
    fn from(value: UiLanguageContent) -> Self {
        match value {
            UiLanguageContent::System => Self::System,
            UiLanguageContent::Chinese => Self::Chinese,
            UiLanguageContent::English => Self::English,
        }
    }
}

impl From<UiLanguage> for UiLanguageContent {
    fn from(value: UiLanguage) -> Self {
        match value {
            UiLanguage::System => Self::System,
            UiLanguage::Chinese => Self::Chinese,
            UiLanguage::English => Self::English,
        }
    }
}

impl Settings for UiLanguage {
    fn from_settings(s: &crate::settings_content::SettingsContent) -> Self {
        s.ui_language.unwrap().into()
    }
}
