use serde_json::Value;

pub const DEFAULT_FIELD_BUDGET: usize = 32 * 1024;
pub const DEFAULT_GROUP_BUDGET: usize = 96 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectionMode {
    Index,
    Batch,
    Targeted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionBatch {
    pub mode: ProjectionMode,
    pub fields: Vec<String>,
}

/// Split read paths only at JSON object keys or array indexes. This keeps every
/// leaf value available to the reader and never truncates a semantic value.
pub fn lossless_projection_batches(
    root: &Value,
    fields: &[String],
    field_budget: usize,
    group_budget: usize,
) -> Result<Vec<ProjectionBatch>, String> {
    let mut expanded = Vec::new();
    let mut was_split = false;
    for field in fields {
        let paths = if value_at_path(root, field).is_some() {
            let paths = split_path(root, field, field_budget)?;
            was_split |= paths.len() != 1 || paths.first().is_none_or(|path| path != field);
            paths
        } else {
            vec![field.clone()]
        };
        expanded.extend(paths);
    }
    expanded.sort();
    expanded.dedup();
    if expanded.is_empty() {
        return Err("request projection has no readable fields".to_string());
    }

    let mut batches = Vec::new();
    let mut current = Vec::new();
    let mut current_bytes = 0usize;
    for field in expanded {
        let bytes = value_at_path(root, &field)
            .map(json_bytes)
            .transpose()?
            .unwrap_or(0);
        if bytes > group_budget {
            return Err(format!(
                "request projection field {field} exceeds group budget {group_budget} bytes"
            ));
        }
        if !current.is_empty() && current_bytes.saturating_add(bytes) > group_budget {
            batches.push(ProjectionBatch {
                mode: projection_mode(&current, was_split),
                fields: std::mem::take(&mut current),
            });
            current_bytes = 0;
        }
        current_bytes = current_bytes.saturating_add(bytes);
        current.push(field);
    }
    if !current.is_empty() {
        batches.push(ProjectionBatch {
            mode: projection_mode(&current, was_split),
            fields: current,
        });
    }
    Ok(batches)
}

fn projection_mode(fields: &[String], was_split: bool) -> ProjectionMode {
    if !was_split {
        return ProjectionMode::Targeted;
    }
    if !fields.is_empty() && fields.iter().all(|field| is_index_field(field)) {
        ProjectionMode::Index
    } else {
        ProjectionMode::Batch
    }
}

fn is_index_field(field: &str) -> bool {
    let leaf = field.rsplit('.').next().unwrap_or(field);
    let lower = leaf.to_ascii_lowercase();
    lower == "id"
        || lower.ends_with("id")
        || lower.ends_with("ids")
        || lower.ends_with("ref")
        || lower.ends_with("refs")
        || lower.contains("fingerprint")
}

fn split_path(root: &Value, field: &str, budget: usize) -> Result<Vec<String>, String> {
    let value = value_at_path(root, field)
        .ok_or_else(|| format!("request projection cannot safely split missing field {field}"))?;
    if json_bytes(value)? <= budget {
        return Ok(vec![field.to_string()]);
    }
    match value {
        Value::Object(object) if !object.is_empty() => {
            let mut paths = Vec::new();
            for key in object.keys() {
                let child = if field.is_empty() {
                    key.clone()
                } else {
                    format!("{field}.{key}")
                };
                paths.extend(split_path(root, &child, budget)?);
            }
            Ok(paths)
        }
        Value::Array(items) if !items.is_empty() => {
            let mut paths = Vec::new();
            for index in 0..items.len() {
                paths.extend(split_path(root, &format!("{field}.{index}"), budget)?);
            }
            Ok(paths)
        }
        _ => Err(format!(
            "request projection field {field} exceeds {budget} bytes and cannot be split without truncation"
        )),
    }
}

fn value_at_path<'a>(root: &'a Value, field: &str) -> Option<&'a Value> {
    let mut current = root;
    for part in field.split('.').filter(|part| !part.is_empty()) {
        current = match current {
            Value::Object(object) => object.get(part)?,
            Value::Array(items) => items.get(part.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(current)
}

fn json_bytes(value: &Value) -> Result<usize, String> {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn splits_large_object_without_losing_keys() {
        let root = json!({"task": {"a": "x", "b": "y"}});
        let batches = lossless_projection_batches(&root, &["task".to_string()], 8, 32)
            .expect("lossless projection");
        let fields = batches
            .into_iter()
            .flat_map(|batch| batch.fields)
            .collect::<Vec<_>>();
        assert_eq!(fields, vec!["task.a", "task.b"]);
    }

    #[test]
    fn rejects_unsplittable_large_scalar() {
        let root = json!({"text": "0123456789"});
        let error = lossless_projection_batches(&root, &["text".to_string()], 2, 8)
            .expect_err("large scalar must not be truncated");
        assert!(error.contains("cannot be split"));
    }
}
