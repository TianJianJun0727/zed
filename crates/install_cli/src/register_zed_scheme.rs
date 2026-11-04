use gpui::{AsyncApp, actions};

actions!(
    cli,
    [
        /// Registers the zed-lite:// URL scheme handler.
        RegisterZedScheme
    ]
);

pub async fn register_zed_scheme(cx: &AsyncApp) -> anyhow::Result<()> {
    cx.update(|cx| cx.register_url_scheme("zed-lite")).await
}
