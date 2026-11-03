use gpui::{Anchor, Entity};
use std::borrow::Cow;
use ui::{
    ContextMenu, ContextMenuEntry, DocumentationSide, IconButton, IconName, IconPosition, IconSize,
    Label, PopoverMenu, Tooltip, prelude::*,
};
use zed_i18n::t;

use crate::{
    TableView,
    settings::{FilterSortOrder, TableViewSettings, VerticalAlignment},
};

///// Settings related /////

/// Adds a toggleable entry that applies `set` to the pane's settings when clicked. `description`,
/// when given, shows as a documentation aside explaining what the entry does.
fn toggle_entry(
    menu: ContextMenu,
    label: impl Into<SharedString>,
    description: Option<Cow<'static, str>>,
    selected: bool,
    view_entity: &Entity<TableView>,
    set: impl Fn(&mut TableViewSettings) + 'static,
) -> ContextMenu {
    let view_entity = view_entity.clone();
    let entry = ContextMenuEntry::new(label)
        .toggleable(IconPosition::Start, selected)
        .handler(move |_, cx| {
            view_entity.update(cx, |this, cx| {
                set(&mut this.settings);
                cx.notify();
            });
        });
    let entry = if let Some(description) = description {
        entry.documentation_aside(DocumentationSide::Right, move |_| {
            Label::new(description.clone()).into_any_element()
        })
    } else {
        entry
    };
    menu.item(entry)
}

pub(crate) fn settings_popover_menu(view_entity: Entity<TableView>) -> PopoverMenu<ContextMenu> {
    PopoverMenu::new("table-settings-menu")
        .trigger_with_tooltip(
            IconButton::new("table-settings-trigger", IconName::Filter)
                .icon_size(IconSize::Small)
                .size(ButtonSize::Compact),
            Tooltip::text("Table Settings"),
        )
        .anchor(Anchor::TopRight)
        .menu(move |window, cx| {
            let view_entity = view_entity.clone();
            Some(ContextMenu::build_persistent(
                window,
                cx,
                move |menu, _window, cx| {
                    let settings = view_entity.read(cx).settings.clone();

                    let menu = toggle_entry(
                        menu.header("Text Alignment"),
                        t!("csv_preview.settings.alignment_top"),
                        Some((t!("csv_preview.settings.text_alignment_tooltip")).into()),
                        matches!(settings.vertical_alignment, VerticalAlignment::Top),
                        &view_entity,
                        |settings| settings.vertical_alignment = VerticalAlignment::Top,
                    );
                    let menu = toggle_entry(
                        menu,
                        t!("csv_preview.settings.alignment_center"),
                        None,
                        matches!(settings.vertical_alignment, VerticalAlignment::Center),
                        &view_entity,
                        |settings| settings.vertical_alignment = VerticalAlignment::Center,
                    );

                    let menu = menu.separator().header("Filter Sort");
                    let menu = toggle_entry(
                        menu,
                        t!("csv_preview.settings.filter_sort_alpha_then_count"),
                        Some((t!("csv_preview.settings.filter_sort_tooltip")).into()),
                        settings.filter_sort_order == FilterSortOrder::AlphaThenCount,
                        &view_entity,
                        |settings| settings.filter_sort_order = FilterSortOrder::AlphaThenCount,
                    );
                    let menu = toggle_entry(
                        menu,
                        t!("csv_preview.settings.filter_sort_count_then_alpha"),
                        None,
                        settings.filter_sort_order == FilterSortOrder::CountThenAlpha,
                        &view_entity,
                        |settings| settings.filter_sort_order = FilterSortOrder::CountThenAlpha,
                    );

                    let menu = toggle_entry(
                        menu.separator(),
                        t!("csv_preview.settings.multiline_label"),
                        Some((t!("csv_preview.settings.multiline_tooltip")).into()),
                        settings.multiline_cells_enabled,
                        &view_entity,
                        |settings| {
                            settings.multiline_cells_enabled = !settings.multiline_cells_enabled
                        },
                    );

                    #[cfg(feature = "dev-tools")]
                    let menu = append_dev_only_entries(menu, &view_entity, &settings);

                    menu
                },
            ))
        })
}

#[cfg(feature = "dev-tools")]
fn append_dev_only_entries(
    menu: ContextMenu,
    view_entity: &Entity<TableView>,
    settings: &TableViewSettings,
) -> ContextMenu {
    use crate::settings::RowRenderMechanism;

    let menu = menu.separator().header("Dev-only: Rendering Mode");
    let menu = toggle_entry(
        menu,
        "Variable Height",
        Some(
            ("Dev-only section used for debugging purposes.\n\
             Will be removed on public release of the tabular data preview feature")
                .into(),
        ),
        settings.rendering_with == RowRenderMechanism::VariableList,
        view_entity,
        |settings| settings.rendering_with = RowRenderMechanism::VariableList,
    );
    let menu = toggle_entry(
        menu,
        "Uniform Height",
        None,
        settings.rendering_with == RowRenderMechanism::UniformList,
        view_entity,
        |settings| settings.rendering_with = RowRenderMechanism::UniformList,
    );

    let menu = toggle_entry(
        menu.separator(),
        "Show perf metrics",
        None,
        settings.show_perf_metrics_overlay,
        view_entity,
        |settings| settings.show_perf_metrics_overlay = !settings.show_perf_metrics_overlay,
    );
    toggle_entry(
        menu,
        "Show cell positions",
        None,
        settings.show_debug_info,
        view_entity,
        |settings| settings.show_debug_info = !settings.show_debug_info,
    )
}
