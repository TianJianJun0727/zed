use serde_json::Value;

pub fn localized_schema(mut schema: Value) -> Value {
    if &*zed_i18n::locale() != "en" || zed_i18n::is_pseudo() {
        localize_descriptions(&mut schema);
    }
    schema
}

fn localize_descriptions(schema: &mut Value) {
    match schema {
        Value::Object(object) => {
            let action = object.get("const").and_then(Value::as_str);
            if let Some(description) = object.get("description").and_then(Value::as_str) {
                let translated = action
                    .and_then(zed_i18n::action_description)
                    .or_else(|| zed_i18n::schema_description(description));
                if let Some(translated) = translated {
                    object.insert("description".into(), translated.into());
                }
            }
            for (key, value) in object {
                if key != "description" {
                    localize_descriptions(value);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(localize_descriptions),
        _ => {}
    }
}
