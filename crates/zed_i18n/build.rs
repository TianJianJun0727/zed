use std::{collections::BTreeMap, env, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut descriptions = BTreeMap::new();
    for path in [
        "locales/en/schema_descriptions.en.toml",
        "locales/en/command_descriptions.en.toml",
    ] {
        println!("cargo:rerun-if-changed={path}");
        let source = fs::read_to_string(path)?;
        let catalog: BTreeMap<String, BTreeMap<String, String>> = toml::from_str(&source)?;
        for (namespace, entries) in catalog {
            for (key, english) in entries {
                descriptions
                    .entry(english)
                    .or_insert(format!("{namespace}.{key}"));
            }
        }
    }
    let entries = descriptions
        .iter()
        .map(|(english, key)| format!("({english:?}, {key:?}),\n"))
        .collect::<String>();
    let page_titles_path = "locales/en/settings_ui.en.toml";
    println!("cargo:rerun-if-changed={page_titles_path}");
    let pages: toml::Value = toml::from_str(&fs::read_to_string(page_titles_path)?)?;
    let page_titles = pages
        .get("settings_ui")
        .and_then(|settings| settings.get("page_data"))
        .and_then(toml::Value::as_table)
        .ok_or("Missing settings page titles")?
        .keys()
        .filter(|key| key.ends_with("_page_title"))
        .map(|key| format!("\"settings_ui.page_data.{key}\",\n"))
        .collect::<String>();
    let output = PathBuf::from(env::var("OUT_DIR")?).join("catalog_indexes.rs");
    fs::write(
        output,
        format!(
            "static SCHEMA_DESCRIPTIONS: &[(&str, &str)] = &[{entries}];\n\
             static SETTINGS_PAGE_TITLE_KEYS: &[&str] = &[{page_titles}];"
        ),
    )?;
    Ok(())
}
