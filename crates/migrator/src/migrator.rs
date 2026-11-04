//! ## When to create a migration and why?
//! A migration is necessary when keymap actions or settings are renamed or transformed (e.g., from an array to a string, a string to an array, a boolean to an enum, etc.).
//!
//! This ensures that users with outdated settings are automatically updated to use the corresponding new settings internally.
//! It also provides a quick way to migrate their existing settings to the latest state using button in UI.
//!
//! ## How to create a migration?
//! Migrations use Tree-sitter to query commonly used patterns, such as actions with a string or actions with an array where the second argument is an object, etc.
//! Once queried, *you can filter out the modified items* and write the replacement logic.
//!
//! You *must not* modify previous migrations; always create new ones instead.
//! This is important because if a user is in an intermediate state, they can smoothly transition to the latest state.
//! Modifying existing migrations means they will only work for users upgrading from version x-1 to x, but not from x-2 to x, and so on, where x is the latest version.
//!
//! You only need to write replacement logic for x-1 to x because you can be certain that, internally, every user will be at x-1, regardless of their on disk state.

use anyhow::{Context as _, Result};
use settings_json::{infer_json_indent_size, parse_json_with_comments, update_value_in_json_text};
use std::{cmp::Reverse, ops::Range, sync::LazyLock};
use streaming_iterator::StreamingIterator;
use tree_sitter::{Query, QueryMatch};

mod migrations;
mod patterns;

fn migrate(text: &str, patterns: MigrationPatterns, query: &Query) -> Result<Option<String>> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_json::LANGUAGE.into())?;
    let syntax_tree = parser
        .parse(text, None)
        .context("failed to parse settings")?;

    let mut cursor = tree_sitter::QueryCursor::new();
    let mut matches = cursor.matches(query, syntax_tree.root_node(), text.as_bytes());

    let mut edits = vec![];
    while let Some(mat) = matches.next() {
        if let Some((_, callback)) = patterns.get(mat.pattern_index) {
            edits.extend(callback(text, mat, query));
        }
    }

    edits.sort_unstable_by_key(|(range, _)| (range.start, Reverse(range.end)));
    edits.dedup_by(|(range_b, _), (range_a, _)| {
        range_a.contains(&range_b.start) || range_a.contains(&range_b.end)
    });

    if edits.is_empty() {
        Ok(None)
    } else {
        let mut new_text = text.to_string();
        for (range, replacement) in edits.iter().rev() {
            new_text.replace_range(range.clone(), replacement);
        }
        if new_text == text {
            log::error!(
                "Edits computed for configuration migration do not cause a change: {:?}",
                edits
            );
            Ok(None)
        } else {
            Ok(Some(new_text))
        }
    }
}

/// Runs the provided migrations on the given text.
/// Will automatically return `Ok(None)` if there's no content to migrate.
fn run_migrations(text: &str, migrations: &[MigrationType]) -> Result<Option<String>> {
    if text.is_empty() {
        return Ok(None);
    }

    let mut current_text = text.to_string();
    let mut result: Option<String> = None;
    let json_indent_size = infer_json_indent_size(&current_text);
    for migration in migrations.iter() {
        let migrated_text = match migration {
            MigrationType::TreeSitter(patterns, query) => migrate(&current_text, patterns, query)?,
            MigrationType::Json(callback) => {
                if current_text.trim().is_empty() {
                    return Ok(None);
                }
                let old_content: serde_json_lenient::Value =
                    parse_json_with_comments(&current_text)?;
                let old_value = serde_json::to_value(&old_content).unwrap();
                let mut new_value = old_value.clone();
                callback(&mut new_value)?;
                if new_value != old_value {
                    let mut current = current_text.clone();
                    let mut edits = vec![];
                    update_value_in_json_text(
                        &mut current,
                        &mut vec![],
                        json_indent_size,
                        &old_value,
                        &new_value,
                        &mut edits,
                    );
                    let mut migrated_text = current_text.clone();
                    for (range, replacement) in edits.into_iter() {
                        migrated_text.replace_range(range, &replacement);
                    }
                    Some(migrated_text)
                } else {
                    None
                }
            }
        };
        if let Some(migrated_text) = migrated_text {
            current_text = migrated_text.clone();
            result = Some(migrated_text);
        }
    }
    Ok(result.filter(|new_text| text != new_text))
}

pub fn migrate_keymap(text: &str) -> Result<Option<String>> {
    let migrations: &[MigrationType] = &[
        MigrationType::TreeSitter(
            migrations::m_2025_01_29::KEYMAP_PATTERNS,
            &KEYMAP_QUERY_2025_01_29,
        ),
        MigrationType::TreeSitter(
            migrations::m_2025_01_30::KEYMAP_PATTERNS,
            &KEYMAP_QUERY_2025_01_30,
        ),
        MigrationType::TreeSitter(
            migrations::m_2025_03_03::KEYMAP_PATTERNS,
            &KEYMAP_QUERY_2025_03_03,
        ),
        MigrationType::TreeSitter(
            migrations::m_2025_03_06::KEYMAP_PATTERNS,
            &KEYMAP_QUERY_2025_03_06,
        ),
        MigrationType::TreeSitter(
            migrations::m_2025_04_15::KEYMAP_PATTERNS,
            &KEYMAP_QUERY_2025_04_15,
        ),
    ];
    run_migrations(text, migrations)
}

enum MigrationType<'a> {
    TreeSitter(MigrationPatterns, &'a Query),
    Json(fn(&mut serde_json::Value) -> Result<()>),
}

