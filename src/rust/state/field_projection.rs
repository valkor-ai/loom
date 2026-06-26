use serde_json::Value;

use crate::store::{StateError, StateResult};

pub(crate) fn selector_parts(field: &str) -> StateResult<Vec<String>> {
    let parts = field
        .split('.')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return Err(StateError::InvalidArgument(
            "field selector is required".to_string(),
        ));
    }
    Ok(parts)
}

pub(crate) fn select_value(root: &Value, parts: &[String]) -> StateResult<Value> {
    let mut current = root;
    for part in parts {
        match current {
            Value::Object(object) => {
                current = object.get(part).ok_or_else(|| {
                    StateError::InvalidArgument(format!("FIELD_NOT_FOUND: {}", parts.join(".")))
                })?;
            }
            Value::Array(array) => {
                let index = part.parse::<usize>().map_err(|_| {
                    StateError::InvalidArgument(format!("invalid array index in selector: {part}"))
                })?;
                current = array.get(index).ok_or_else(|| {
                    StateError::InvalidArgument(format!("array index out of bounds: {part}"))
                })?;
            }
            _ => {
                return Err(StateError::InvalidArgument(format!(
                    "selector cannot traverse non-container value at {part}"
                )));
            }
        }
    }
    Ok(current.clone())
}

pub(crate) fn select_compact_keyword_hints(
    root: &Value,
    selector_parts: &[String],
) -> StateResult<Value> {
    let compact = compact_keyword_hints(root);
    if selector_parts.is_empty() {
        return Ok(compact);
    }
    select_value(&compact, selector_parts)
}

pub(crate) fn select_compact_requirement_semantic_rules(root: &Value) -> Value {
    let Some(object) = root.as_object() else {
        return Value::Array(vec![]);
    };
    let Some(semantic) = object
        .get("requirementSemanticGrounding")
        .and_then(Value::as_object)
    else {
        return Value::Array(vec![]);
    };
    if let Some(compact_rules) = semantic.get("compactRules").and_then(Value::as_array) {
        return Value::Array(
            compact_rules
                .iter()
                .filter_map(Value::as_str)
                .map(|item| Value::String(item.to_string()))
                .collect(),
        );
    }
    Value::Array(
        semantic
            .get("rules")
            .and_then(Value::as_array)
            .map(|rules| {
                rules
                    .iter()
                    .filter_map(Value::as_str)
                    .take(7)
                    .map(|item| Value::String(item.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
    )
}

fn compact_keyword_hints(root: &Value) -> Value {
    let Some(object) = root.as_object() else {
        return serde_json::json!({
            "usage": "advisory_only",
            "status": "empty",
            "languageHints": [],
            "topKeywords": [],
            "sectionKeywords": [],
            "rules": keyword_hint_compact_rules(),
        });
    };
    let source = object
        .get("compact")
        .and_then(Value::as_object)
        .unwrap_or(object);

    let language_hints = source
        .get("languageHints")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .take(5)
                .map(|item| Value::String(item.to_string()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let top_keywords_source = source
        .get("topKeywords")
        .or_else(|| object.get("globalKeywords"));
    let top_keywords = top_keywords_source
        .and_then(Value::as_array)
        .map(|items| keyword_strings(items, 16))
        .unwrap_or_default();
    let section_keywords = source
        .get("sectionKeywords")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_object)
                .take(6)
                .filter_map(|section| {
                    let section_id = section
                        .get("sectionId")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let title = section.get("title").and_then(Value::as_str);
                    let source_item_id = section
                        .get("sourceItemId")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let keywords = section
                        .get("keywords")
                        .and_then(Value::as_array)
                        .map(|items| keyword_strings(items, 6))
                        .unwrap_or_default();
                    if section_id.is_empty() && keywords.is_empty() {
                        return None;
                    }
                    let mut value = serde_json::Map::new();
                    value.insert(
                        "sectionId".to_string(),
                        Value::String(section_id.to_string()),
                    );
                    value.insert(
                        "sourceItemId".to_string(),
                        Value::String(source_item_id.to_string()),
                    );
                    if let Some(title) = title {
                        value.insert("title".to_string(), Value::String(title.to_string()));
                    }
                    value.insert("keywords".to_string(), Value::Array(keywords));
                    Some(Value::Object(value))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    serde_json::json!({
        "usage": "advisory_only",
        "status": if object.get("status").and_then(Value::as_str) == Some("completed") {
            "completed"
        } else {
            "empty"
        },
        "languageHints": language_hints,
        "topKeywords": top_keywords,
        "sectionKeywords": section_keywords,
        "rules": keyword_hint_compact_rules(),
    })
}

fn keyword_strings(items: &[Value], limit: usize) -> Vec<Value> {
    items
        .iter()
        .filter_map(keyword_string)
        .filter(|keyword| !keyword.is_empty())
        .take(limit)
        .map(Value::String)
        .collect()
}

fn keyword_string(value: &Value) -> Option<String> {
    if let Some(keyword) = value.as_str() {
        return Some(keyword.trim().to_string());
    }
    let object = value.as_object()?;
    object
        .get("keyword")
        .or_else(|| object.get("term"))
        .and_then(Value::as_str)
        .map(str::trim)
        .map(str::to_string)
}

fn keyword_hint_compact_rules() -> Value {
    serde_json::json!({
        "advisoryOnly": true,
        "mustNotTreatAsScope": true,
        "mustNotTreatAsAcceptance": true,
        "ignoreWhenIrrelevant": true,
    })
}
