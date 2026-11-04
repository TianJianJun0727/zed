use client::os_info;
use extension_host::ExtensionStore;
use gpui::{App, ClipboardItem, PromptLevel, actions};
use system_specs::{CopySystemSpecsIntoClipboard, SystemSpecs};
use util::ResultExt;
use workspace::Workspace;
use zed_actions::feedback::{FileBugReport, RequestFeature};
use zed_i18n::t;

actions!(
    zed,
    [
        /// Opens the Zed repository on GitHub.
        OpenZedRepo,
        /// Copies installed extensions to the clipboard for bug reports.
        CopyInstalledExtensionsIntoClipboard
    ]
);

const ZED_REPO_URL: &str = "https://github.com/TianJianJun0727/zed-lite";

const REQUEST_FEATURE_URL: &str =
    "https://github.com/TianJianJun0727/zed-lite/issues/new?labels=enhancement";

pub fn init(cx: &mut App) {
    cx.observe_new(|workspace: &mut Workspace, _, _| {
        workspace
            .register_action(|_, _: &CopySystemSpecsIntoClipboard, window, cx| {
                let specs = SystemSpecs::new(window, cx, os_info::os_name(), os_info::os_version());

                cx.spawn_in(window, async move |_, cx| {
                    let specs = specs.await.to_string();

                    cx.update(|_, cx| {
                        cx.write_to_clipboard(ClipboardItem::new_string(specs.clone()))
                    })
                    .log_err();

                    let message = t!("feedback.copy.copied_into_clipboard");
                    let ok = t!("feedback.copy.ok");
                    cx.prompt(PromptLevel::Info, &message, Some(&specs), &[&*ok])
                        .await
                })
                .detach();
            })
            .register_action(|_, _: &CopyInstalledExtensionsIntoClipboard, window, cx| {
                let clipboard_text = format_installed_extensions_for_clipboard(cx);
                cx.write_to_clipboard(ClipboardItem::new_string(clipboard_text.clone()));
                let message = t!("feedback.copy.copied_into_clipboard");
                let ok = t!("feedback.copy.ok");
                drop(window.prompt(
                    PromptLevel::Info,
                    &message,
                    Some(&clipboard_text),
                    &[&*ok],
                    cx,
                ));
            })
            .register_action(|_, _: &RequestFeature, _, cx| {
                cx.open_url(REQUEST_FEATURE_URL);
            })
            .register_action(|_, _: &FileBugReport, _, cx| {
                cx.open_url("https://github.com/TianJianJun0727/zed-lite/issues/new");
            })
            .register_action(move |_, _: &OpenZedRepo, _, cx| {
                cx.open_url(ZED_REPO_URL);
            });
    })
    .detach();
}

fn format_installed_extensions_for_clipboard(cx: &mut App) -> String {
    let store = ExtensionStore::global(cx);
    let store = store.read(cx);
    let mut lines = Vec::with_capacity(store.extension_index.extensions.len());

    for (extension_id, entry) in store.extension_index.extensions.iter() {
        let line = format!(
            "- {} ({}) v{}{}",
            entry.manifest.name,
            extension_id,
            entry.manifest.version,
            if entry.dev { " (dev)" } else { "" }
        );
        lines.push(line);
    }

    lines.sort();

    if lines.is_empty() {
        return "No extensions installed.".to_string();
    }

    format!(
        "Installed extensions ({}):\n{}",
        lines.len(),
        lines.join("\n")
    )
}
