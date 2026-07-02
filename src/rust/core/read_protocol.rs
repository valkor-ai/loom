use std::{
    borrow::Cow,
    collections::{btree_map, BTreeMap},
    ops::{Deref, Index},
};

use schemars::{json_schema, JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};

use crate::ReadGroupRef;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectRequestInput {
    pub project_root: String,
    pub request_ref: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InspectRequestResult {
    pub request_ref: String,
    pub request_id: String,
    pub project_id: String,
    pub request_kind: String,
    pub read_groups: Vec<ReadGroupRef>,
    pub write_targets: Vec<Value>,
    pub submit_tool: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadFieldGroupInput {
    pub project_root: String,
    pub request_ref: String,
    pub group_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadFieldGroupResult {
    pub fields: ReadFieldGroupFields,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReadFieldGroupFields {
    nested: Value,
    flat: BTreeMap<String, FieldReadResult>,
}

impl ReadFieldGroupFields {
    pub fn from_flat(flat: BTreeMap<String, FieldReadResult>) -> Self {
        Self {
            nested: nested_fields(&flat),
            flat,
        }
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.flat.contains_key(key)
    }

    pub fn get(&self, key: &str) -> Option<&FieldReadResult> {
        self.flat.get(key)
    }

    pub fn keys(&self) -> btree_map::Keys<'_, String, FieldReadResult> {
        self.flat.keys()
    }

    pub fn iter(&self) -> btree_map::Iter<'_, String, FieldReadResult> {
        self.flat.iter()
    }

    pub fn contains(&self, key: &String) -> bool {
        self.flat.contains_key(key)
    }

    pub fn is_empty(&self) -> bool {
        self.flat.is_empty()
    }

    pub fn as_value(&self) -> &Value {
        &self.nested
    }

    pub fn as_object(&self) -> Option<&Map<String, Value>> {
        self.nested.as_object()
    }
}

impl Serialize for ReadFieldGroupFields {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.nested.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ReadFieldGroupFields {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let nested = Value::deserialize(deserializer)?;
        Ok(Self {
            nested,
            flat: BTreeMap::new(),
        })
    }
}

impl JsonSchema for ReadFieldGroupFields {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        "ReadFieldGroupFields".into()
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "object",
            "additionalProperties": true
        })
    }
}

impl Index<&str> for ReadFieldGroupFields {
    type Output = FieldReadResult;

    fn index(&self, index: &str) -> &Self::Output {
        &self.flat[index]
    }
}

impl Deref for ReadFieldGroupFields {
    type Target = Value;

    fn deref(&self) -> &Self::Target {
        &self.nested
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadRequestFieldsInput {
    pub project_root: String,
    pub request_ref: String,
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadRequestFieldsResult {
    pub fields: BTreeMap<String, FieldReadResult>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct FieldReadResult {
    pub value: Value,
}

fn nested_fields(fields: &BTreeMap<String, FieldReadResult>) -> Value {
    let mut root = Value::Object(Map::new());
    for (field, result) in fields {
        let parts = field
            .split('.')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        insert_nested_value(&mut root, &parts, result.value.clone());
    }
    root
}

fn insert_nested_value(current: &mut Value, parts: &[&str], value: Value) {
    let Some((head, tail)) = parts.split_first() else {
        return;
    };
    match current {
        Value::Object(object) => {
            if tail.is_empty() {
                object.insert((*head).to_string(), value);
                return;
            }
            let next = object
                .entry((*head).to_string())
                .or_insert_with(|| empty_container_for(tail[0]));
            if next.is_null() {
                *next = empty_container_for(tail[0]);
            }
            insert_nested_value(next, tail, value);
        }
        Value::Array(items) => {
            let Ok(index) = head.parse::<usize>() else {
                return;
            };
            if items.len() <= index {
                items.resize_with(index + 1, || Value::Null);
            }
            if tail.is_empty() {
                items[index] = value;
                return;
            }
            if items[index].is_null() {
                items[index] = empty_container_for(tail[0]);
            }
            insert_nested_value(&mut items[index], tail, value);
        }
        _ => {}
    }
}

fn empty_container_for(next_part: &str) -> Value {
    if next_part.parse::<usize>().is_ok() {
        Value::Array(vec![])
    } else {
        Value::Object(Map::new())
    }
}