pub fn migrate_settings(text: &str) -> Result<Option<String>> {
    let migrations: &[MigrationType] = &[
        MigrationType::TreeSitter(
            migrations::m_2025_01_02::SETTINGS_PATTERNS,
            &SETTINGS_QUERY_2025_01_02,
        ),
        MigrationType::TreeSitter(
            migrations::m_2025_01_30::SETTINGS_PATTERNS,
            &SETTINGS_QUERY_2025_01_30,
        ),
        MigrationType::TreeSitter(
            migrations::m_2025_03_29::SETTINGS_PATTERNS,
            &SETTINGS_QUERY_2025_03_29,
        ),
        MigrationType::TreeSitter(
            migrations::m_2025_07_08::SETTINGS_PATTERNS,
            &SETTINGS_QUERY_2025_07_08,
        ),
        MigrationType::Json(migrations::m_2025_10_01::flatten_code_actions_formatters),
        MigrationType::Json(migrations::m_2025_10_02::remove_formatters_on_save),
        MigrationType::Json(migrations::m_2025_10_16::restore_code_actions_on_format),
        MigrationType::Json(migrations::m_2025_10_17::make_file_finder_include_ignored_an_enum),
        MigrationType::Json(migrations::m_2025_10_21::make_relative_line_numbers_an_enum),
        MigrationType::TreeSitter(
            migrations::m_2025_11_12::SETTINGS_PATTERNS,
            &SETTINGS_QUERY_2025_11_12,
        ),
        MigrationType::TreeSitter(
            migrations::m_2025_12_01::SETTINGS_PATTERNS,
            &SETTINGS_QUERY_2025_12_01,
        ),
        MigrationType::TreeSitter(
            migrations::m_2025_12_15::SETTINGS_PATTERNS,
            &SETTINGS_QUERY_2025_12_15,
        ),
        MigrationType::Json(migrations::m_2025_01_27::make_auto_indent_an_enum),
        MigrationType::TreeSitter(
            migrations::m_2026_03_16::SETTINGS_PATTERNS,
            &SETTINGS_QUERY_2026_03_16,
        ),
        MigrationType::Json(migrations::m_2026_04_01::restructure_profiles_with_settings_key),
        MigrationType::Json(
            migrations::m_2026_04_17::promote_show_branch_icon_true_to_show_branch_status_icon,
        ),
        MigrationType::TreeSitter(
            migrations::m_2026_05_04::SETTINGS_PATTERNS,
            &SETTINGS_QUERY_2026_05_04,
        ),
        MigrationType::Json(migrations::m_2026_08_17::make_git_gutter_width_an_enum),
        MigrationType::Json(migrations::m_2026_08_26::rename_folder_icons_to_folder_indicator),
        MigrationType::Json(migrations::m_2026_08_30::nest_markdown_preview_settings),
    ];
    run_migrations(text, migrations)
}

pub type MigrationPatterns = &'static [(
    &'static str,
    fn(&str, &QueryMatch, &Query) -> Option<(Range<usize>, String)>,
)];

macro_rules! define_query {
    ($var_name:ident, $patterns_path:path) => {
        static $var_name: LazyLock<Query> = LazyLock::new(|| {
            Query::new(
                &tree_sitter_json::LANGUAGE.into(),
                &$patterns_path
                    .iter()
                    .map(|pattern| pattern.0)
                    .collect::<String>(),
            )
            .unwrap()
        });
    };
}

// keymap
define_query!(
    KEYMAP_QUERY_2025_01_29,
    migrations::m_2025_01_29::KEYMAP_PATTERNS
);
define_query!(
    KEYMAP_QUERY_2025_01_30,
    migrations::m_2025_01_30::KEYMAP_PATTERNS
);
define_query!(
    KEYMAP_QUERY_2025_03_03,
    migrations::m_2025_03_03::KEYMAP_PATTERNS
);
define_query!(
    KEYMAP_QUERY_2025_03_06,
    migrations::m_2025_03_06::KEYMAP_PATTERNS
);
define_query!(
    KEYMAP_QUERY_2025_04_15,
    migrations::m_2025_04_15::KEYMAP_PATTERNS
);

// settings
define_query!(
    SETTINGS_QUERY_2025_01_02,
    migrations::m_2025_01_02::SETTINGS_PATTERNS
);
define_query!(
    SETTINGS_QUERY_2025_01_30,
    migrations::m_2025_01_30::SETTINGS_PATTERNS
);
define_query!(
    SETTINGS_QUERY_2025_03_29,
    migrations::m_2025_03_29::SETTINGS_PATTERNS
);
define_query!(
    SETTINGS_QUERY_2025_07_08,
    migrations::m_2025_07_08::SETTINGS_PATTERNS
);
define_query!(
    SETTINGS_QUERY_2025_11_12,
    migrations::m_2025_11_12::SETTINGS_PATTERNS
);
define_query!(
    SETTINGS_QUERY_2025_12_01,
    migrations::m_2025_12_01::SETTINGS_PATTERNS
);
define_query!(
    SETTINGS_QUERY_2025_12_15,
    migrations::m_2025_12_15::SETTINGS_PATTERNS
);
define_query!(
    SETTINGS_QUERY_2026_03_16,
    migrations::m_2026_03_16::SETTINGS_PATTERNS
);
define_query!(
    SETTINGS_QUERY_2026_05_04,
    migrations::m_2026_05_04::SETTINGS_PATTERNS
);

