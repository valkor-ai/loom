use delivery_core::LoomMcpActionResult;
use rmcp::model::{
    AnnotateAble, ListResourceTemplatesResult, RawResourceTemplate, ReadResourceResult,
    ResourceContents, ResourceTemplate,
};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceTemplateRegistration {
    pub uri_template: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub target_batch: u32,
}

pub const BATCH_2_RESOURCE_TEMPLATES: &[ResourceTemplateRegistration] = &[
    ResourceTemplateRegistration {
        uri_template: "loom://projects/{projectId}/requests/{requestId}/field-groups/{groupId}",
        name: "request-field-group",
        description: "Read a declared request field group.",
        target_batch: 3,
    },
    ResourceTemplateRegistration {
        uri_template: "loom://projects/{projectId}/requests/{requestId}/fields/{encodedFieldPath}",
        name: "request-field",
        description: "Read declared request fields by path.",
        target_batch: 3,
    },
    ResourceTemplateRegistration {
        uri_template: "loom://knowledge/{sourceId}/builds/{buildId}/chunks/{chunkId}",
        name: "knowledge-chunk",
        description: "Inspect a built knowledge chunk.",
        target_batch: 6,
    },
];

#[derive(Debug, Clone, Default)]
pub struct ResourceRegistry {
    templates: Vec<ResourceTemplateRegistration>,
}

impl ResourceRegistry {
    pub fn batch_2() -> Self {
        Self {
            templates: BATCH_2_RESOURCE_TEMPLATES.to_vec(),
        }
    }

    pub fn list_resource_templates(&self) -> ListResourceTemplatesResult {
        let mut templates: Vec<ResourceTemplate> = self
            .templates
            .iter()
            .map(|template| {
                RawResourceTemplate::new(template.uri_template, template.name)
                    .with_description(template.description)
                    .with_mime_type("application/json")
                    .no_annotation()
            })
            .collect();
        templates.sort_by(|left, right| left.uri_template.cmp(&right.uri_template));
        ListResourceTemplatesResult::with_all_items(templates)
    }

    pub fn read_placeholder(&self, uri: &str) -> ReadResourceResult {
        let target_batch = self
            .templates
            .iter()
            .find(|template| matches_template(uri, template.uri_template))
            .map(|template| template.target_batch)
            .unwrap_or(3);
        let result =
            LoomMcpActionResult::not_implemented_for_batch("", "resources/read", target_batch);
        ReadResourceResult::new(vec![ResourceContents::text(
            to_json_string(result),
            uri.to_string(),
        )
        .with_mime_type("application/json")])
    }
}

fn matches_template(uri: &str, template: &str) -> bool {
    let prefix = template.split('{').next().unwrap_or(template);
    uri.starts_with(prefix)
}

fn to_json_string(result: LoomMcpActionResult) -> String {
    serde_json::to_string(&result).unwrap_or_else(|error| {
        serde_json::json!({
            "state": "failed",
            "projectRoot": "",
            "error": {
                "code": "serialization_failed",
                "message": error.to_string(),
                "targetBatch": Value::Null,
            }
        })
        .to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_templates_are_registered() {
        let templates = ResourceRegistry::batch_2().list_resource_templates();
        let uris: Vec<&str> = templates
            .resource_templates
            .iter()
            .map(|template| template.uri_template.as_str())
            .collect();
        assert_eq!(
            uris,
            vec![
                "loom://knowledge/{sourceId}/builds/{buildId}/chunks/{chunkId}",
                "loom://projects/{projectId}/requests/{requestId}/field-groups/{groupId}",
                "loom://projects/{projectId}/requests/{requestId}/fields/{encodedFieldPath}",
            ]
        );
    }
}