// custom query

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;

    #[track_caller]
    fn assert_migrated_correctly(migrated: Option<String>, expected: Option<&str>) {
        match (&migrated, &expected) {
            (Some(migrated), Some(expected)) => {
                pretty_assertions::assert_str_eq!(expected, migrated);
            }
            _ => {
                pretty_assertions::assert_eq!(migrated.as_deref(), expected);
            }
        }
    }

    #[track_caller]
    fn assert_migrate_keymap(input: &str, output: Option<&str>) {
        let migrated = migrate_keymap(input).unwrap();
        pretty_assertions::assert_eq!(migrated.as_deref(), output);
    }

    #[track_caller]
    fn assert_migrate_settings(input: &str, output: Option<&str>) {
        let migrated = migrate_settings(input).unwrap();
        assert_migrated_correctly(migrated.clone(), output);

        // expect that rerunning the migration does not result in another migration
        if let Some(migrated) = migrated {
            let rerun = migrate_settings(&migrated).unwrap();
            assert_migrated_correctly(rerun, None);
        }
    }

    #[track_caller]
    fn assert_migrate_with_migrations(
        migrations: &[MigrationType],
        input: &str,
        output: Option<&str>,
    ) {
        let migrated = run_migrations(input, migrations).unwrap();
        assert_migrated_correctly(migrated.clone(), output);

        // expect that rerunning the migration does not result in another migration
        if let Some(migrated) = migrated {
            let rerun = run_migrations(&migrated, migrations).unwrap();
            assert_migrated_correctly(rerun, None);
        }
    }

    #[test]
    fn test_empty_content() {
        assert_migrate_settings("", None)
    }

    #[test]
    fn test_replace_array_with_single_string() {
        assert_migrate_keymap(
            r#"
            [
                {
                    "bindings": {
                        "cmd-1": ["workspace::ActivatePaneInDirection", "Up"]
                    }
                }
            ]
            "#,
            Some(
                r#"
            [
                {
                    "bindings": {
                        "cmd-1": "workspace::ActivatePaneUp"
                    }
                }
            ]
            "#,
            ),
        )
    }

    #[test]
    fn test_replace_action_argument_object_with_single_value() {
        assert_migrate_keymap(
            r#"
            [
                {
                    "bindings": {
                        "cmd-1": ["editor::FoldAtLevel", { "level": 1 }]
                    }
                }
            ]
            "#,
            Some(
                r#"
            [
                {
                    "bindings": {
                        "cmd-1": ["editor::FoldAtLevel", 1]
                    }
                }
            ]
            "#,
            ),
        )
    }

    #[test]
    fn test_replace_action_argument_object_with_single_value_2() {
        assert_migrate_keymap(
            r#"
            [
                {
                    "bindings": {
                        "cmd-1": ["vim::PushOperator", { "Object": { "some" : "value" } }]
                    }
                }
            ]
            "#,
            Some(
                r#"
            [
                {
                    "bindings": {
                        "cmd-1": ["vim::PushObject", { "some" : "value" }]
                    }
                }
            ]
            "#,
            ),
        )
    }

    #[test]
    fn test_incremental_migrations() {
        // Here string transforms to array internally. Then, that array transforms back to string.
        assert_migrate_keymap(
            r#"
                [
                    {
                        "bindings": {
                            "ctrl-q": "editor::GoToHunk", // should remain same
                            "ctrl-w": "editor::GoToPrevHunk", // should rename
                            "ctrl-q": ["editor::GoToHunk", { "center_cursor": true }], // should transform
                            "ctrl-w": ["editor::GoToPreviousHunk", { "center_cursor": true }] // should transform
                        }
                    }
                ]
            "#,
            Some(
                r#"
                [
                    {
                        "bindings": {
                            "ctrl-q": "editor::GoToHunk", // should remain same
                            "ctrl-w": "editor::GoToPreviousHunk", // should rename
                            "ctrl-q": "editor::GoToHunk", // should transform
                            "ctrl-w": "editor::GoToPreviousHunk" // should transform
                        }
                    }
                ]
            "#,
            ),
        )
    }

    #[test]
    fn test_action_argument_snake_case() {
        // First performs transformations, then replacements
        assert_migrate_keymap(
            r#"
            [
                {
                    "bindings": {
                        "cmd-1": ["vim::PushOperator", { "Object": { "around": false } }],
                        "cmd-3": ["pane::CloseActiveItem", { "saveIntent": "saveAll" }],
                        "cmd-2": ["vim::NextWordStart", { "ignorePunctuation": true }],
                        "cmd-4": ["task::Spawn", { "task_name": "a b" }] // should remain as it is
                    }
                }
            ]
            "#,
            Some(
                r#"
            [
                {
                    "bindings": {
                        "cmd-1": ["vim::PushObject", { "around": false }],
                        "cmd-3": ["pane::CloseActiveItem", { "save_intent": "save_all" }],
                        "cmd-2": ["vim::NextWordStart", { "ignore_punctuation": true }],
                        "cmd-4": ["task::Spawn", { "task_name": "a b" }] // should remain as it is
                    }
                }
            ]
            "#,
            ),
        )
    }

    #[test]
    fn test_replace_settings_value() {
        assert_migrate_settings(
            r#"
                {
                    "scrollbar": {
                        "diagnostics": true

                    }
                }
            "#,
            Some(
                r#"
                {
                    "scrollbar": {
                        "diagnostics": "all"

                    }
                }
            "#,
            ),
        )
    }

    #[test]
    fn test_replace_settings_name_and_value() {
        assert_migrate_settings(
            r#"
                {
                    "tabs": {
                        "always_show_close_button": true
                    }
                }
            "#,
            Some(
                r#"
                {
                    "tabs": {
                        "show_close_button": "always"
                    }
                }
            "#,
            ),
        )
    }

    #[test]
    fn test_flatten_code_action_formatters_basic_array() {
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_01::flatten_code_actions_formatters,
            )],
            indoc! {r#"
                {
                  "formatter": [
                    {
                      "code_actions": {
                        "included-1": true,
                        "included-2": true,
                        "excluded": false,
                      }
                    }
                  ]
                }"#},
            Some(indoc! {r#"
                {
                  "formatter": [
                    {
                      "code_action": "included-1"
                    },
                    {
                      "code_action": "included-2"
                    }
                  ]
                }"#}),
        );
    }

    #[test]
    fn test_flatten_code_action_formatters_basic_object() {
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_01::flatten_code_actions_formatters,
            )],
            indoc! {r#"
                {
                  "formatter": {
                    "code_actions": {
                      "included-1": true,
                      "excluded": false,
                      "included-2": true
                    }
                  }
                }"#},
            Some(indoc! {r#"
                {
                  "formatter": [
                    {
                      "code_action": "included-1"
                    },
                    {
                      "code_action": "included-2"
                    }
                  ]
                }"#}),
        );
    }

    #[test]
    fn test_flatten_code_action_formatters_array_with_multiple_action_blocks() {
        assert_migrate_settings(
            indoc! {r#"
                {
                  "formatter": [
                    {
                       "code_actions": {
                          "included-1": true,
                          "included-2": true,
                          "excluded": false,
                       }
                    },
                    {
                      "language_server": "ruff"
                    },
                    {
                       "code_actions": {
                          "excluded": false,
                          "excluded-2": false,
                       }
                    }
                    // some comment
                    ,
                    {
                       "code_actions": {
                        "excluded": false,
                        "included-3": true,
                        "included-4": true,
                       }
                    },
                  ]
                }"#},
            Some(indoc! {r#"
                {
                  "formatter": [
                    {
                      "code_action": "included-1"
                    },
                    {
                      "code_action": "included-2"
                    },
                    {
                      "language_server": "ruff"
                    },
                    {
                      "code_action": "included-3"
                    },
                    {
                      "code_action": "included-4"
                    }
                  ]
                }"#}),
        );
    }

    #[test]
    fn test_flatten_code_action_formatters_array_with_multiple_action_blocks_in_languages() {
        assert_migrate_settings(
            indoc! {r#"
                {
                  "languages": {
                    "Rust": {
                      "formatter": [
                        {
                          "code_actions": {
                            "included-1": true,
                            "included-2": true,
                            "excluded": false,
                          }
                        },
                        {
                          "language_server": "ruff"
                        },
                        {
                          "code_actions": {
                            "excluded": false,
                            "excluded-2": false,
                          }
                        }
                        // some comment
                        ,
                        {
                          "code_actions": {
                            "excluded": false,
                            "included-3": true,
                            "included-4": true,
                          }
                        },
                      ]
                    }
                  }
                }"#},
            Some(indoc! {r#"
                {
                  "languages": {
                    "Rust": {
                      "formatter": [
                        {
                          "code_action": "included-1"
                        },
                        {
                          "code_action": "included-2"
                        },
                        {
                          "language_server": "ruff"
                        },
                        {
                          "code_action": "included-3"
                        },
                        {
                          "code_action": "included-4"
                        }
                      ]
                    }
                  }
                }"#}),
        );
    }

    #[test]
    fn test_flatten_code_action_formatters_array_with_multiple_action_blocks_in_defaults_and_multiple_languages()
     {
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_01::flatten_code_actions_formatters,
            )],
            indoc! {r#"
                {
                  "formatter": {
                    "code_actions": {
                      "default-1": true,
                      "default-2": true,
                      "default-3": true,
                      "default-4": true,
                    }
                  },
                  "languages": {
                    "Rust": {
                      "formatter": [
                        {
                          "code_actions": {
                            "included-1": true,
                            "included-2": true,
                            "excluded": false,
                          }
                        },
                        {
                          "language_server": "ruff"
                        },
                        {
                          "code_actions": {
                            "excluded": false,
                            "excluded-2": false,
                          }
                        }
                        // some comment
                        ,
                        {
                          "code_actions": {
                            "excluded": false,
                            "included-3": true,
                            "included-4": true,
                          }
                        },
                      ]
                    },
                    "Python": {
                      "formatter": [
                        {
                          "language_server": "ruff"
                        },
                        {
                          "code_actions": {
                            "excluded": false,
                            "excluded-2": false,
                          }
                        }
                        // some comment
                        ,
                        {
                          "code_actions": {
                            "excluded": false,
                            "included-3": true,
                            "included-4": true,
                          }
                        },
                      ]
                    }
                  }
                }"#},
            Some(indoc! {r#"
                {
                  "formatter": [
                    {
                      "code_action": "default-1"
                    },
                    {
                      "code_action": "default-2"
                    },
                    {
                      "code_action": "default-3"
                    },
                    {
                      "code_action": "default-4"
                    }
                  ],
                  "languages": {
                    "Rust": {
                      "formatter": [
                        {
                          "code_action": "included-1"
                        },
                        {
                          "code_action": "included-2"
                        },
                        {
                          "language_server": "ruff"
                        },
                        {
                          "code_action": "included-3"
                        },
                        {
                          "code_action": "included-4"
                        }
                      ]
                    },
                    "Python": {
                      "formatter": [
                        {
                          "language_server": "ruff"
                        },
                        {
                          "code_action": "included-3"
                        },
                        {
                          "code_action": "included-4"
                        }
                      ]
                    }
                  }
                }"#}),
        );
    }

    #[test]
    fn test_flatten_code_action_formatters_array_with_format_on_save_and_multiple_languages() {
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_01::flatten_code_actions_formatters,
            )],
            indoc! {r#"
                {
                  "formatter": {
                    "code_actions": {
                      "default-1": true,
                      "default-2": true,
                      "default-3": true,
                      "default-4": true,
                    }
                  },
                  "format_on_save": [
                    {
                      "code_actions": {
                        "included-1": true,
                        "included-2": true,
                        "excluded": false,
                      }
                    },
                    {
                      "language_server": "ruff"
                    },
                    {
                      "code_actions": {
                        "excluded": false,
                        "excluded-2": false,
                      }
                    }
                    // some comment
                    ,
                    {
                      "code_actions": {
                        "excluded": false,
                        "included-3": true,
                        "included-4": true,
                      }
                    },
                  ],
                  "languages": {
                    "Rust": {
                      "format_on_save": "prettier",
                      "formatter": [
                        {
                          "code_actions": {
                            "included-1": true,
                            "included-2": true,
                            "excluded": false,
                          }
                        },
                        {
                          "language_server": "ruff"
                        },
                        {
                          "code_actions": {
                            "excluded": false,
                            "excluded-2": false,
                          }
                        }
                        // some comment
                        ,
                        {
                          "code_actions": {
                            "excluded": false,
                            "included-3": true,
                            "included-4": true,
                          }
                        },
                      ]
                    },
                    "Python": {
                      "format_on_save": {
                        "code_actions": {
                          "on-save-1": true,
                          "on-save-2": true,
                        }
                      },
                      "formatter": [
                        {
                          "language_server": "ruff"
                        },
                        {
                          "code_actions": {
                            "excluded": false,
                            "excluded-2": false,
                          }
                        }
                        // some comment
                        ,
                        {
                          "code_actions": {
                            "excluded": false,
                            "included-3": true,
                            "included-4": true,
                          }
                        },
                      ]
                    }
                  }
                }"#},
            Some(indoc! {r#"
                {
                  "formatter": [
                    {
                      "code_action": "default-1"
                    },
                    {
                      "code_action": "default-2"
                    },
                    {
                      "code_action": "default-3"
                    },
                    {
                      "code_action": "default-4"
                    }
                  ],
                  "format_on_save": [
                    {
                      "code_action": "included-1"
                    },
                    {
                      "code_action": "included-2"
                    },
                    {
                      "language_server": "ruff"
                    },
                    {
                      "code_action": "included-3"
                    },
                    {
                      "code_action": "included-4"
                    }
                  ],
                  "languages": {
                    "Rust": {
                      "format_on_save": "prettier",
                      "formatter": [
                        {
                          "code_action": "included-1"
                        },
                        {
                          "code_action": "included-2"
                        },
                        {
                          "language_server": "ruff"
                        },
                        {
                          "code_action": "included-3"
                        },
                        {
                          "code_action": "included-4"
                        }
                      ]
                    },
                    "Python": {
                      "format_on_save": [
                        {
                          "code_action": "on-save-1"
                        },
                        {
                          "code_action": "on-save-2"
                        }
                      ],
                      "formatter": [
                        {
                          "language_server": "ruff"
                        },
                        {
                          "code_action": "included-3"
                        },
                        {
                          "code_action": "included-4"
                        }
                      ]
                    }
                  }
                }"#}),
        );
    }

    #[test]
    fn test_format_on_save_formatter_migration_basic() {
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_02::remove_formatters_on_save,
            )],
            indoc! {r#"
                {
                    "format_on_save": "prettier"
                }"#},
            Some(indoc! {r#"
                {
                    "formatter": "prettier",
                    "format_on_save": "on"
                }"#}),
        );
    }

    #[test]
    fn test_format_on_save_formatter_migration_array() {
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_02::remove_formatters_on_save,
            )],
            indoc! {r#"
                {
                    "format_on_save": ["prettier", {"language_server": "eslint"}]
                }"#},
            Some(indoc! {r#"
                {
                    "formatter": [
                        "prettier",
                        {
                            "language_server": "eslint"
                        }
                    ],
                    "format_on_save": "on"
                }"#}),
        );
    }

    #[test]
    fn test_format_on_save_on_off_unchanged() {
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_02::remove_formatters_on_save,
            )],
            indoc! {r#"
                {
                    "format_on_save": "on"
                }"#},
            None,
        );

        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_02::remove_formatters_on_save,
            )],
            indoc! {r#"
                {
                    "format_on_save": "off"
                }"#},
            None,
        );
    }

    #[test]
    fn test_format_on_save_formatter_migration_in_languages() {
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_02::remove_formatters_on_save,
            )],
            indoc! {r#"
                {
                    "languages": {
                        "Rust": {
                            "format_on_save": "rust-analyzer"
                        },
                        "Python": {
                            "format_on_save": ["ruff", "black"]
                        }
                    }
                }"#},
            Some(indoc! {r#"
                {
                    "languages": {
                        "Rust": {
                            "formatter": "rust-analyzer",
                            "format_on_save": "on"
                        },
                        "Python": {
                            "formatter": [
                                "ruff",
                                "black"
                            ],
                            "format_on_save": "on"
                        }
                    }
                }"#}),
        );
    }

    #[test]
    fn test_format_on_save_formatter_migration_mixed_global_and_languages() {
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_02::remove_formatters_on_save,
            )],
            indoc! {r#"
                {
                    "format_on_save": "prettier",
                    "languages": {
                        "Rust": {
                            "format_on_save": "rust-analyzer"
                        },
                        "Python": {
                            "format_on_save": "on"
                        }
                    }
                }"#},
            Some(indoc! {r#"
                {
                    "formatter": "prettier",
                    "format_on_save": "on",
                    "languages": {
                        "Rust": {
                            "formatter": "rust-analyzer",
                            "format_on_save": "on"
                        },
                        "Python": {
                            "format_on_save": "on"
                        }
                    }
                }"#}),
        );
    }

    #[test]
    fn test_format_on_save_no_migration_when_no_format_on_save() {
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_02::remove_formatters_on_save,
            )],
            indoc! {r#"
                {
                    "formatter": ["prettier"]
                }"#},
            None,
        );
    }

    #[test]
    fn test_restore_code_actions_on_format() {
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_16::restore_code_actions_on_format,
            )],
            indoc! {r#"
                {
                    "formatter": {
                        "code_action": "foo"
                    }
                }"#},
            Some(indoc! {r#"
                {
                    "code_actions_on_format": {
                        "foo": true
                    },
                    "formatter": []
                }"#}),
        );

        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_16::restore_code_actions_on_format,
            )],
            indoc! {r#"
                {
                    "formatter": [
                        { "code_action": "foo" },
                        "auto"
                    ]
                }"#},
            None,
        );

        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_16::restore_code_actions_on_format,
            )],
            indoc! {r#"
                {
                    "formatter": {
                        "code_action": "foo"
                    },
                    "code_actions_on_format": {
                        "bar": true,
                        "baz": false
                    }
                }"#},
            Some(indoc! {r#"
                {
                    "formatter": [],
                    "code_actions_on_format": {
                        "foo": true,
                        "bar": true,
                        "baz": false
                    }
                }"#}),
        );

        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_16::restore_code_actions_on_format,
            )],
            indoc! {r#"
                {
                    "formatter": [
                        { "code_action": "foo" },
                        { "code_action": "qux" },
                    ],
                    "code_actions_on_format": {
                        "bar": true,
                        "baz": false
                    }
                }"#},
            Some(indoc! {r#"
                {
                    "formatter": [],
                    "code_actions_on_format": {
                        "foo": true,
                        "qux": true,
                        "bar": true,
                        "baz": false
                    }
                }"#}),
        );

        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_16::restore_code_actions_on_format,
            )],
            indoc! {r#"
                {
                    "formatter": [],
                    "code_actions_on_format": {
                        "bar": true,
                        "baz": false
                    }
                }"#},
            None,
        );
    }

    #[test]
    fn test_make_file_finder_include_ignored_an_enum() {
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_17::make_file_finder_include_ignored_an_enum,
            )],
            r#"{ }"#,
            None,
        );

        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_17::make_file_finder_include_ignored_an_enum,
            )],
            indoc! {r#"
                {
                    "file_finder": {
                        "include_ignored": true
                    }
                }"#},
            Some(indoc! {r#"
                {
                    "file_finder": {
                        "include_ignored": "all"
                    }
                }"#}),
        );

        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_17::make_file_finder_include_ignored_an_enum,
            )],
            indoc! {r#"
                {
                    "file_finder": {
                        "include_ignored": false
                    }
                }"#},
            Some(indoc! {r#"
                {
                    "file_finder": {
                        "include_ignored": "indexed"
                    }
                }"#}),
        );

        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_17::make_file_finder_include_ignored_an_enum,
            )],
            indoc! {r#"
                {
                    "file_finder": {
                        "include_ignored": null
                    }
                }"#},
            Some(indoc! {r#"
                {
                    "file_finder": {
                        "include_ignored": "smart"
                    }
                }"#}),
        );

        // Platform key: settings nested inside "linux" should be migrated
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_17::make_file_finder_include_ignored_an_enum,
            )],
            indoc! {r#"
                {
                    "linux": {
                        "file_finder": {
                            "include_ignored": true
                        }
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    "linux": {
                        "file_finder": {
                            "include_ignored": "all"
                        }
                    }
                }
            "#}),
        );

        // Profile: settings nested inside profiles should be migrated
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_17::make_file_finder_include_ignored_an_enum,
            )],
            indoc! {r#"
                {
                    "profiles": {
                        "work": {
                            "file_finder": {
                                "include_ignored": false
                            }
                        }
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    "profiles": {
                        "work": {
                            "file_finder": {
                                "include_ignored": "indexed"
                            }
                        }
                    }
                }
            "#}),
        );
    }

    #[test]
    fn test_make_relative_line_numbers_an_enum() {
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_21::make_relative_line_numbers_an_enum,
            )],
            r#"{ }"#,
            None,
        );

        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_21::make_relative_line_numbers_an_enum,
            )],
            indoc! {r#"
                {
                    "relative_line_numbers": true
                }"#},
            Some(indoc! {r#"
                {
                    "relative_line_numbers": "enabled"
                }"#}),
        );

        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_21::make_relative_line_numbers_an_enum,
            )],
            indoc! {r#"
                {
                    "relative_line_numbers": false
                }"#},
            Some(indoc! {r#"
                {
                    "relative_line_numbers": "disabled"
                }"#}),
        );

        // Platform key: settings nested inside "macos" should be migrated
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_21::make_relative_line_numbers_an_enum,
            )],
            indoc! {r#"
                {
                    "macos": {
                        "relative_line_numbers": true
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    "macos": {
                        "relative_line_numbers": "enabled"
                    }
                }
            "#}),
        );

        // Profile: settings nested inside profiles should be migrated
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_10_21::make_relative_line_numbers_an_enum,
            )],
            indoc! {r#"
                {
                    "profiles": {
                        "dev": {
                            "relative_line_numbers": false
                        }
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    "profiles": {
                        "dev": {
                            "relative_line_numbers": "disabled"
                        }
                    }
                }
            "#}),
        );
    }

    #[test]
    fn test_project_panel_open_file_on_paste_migration() {
        assert_migrate_settings(
            indoc! {r#"
                {
                    "project_panel": {
                        "open_file_on_paste": true
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    "project_panel": {
                        "auto_open": { "on_paste": true }
                    }
                }
            "#}),
        );

        assert_migrate_settings(
            indoc! {r#"
                {
                    "project_panel": {
                        "open_file_on_paste": false
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    "project_panel": {
                        "auto_open": { "on_paste": false }
                    }
                }
            "#}),
        );
    }

    #[test]
    fn test_enable_preview_from_code_navigation_migration() {
        assert_migrate_settings(
            indoc! {r#"
                {
                    "other_setting_1": 1,
                    "preview_tabs": {
                        "other_setting_2": 2,
                        "enable_preview_from_code_navigation": false
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    "other_setting_1": 1,
                    "preview_tabs": {
                        "other_setting_2": 2,
                        "enable_keep_preview_on_code_navigation": false
                    }
                }
            "#}),
        );

        assert_migrate_settings(
            indoc! {r#"
                {
                    "other_setting_1": 1,
                    "preview_tabs": {
                        "other_setting_2": 2,
                        "enable_preview_from_code_navigation": true
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    "other_setting_1": 1,
                    "preview_tabs": {
                        "other_setting_2": 2,
                        "enable_keep_preview_on_code_navigation": true
                    }
                }
            "#}),
        );
    }

    #[test]
    fn test_make_auto_indent_an_enum() {
        // Empty settings should not change
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_01_27::make_auto_indent_an_enum,
            )],
            r#"{ }"#,
            None,
        );

        // true should become "syntax_aware"
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_01_27::make_auto_indent_an_enum,
            )],
            indoc! {r#"
                {
                    "auto_indent": true
                }"#},
            Some(indoc! {r#"
                {
                    "auto_indent": "syntax_aware"
                }"#}),
        );

        // false should become "none"
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_01_27::make_auto_indent_an_enum,
            )],
            indoc! {r#"
                {
                    "auto_indent": false
                }"#},
            Some(indoc! {r#"
                {
                    "auto_indent": "none"
                }"#}),
        );

        // Already valid enum values should not change
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_01_27::make_auto_indent_an_enum,
            )],
            indoc! {r#"
                {
                    "auto_indent": "preserve_indent"
                }"#},
            None,
        );

        // Should also work inside languages
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2025_01_27::make_auto_indent_an_enum,
            )],
            indoc! {r#"
                {
                    "auto_indent": true,
                    "languages": {
                        "Python": {
                            "auto_indent": false
                        }
                    }
                }"#},
            Some(indoc! {r#"
                {
                    "auto_indent": "syntax_aware",
                    "languages": {
                        "Python": {
                            "auto_indent": "none"
                        }
                    }
                }"#}),
        );
    }

    #[test]
    fn test_restructure_profiles_with_settings_key() {
        assert_migrate_settings(
            indoc! {r#"
                {
                    "buffer_font_size": 14,
                    "profiles": {
                        "Presenting": {
                            "buffer_font_size": 20,
                            "theme": "One Light"
                        },
                        "Minimal": {
                            "vim_mode": true
                        }
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    "buffer_font_size": 14,
                    "profiles": {
                        "Presenting": {
                            "settings": {
                                "buffer_font_size": 20,
                                "theme": "One Light"
                            }
                        },
                        "Minimal": {
                            "settings": {
                                "vim_mode": true
                            }
                        }
                    }
                }
            "#}),
        );
    }

    #[test]
    fn test_restructure_profiles_with_settings_key_already_migrated() {
        assert_migrate_settings(
            indoc! {r#"
                {
                    "profiles": {
                        "Presenting": {
                            "settings": {
                                "buffer_font_size": 20
                            }
                        }
                    }
                }
            "#},
            None,
        );
    }

    #[test]
    fn test_restructure_profiles_with_settings_key_no_profiles() {
        assert_migrate_settings(
            indoc! {r#"
                {
                    "buffer_font_size": 14
                }
            "#},
            None,
        );
    }

    #[test]
    fn test_migration_helpers_handle_various_profile_forms() {
        let setting = "a_setting";
        let old_value = "old_value";
        let new_value = "new_value";

        fn language_setting_fn(value: &mut serde_json::Value, _: &[&str]) -> anyhow::Result<()> {
            if let Some(obj) = value.as_object_mut() {
                if let Some(v) = obj.get_mut("a_setting") {
                    *v = serde_json::json!("new_value");
                }
            }
            Ok(())
        }

        let mut settings_fn = |map: &mut serde_json::Map<String, serde_json::Value>| {
            if let Some(v) = map.get_mut(setting) {
                *v = serde_json::json!(new_value);
            }
            Ok(())
        };

        // Legacy form
        let input = serde_json::json!({
            "profiles": {
                "work": {
                    setting: old_value
                }
            }
        });
        let expected = serde_json::json!({
            "profiles": {
                "work": {
                    setting: new_value
                }
            }
        });

        let mut value = input.clone();
        migrations::migrate_settings(&mut value, &mut settings_fn).unwrap();
        assert_eq!(value, expected);

        let mut value = input;
        migrations::migrate_language_setting(&mut value, language_setting_fn).unwrap();
        assert_eq!(value, expected);

        // Form after migration: `m_2026_04_01`
        let input = serde_json::json!({
            "profiles": {
                "work": {
                    "settings": {
                        setting: old_value
                    }
                }
            }
        });
        let expected = serde_json::json!({
            "profiles": {
                "work": {
                    "settings": {
                        setting: new_value
                    }
                }
            }
        });

        let mut value = input.clone();
        migrations::migrate_settings(&mut value, &mut settings_fn).unwrap();
        assert_eq!(value, expected);

        let mut value = input;
        migrations::migrate_language_setting(&mut value, language_setting_fn).unwrap();
        assert_eq!(value, expected);

        // Base-only form after migration: `m_2026_04_01` (no settings to migrate)
        let input = serde_json::json!({
            "profiles": {
                "work": {
                    "base": "default"
                }
            }
        });

        let mut value = input.clone();
        migrations::migrate_settings(&mut value, &mut settings_fn).unwrap();
        assert_eq!(value, input);

        let mut value = input.clone();
        migrations::migrate_language_setting(&mut value, language_setting_fn).unwrap();
        assert_eq!(value, input);
    }

    #[test]
    fn test_promote_show_branch_icon_true_to_show_branch_status_icon_at_root() {
        assert_migrate_settings(
            indoc! {r#"
                {
                    "title_bar": {
                        "show_branch_icon": true,
                        "show_branch_name": true
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    "title_bar": {
                        "show_branch_status_icon": true,
                        "show_branch_name": true
                    }
                }
            "#}),
        );
    }

    #[test]
    fn test_drop_show_branch_icon_false_without_setting_status_icon() {
        assert_migrate_settings(
            indoc! {r#"
                {
                    "title_bar": {
                        "show_branch_icon": false,
                        "show_branch_name": true
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    "title_bar": {
                        "show_branch_name": true
                    }
                }
            "#}),
        );
    }

    #[test]
    fn test_promote_show_branch_icon_true_to_show_branch_status_icon_in_platform_override() {
        assert_migrate_settings(
            indoc! {r#"
                {
                    "macos": {
                        "title_bar": {
                            "show_branch_icon": true,
                            "show_branch_name": true
                        }
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    "macos": {
                        "title_bar": {
                            "show_branch_status_icon": true,
                            "show_branch_name": true
                        }
                    }
                }
            "#}),
        );
    }

    #[test]
    fn test_promote_show_branch_icon_true_to_show_branch_status_icon_in_release_override() {
        assert_migrate_settings(
            indoc! {r#"
                {
                    "preview": {
                        "title_bar": {
                            "show_branch_icon": true,
                            "show_branch_name": true
                        }
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    "preview": {
                        "title_bar": {
                            "show_branch_status_icon": true,
                            "show_branch_name": true
                        }
                    }
                }
            "#}),
        );
    }

    #[test]
    fn test_promote_show_branch_icon_true_to_show_branch_status_icon_in_profiles() {
        assert_migrate_settings(
            indoc! {r#"
                {
                    "profiles": {
                        "work": {
                            "title_bar": {
                                "show_branch_icon": true,
                                "show_branch_name": true
                            }
                        }
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    "profiles": {
                        "work": {
                            "settings": {
                                "title_bar": {
                                    "show_branch_status_icon": true,
                                    "show_branch_name": true
                                }
                            }
                        }
                    }
                }
            "#}),
        );
    }

    #[test]
    fn test_promote_show_branch_icon_true_to_show_branch_status_icon_across_all_scopes() {
        assert_migrate_settings(
            indoc! {r#"
                {
                    "title_bar": {
                        "show_branch_icon": true,
                        "show_branch_name": true
                    },
                    "macos": {
                        "title_bar": {
                            "show_branch_icon": true,
                            "show_branch_name": true
                        }
                    },
                    "preview": {
                        "title_bar": {
                            "show_branch_icon": true,
                            "show_branch_name": true
                        }
                    },
                    "profiles": {
                        "work": {
                            "title_bar": {
                                "show_branch_icon": true,
                                "show_branch_name": true
                            }
                        }
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    "title_bar": {
                        "show_branch_status_icon": true,
                        "show_branch_name": true
                    },
                    "macos": {
                        "title_bar": {
                            "show_branch_status_icon": true,
                            "show_branch_name": true
                        }
                    },
                    "preview": {
                        "title_bar": {
                            "show_branch_status_icon": true,
                            "show_branch_name": true
                        }
                    },
                    "profiles": {
                        "work": {
                            "settings": {
                                "title_bar": {
                                    "show_branch_status_icon": true,
                                    "show_branch_name": true
                                }
                            }
                        }
                    }
                }
            "#}),
        );
    }

    #[test]
    fn test_rename_hide_mouse_on_typing_and_movement_to_on_typing_and_action() {
        assert_migrate_settings(
            r#"
                {
                    "hide_mouse": "on_typing_and_movement"
                }
            "#,
            Some(
                r#"
                {
                    "hide_mouse": "on_typing_and_action"
                }
            "#,
            ),
        );
    }

    #[test]
    fn test_chain_hide_mouse_while_typing_to_on_typing_and_action() {
        assert_migrate_settings(
            r#"
                {
                    "hide_mouse_while_typing": true
                }
            "#,
            Some(
                r#"
                {
                    "hide_mouse": "on_typing_and_action"
                }
            "#,
            ),
        );
    }

    #[test]
    fn test_promote_show_branch_icon_true_to_show_branch_status_icon_no_change_when_already_migrated()
     {
        assert_migrate_settings(
            indoc! {r#"
                {
                    "title_bar": {
                        "show_branch_status_icon": true,
                        "show_branch_name": true
                    }
                }
            "#},
            None,
        );

        // No title_bar key — should be unchanged
        assert_migrate_settings(r#"{ "theme": "One Dark" }"#, None);

        // title_bar without show_branch_icon — should be unchanged
        assert_migrate_settings(
            indoc! {r#"
                {
                    "title_bar": {
                        "show_branch_name": true
                    }
                }
            "#},
            None,
        );
    }

    #[test]
    fn test_make_git_gutter_width_an_enum_from_number() {
        assert_migrate_settings(
            indoc! {r#"
                {
                    "gutter": {
                        "git_gutter_width": 4.0
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    "gutter": {
                        "git_gutter_width": {
                            "custom": 4.0
                        }
                    }
                }
            "#}),
        );
    }

    #[test]
    fn test_make_git_gutter_width_an_enum_no_change_when_already_migrated() {
        // already "default" string — no change
        assert_migrate_settings(
            indoc! {r#"
                {
                    "gutter": {
                        "git_gutter_width": "default"
                    }
                }
            "#},
            None,
        );

        // already custom object — no change
        assert_migrate_settings(
            indoc! {r#"
                {
                    "gutter": {
                        "git_gutter_width": { "custom": 4.0 }
                    }
                }
            "#},
            None,
        );

        // no gutter key — no change
        assert_migrate_settings(r#"{ "theme": "One Dark" }"#, None);
    }

    #[test]
    fn test_rename_folder_icons_to_folder_indicator_in_all_panels() {
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2026_08_26::rename_folder_icons_to_folder_indicator,
            )],
            indoc! {r#"
                {
                    "project_panel": {
                        "folder_icons": true
                    },
                    "outline_panel": {
                        "folder_icons": false
                    },
                    "git_panel": {
                        "folder_icons": true
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    "project_panel": {
                        "folder_indicator": "icon"
                    },
                    "outline_panel": {
                        "folder_indicator": "chevron"
                    },
                    "git_panel": {
                        "folder_indicator": "icon"
                    }
                }
            "#}),
        );
    }

    // The shared JSON migration driver applies a rename as a delete plus an add, and
    // added keys are written to the front of their object. Comments and sibling values
    // survive; only the renamed key's position moves.
    #[test]
    fn test_rename_folder_icons_to_folder_indicator_preserves_comments_and_siblings() {
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2026_08_26::rename_folder_icons_to_folder_indicator,
            )],
            indoc! {r#"
                {
                    // Keep this comment.
                    "project_panel": {
                        "file_icons": true,
                        "folder_icons": false,
                        "indent_size": 20
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    // Keep this comment.
                    "project_panel": {
                        "folder_indicator": "chevron",
                        "file_icons": true,
                        "indent_size": 20
                    }
                }
            "#}),
        );
    }

    #[test]
    fn test_rename_folder_icons_to_folder_indicator_in_platform_overrides() {
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2026_08_26::rename_folder_icons_to_folder_indicator,
            )],
            indoc! {r#"
                {
                    "macos": {
                        "project_panel": {
                            "folder_icons": false
                        }
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    "macos": {
                        "project_panel": {
                            "folder_indicator": "chevron"
                        }
                    }
                }
            "#}),
        );
    }

    #[test]
    fn test_rename_folder_icons_to_folder_indicator_does_not_clobber_new_key() {
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2026_08_26::rename_folder_icons_to_folder_indicator,
            )],
            indoc! {r#"
                {
                    "project_panel": {
                        "folder_icons": true,
                        "folder_indicator": "both"
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    "project_panel": {
                        "folder_indicator": "both"
                    }
                }
            "#}),
        );
    }

    #[test]
    fn test_rename_folder_icons_to_folder_indicator_no_change_cases() {
        let migrations = &[MigrationType::Json(
            migrations::m_2026_08_26::rename_folder_icons_to_folder_indicator,
        )];

        // Already migrated.
        assert_migrate_with_migrations(
            migrations,
            indoc! {r#"
                {
                    "project_panel": {
                        "folder_indicator": "both"
                    }
                }
            "#},
            None,
        );

        // A non-boolean value is already invalid; leave it rather than guess.
        assert_migrate_with_migrations(
            migrations,
            indoc! {r#"
                {
                    "project_panel": {
                        "folder_icons": 3
                    }
                }
            "#},
            None,
        );

        // `folder_icons` outside the three panels is not ours to rename.
        assert_migrate_with_migrations(
            migrations,
            indoc! {r#"
                {
                    "terminal": {
                        "folder_icons": true
                    }
                }
            "#},
            None,
        );
    }

    #[test]
    fn test_rename_folder_icons_to_folder_indicator_is_registered() {
        assert_migrate_settings(
            indoc! {r#"
                {
                    "project_panel": {
                        "folder_icons": false
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    "project_panel": {
                        "folder_indicator": "chevron"
                    }
                }
            "#}),
        );
    }

    #[test]
    fn test_nest_markdown_preview_settings_across_all_scopes() {
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2026_08_30::nest_markdown_preview_settings,
            )],
            indoc! {r#"
                {
                    "markdown_preview_font_size": 14,
                    "macos": {
                        "markdown_preview_font_size": 15
                    },
                    "preview": {
                        "markdown_preview_font_size": 16
                    },
                    "profiles": {
                        "work": {
                            "settings": {
                                "markdown_preview_font_size": 17
                            }
                        }
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    "markdown_preview": {
                        "font_size": 14
                    },
                    "macos": {
                        "markdown_preview": {
                            "font_size": 15
                        }
                    },
                    "preview": {
                        "markdown_preview": {
                            "font_size": 16
                        }
                    },
                    "profiles": {
                        "work": {
                            "settings": {
                                "markdown_preview": {
                                    "font_size": 17
                                }
                            }
                        }
                    }
                }
            "#}),
        );
    }

    #[test]
    fn test_nest_markdown_preview_settings_merges_without_clobbering() {
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2026_08_30::nest_markdown_preview_settings,
            )],
            indoc! {r#"
                {
                    "markdown_preview_font_size": 15,
                    "markdown_preview_font_family": "Zed Sans",
                    "markdown_preview_code_font_family": "Zed Mono",
                    "markdown_preview_theme": "One Dark",
                    "markdown_preview": {
                        "font_size": 18,
                        "limit_content_width": false,
                        "max_width": 900
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    "markdown_preview": {
                        "theme": "One Dark",
                        "code_font_family": "Zed Mono",
                        "font_family": "Zed Sans",
                        "font_size": 18,
                        "limit_content_width": false,
                        "max_width": 900
                    }
                }
            "#}),
        );
    }

    #[test]
    fn test_nest_markdown_preview_settings_leaves_malformed_object() {
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2026_08_30::nest_markdown_preview_settings,
            )],
            indoc! {r#"
                {
                    "markdown_preview_font_size": 15,
                    "markdown_preview": false
                }
            "#},
            None,
        );
    }

    #[test]
    fn test_nest_markdown_preview_settings_replaces_null_when_legacy_settings_exist() {
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2026_08_30::nest_markdown_preview_settings,
            )],
            indoc! {r#"
                {
                    "markdown_preview": null,
                    "markdown_preview_font_size": 15,
                    "preview": {
                        "markdown_preview": null,
                        "markdown_preview_theme": "One Dark"
                    }
                }
            "#},
            Some(indoc! {r#"
                {
                    "markdown_preview": {
                        "font_size": 15
                    },
                    "preview": {
                        "markdown_preview": {
                            "theme": "One Dark"
                        }
                    }
                }
            "#}),
        );
    }

    #[test]
    fn test_nest_markdown_preview_settings_leaves_null_without_legacy_settings() {
        assert_migrate_with_migrations(
            &[MigrationType::Json(
                migrations::m_2026_08_30::nest_markdown_preview_settings,
            )],
            indoc! {r#"
                {
                    "markdown_preview": null,
                    "preview": {
                        "markdown_preview": null
                    }
                }
            "#},
            None,
        );
    }

    #[test]
    fn test_nest_markdown_preview_settings_is_registered() {
        assert_migrate_settings(
            indoc! {r#"
                {
                    "markdown_preview_font_size": 15,
                    "markdown_preview_font_family": "Zed Sans",
                    "markdown_preview_code_font_family": "Zed Mono",
                    "markdown_preview_theme": "One Dark"
                }
            "#},
            Some(indoc! {r#"
                {
                    "markdown_preview": {
                        "font_size": 15,
                        "font_family": "Zed Sans",
                        "code_font_family": "Zed Mono",
                        "theme": "One Dark"
                    }
                }
            "#}),
        );
    }
}
