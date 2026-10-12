use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use delivery_core::ProjectToolInput;
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;
use state::{
    paths::project_paths,
    store::{read_json_value, StateError, StateResult},
};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeliveryGraph {
    pub schema_version: u32,
    pub delivery_id: Option<String>,
    pub phase_id: Option<String>,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub mermaid: String,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    pub kind: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub relation: String,
}

#[derive(Default)]
struct Builder {
    nodes: BTreeMap<String, GraphNode>,
    edges: BTreeSet<GraphEdge>,
}

impl Builder {
    fn node(&mut self, kind: &str, key: &str, label: &str, status: Option<&str>) -> String {
        let id = format!("{kind}:{key}");
        self.nodes.entry(id.clone()).or_insert_with(|| GraphNode {
            id: id.clone(),
            kind: kind.to_string(),
            label: label.to_string(),
            status: status.map(str::to_string),
            summary: None,
        });
        id
    }

    fn summary(&mut self, id: &str, summary: Option<&str>) {
        if let Some(node) = self.nodes.get_mut(id) {
            node.summary = summary.map(str::to_string);
        }
    }

    fn edge(&mut self, from: &str, to: &str, relation: &str) {
        if self.nodes.contains_key(from) && self.nodes.contains_key(to) {
            self.edges.insert(GraphEdge {
                from: from.to_string(),
                to: to.to_string(),
                relation: relation.to_string(),
            });
        }
    }

    fn finish(self, delivery_id: Option<String>, phase_id: Option<String>) -> DeliveryGraph {
        let nodes: Vec<_> = self.nodes.into_values().collect();
        let edges: Vec<_> = self.edges.into_iter().collect();
        let ids: BTreeMap<_, _> = nodes
            .iter()
            .enumerate()
            .map(|(i, node)| (node.id.as_str(), format!("n{i}")))
            .collect();
        let mut mermaid = String::from("flowchart LR\n");
        for node in &nodes {
            let label = escape_mermaid(&format!("{}: {}", node.kind, node.label));
            mermaid.push_str(&format!("  {}[\"{}\"]\n", ids[node.id.as_str()], label));
        }
        for edge in &edges {
            mermaid.push_str(&format!(
                "  {} -->|{}| {}\n",
                ids[edge.from.as_str()],
                escape_mermaid(&edge.relation),
                ids[edge.to.as_str()]
            ));
        }
        DeliveryGraph {
            schema_version: 1,
            delivery_id,
            phase_id,
            nodes,
            edges,
            mermaid,
        }
    }
}

fn escape_mermaid(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('|', "&#124;")
        .replace(['\n', '\r'], " ")
}

fn items<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn field<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value
        .get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

fn safe_segment(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
}

fn read_ref(root: &Path, loom_dir: &Path, reference: &str) -> StateResult<Value> {
    let relative = state::paths::from_project_relative(root, reference)?;
    let canonical = fs::canonicalize(&relative)?;
    if !canonical.starts_with(fs::canonicalize(loom_dir)?)
        || canonical.extension().is_none_or(|ext| ext != "json")
    {
        return Err(StateError::InvalidArgument(format!(
            "reference outside .loom JSON: {reference}"
        )));
    }
    read_json_value(&canonical)
}

fn latest_ref(root: &Path, loom_dir: &Path, refs: &Value, key: &str) -> StateResult<Option<Value>> {
    field(refs, key)
        .map(|reference| read_ref(root, loom_dir, reference))
        .transpose()
}

struct PhaseSelection {
    paths: state::paths::ProjectPaths,
    delivery_id: String,
    phase_id: String,
    refs: Value,
}

fn select_phase(
    input: ProjectToolInput,
    requested_phase: Option<&str>,
) -> StateResult<Option<PhaseSelection>> {
    let paths = project_paths(&input.project_root)?;
    if requested_phase.is_some_and(|phase| !safe_segment(phase)) {
        return Err(StateError::InvalidArgument("invalid phase ID".into()));
    }
    if !paths.status_file.exists() {
        return Ok(None);
    }
    let status = read_json_value(&paths.status_file)?;
    let Some(delivery_id) =
        field(&status, "activeDeliveryId").or_else(|| field(&status, "lastCompletedDeliveryId"))
    else {
        return Ok(None);
    };
    if !safe_segment(delivery_id) {
        return Err(StateError::InvalidArgument("invalid delivery ID".into()));
    }
    let index_path = paths.deliveries_dir.join(delivery_id).join("index.json");
    let index = read_ref(
        &paths.root,
        &paths.loom_dir,
        &index_path
            .strip_prefix(&paths.root)
            .unwrap()
            .to_string_lossy(),
    )?;
    let phase_id = requested_phase
        .or_else(|| field(&index, "activePhaseId"))
        .ok_or_else(|| StateError::StateCorrupted("delivery index missing activePhaseId".into()))?;
    if !safe_segment(phase_id) {
        return Err(StateError::InvalidArgument("invalid phase ID".into()));
    }
    let phase = items(&index, "phases")
        .iter()
        .find(|phase| field(phase, "phaseId") == Some(phase_id))
        .ok_or_else(|| StateError::InvalidArgument(format!("phase not found: {phase_id}")))?;
    let refs = phase.get("latestRefs").cloned().unwrap_or(Value::Null);
    Ok(Some(PhaseSelection {
        paths,
        delivery_id: delivery_id.into(),
        phase_id: phase_id.into(),
        refs,
    }))
}

pub fn read(input: ProjectToolInput) -> StateResult<DeliveryGraph> {
    let mut graph = Builder::default();
    let Some(PhaseSelection {
        paths,
        delivery_id,
        phase_id,
        refs,
    }) = select_phase(input, None)?
    else {
        return Ok(graph.finish(None, None));
    };
    let architecture = latest_ref(&paths.root, &paths.loom_dir, &refs, "architectureArtifact")?;
    let plan = latest_ref(&paths.root, &paths.loom_dir, &refs, "taskPlan")?;
    let run = latest_ref(&paths.root, &paths.loom_dir, &refs, "taskPlanRun")?;
    let review = latest_ref(&paths.root, &paths.loom_dir, &refs, "reviewResult")?;

    if let Some(architecture) = &architecture {
        for module in items(architecture, "modules") {
            if let Some(id) = field(module, "moduleId") {
                graph.node("module", id, field(module, "name").unwrap_or(id), None);
            }
        }
        for flow in items(architecture, "userFlows") {
            if let Some(id) = field(flow, "flowId") {
                graph.node("flow", id, field(flow, "name").unwrap_or(id), None);
            }
        }
        for acceptance in items(architecture, "acceptanceMatrix") {
            let Some(id) = field(acceptance, "acceptanceId") else {
                continue;
            };
            let source = graph.node(
                "acceptance",
                id,
                field(acceptance, "statement").unwrap_or(id),
                field(acceptance, "coverageStatus"),
            );
            for coverage in items(acceptance, "coverage") {
                let kind = match field(coverage, "type") {
                    Some("module") => "module",
                    Some("user_flow") => "flow",
                    _ => continue,
                };
                for target in items(coverage, "refs").iter().filter_map(Value::as_str) {
                    graph.edge(&source, &format!("{kind}:{target}"), "covers");
                }
            }
        }
    }

    if let Some(plan) = &plan {
        for task in items(plan, "tasks") {
            let Some(id) = field(task, "taskId") else {
                continue;
            };
            graph.node("task", id, field(task, "title").unwrap_or(id), None);
        }
        for task in items(plan, "tasks") {
            let Some(id) = field(task, "taskId") else {
                continue;
            };
            let task_node = format!("task:{id}");
            for reference in items(task, "acceptanceRefs")
                .iter()
                .filter_map(Value::as_str)
            {
                graph.edge(&format!("acceptance:{reference}"), &task_node, "planned by");
            }
            for dependency in items(task, "dependsOn").iter().filter_map(Value::as_str) {
                graph.edge(&format!("task:{dependency}"), &task_node, "precedes");
            }
            if let Some(artifacts) = task
                .get("writeBoundary")
                .and_then(|boundary| boundary.get("artifactRefs"))
            {
                for (key, kind) in [("modules", "module"), ("userFlows", "flow")] {
                    for target in items(artifacts, key).iter().filter_map(Value::as_str) {
                        graph.edge(&task_node, &format!("{kind}:{target}"), "planned artifact");
                    }
                }
            }
        }
    }

    if let Some(run) = &run {
        let run_id = field(run, "runId").unwrap_or("");
        for task in items(run, "taskStates") {
            let (Some(task_id), Some(result_id)) = (field(task, "taskId"), field(task, "resultId"))
            else {
                continue;
            };
            if ![run_id, task_id, result_id]
                .iter()
                .all(|part| safe_segment(part))
            {
                continue;
            }
            let result_path: PathBuf = paths
                .deliveries_dir
                .join(&delivery_id)
                .join("tasks")
                .join(&phase_id)
                .join("results")
                .join(run_id)
                .join(task_id)
                .join(format!("{result_id}.json"));
            if !result_path.exists() {
                continue;
            }
            let relative = result_path
                .strip_prefix(&paths.root)
                .unwrap()
                .to_string_lossy();
            let result = read_ref(&paths.root, &paths.loom_dir, &relative)?;
            if field(&result, "taskId") != Some(task_id)
                || field(&result, "taskResultId") != Some(result_id)
            {
                continue;
            }
            let result_node = graph.node(
                "result",
                result_id,
                "Execution result",
                field(&result, "status"),
            );
            graph.edge(&format!("task:{task_id}"), &result_node, "produced");
            for file in items(&result, "changedFiles")
                .iter()
                .filter_map(Value::as_str)
            {
                let file_node = graph.node("file", file, file, None);
                graph.edge(&result_node, &file_node, "changed");
            }
            for verification in items(&result, "verificationResults") {
                if let Some(id) = field(verification, "verificationId") {
                    let verification_node = graph.node(
                        "verification",
                        id,
                        field(verification, "evidenceType").unwrap_or("Verification"),
                        field(verification, "status"),
                    );
                    graph.edge(&result_node, &verification_node, "verified by");
                }
            }
        }
    }

    if let Some(review) = &review {
        if let Some(id) = field(review, "reviewId") {
            let review_node =
                graph.node("review", id, "Delivery review", field(review, "decision"));
            if let Some(assessment) = review.get("coverageAssessment") {
                for acceptance in items(assessment, "mustAcceptance") {
                    if let Some(reference) = field(acceptance, "acceptanceRef") {
                        graph.edge(&format!("acceptance:{reference}"), &review_node, "reviewed");
                    }
                    for result in items(acceptance, "supportingTaskResults")
                        .iter()
                        .filter_map(Value::as_str)
                    {
                        graph.edge(&format!("result:{result}"), &review_node, "supports");
                    }
                }
            }
        }
    }
    Ok(graph.finish(Some(delivery_id.to_string()), Some(phase_id.to_string())))
}

/// Project the declared technical architecture without reading execution or review records.
pub fn read_architecture(
    input: ProjectToolInput,
    phase: Option<&str>,
) -> StateResult<DeliveryGraph> {
    let mut graph = Builder::default();
    let Some(PhaseSelection {
        paths,
        delivery_id,
        phase_id,
        refs,
    }) = select_phase(input, phase)?
    else {
        return Ok(graph.finish(None, None));
    };
    if let Some(architecture) =
        latest_ref(&paths.root, &paths.loom_dir, &refs, "architectureArtifact")?
    {
        project_architecture(&mut graph, &architecture);
    }
    Ok(graph.finish(Some(delivery_id), Some(phase_id)))
}

fn project_architecture(graph: &mut Builder, architecture: &Value) {
    let boundary = architecture
        .get("engineeringBoundary")
        .unwrap_or(&Value::Null);
    let data = architecture.get("dataModel").unwrap_or(&Value::Null);
    let data_architecture = data.get("dataArchitecture").unwrap_or(&Value::Null);
    for (collection, key, kind, parent) in [
        ("applications", "applicationId", "application", boundary),
        ("modules", "moduleId", "module", architecture),
        ("interfaces", "interfaceId", "interface", architecture),
        (
            "applicationInteractions",
            "interactionId",
            "interaction",
            boundary,
        ),
        ("entities", "entityId", "entity", data),
        ("userFlows", "flowId", "flow", architecture),
    ] {
        for item in items(parent, collection) {
            let Some(id) = field(item, key) else { continue };
            let label = field(item, "name")
                .or_else(|| field(item, "interactionType"))
                .unwrap_or(id);
            let node = graph.node(kind, id, label, None);
            let summary = field(item, "responsibility")
                .or_else(|| field(item, "protocol"))
                .or_else(|| field(item, "successOutcome"))
                .or_else(|| field(item, "rootPath"));
            graph.summary(&node, summary);
        }
    }
    // Source of truth is a declared persistence boundary, not an inferred database.
    if let Some(source) = field(data_architecture, "sourceOfTruth") {
        let node = graph.node("datastore", "source-of-truth", "Source of truth", None);
        graph.summary(&node, Some(source));
    }
    for interaction in items(boundary, "applicationInteractions") {
        let Some(id) = field(interaction, "interactionId") else {
            continue;
        };
        let target = format!("interaction:{id}");
        for (key, kind, relation) in [
            ("providerApplicationRef", "application", "provides"),
            ("providerModuleRef", "module", "implements"),
        ] {
            if let Some(provider) = field(interaction, key) {
                graph.edge(&format!("{kind}:{provider}"), &target, relation);
            }
        }
        for consumer in items(interaction, "consumerApplicationRefs")
            .iter()
            .filter_map(Value::as_str)
        {
            graph.edge(&format!("application:{consumer}"), &target, "consumes");
        }
        for interface in items(interaction, "interfaceRefs")
            .iter()
            .filter_map(Value::as_str)
        {
            graph.edge(&target, &format!("interface:{interface}"), "uses contract");
        }
    }
    for interface in items(architecture, "interfaces") {
        let Some(id) = field(interface, "interfaceId") else {
            continue;
        };
        let target = format!("interface:{id}");
        if let Some(provider) = field(interface, "providerModuleRef") {
            graph.edge(&format!("module:{provider}"), &target, "provides");
        }
        for (key, kind, relation) in [
            ("consumerModuleRefs", "module", "consumes"),
            ("moduleRefs", "module", "associated interface"),
            ("entityRefs", "entity", "contract data"),
        ] {
            for reference in items(interface, key).iter().filter_map(Value::as_str) {
                graph.edge(&format!("{kind}:{reference}"), &target, relation);
            }
        }
    }
    for ownership in items(data_architecture, "ownership") {
        if let (Some(owner), Some(data)) = (
            field(ownership, "ownerModuleRef"),
            field(ownership, "dataRef"),
        ) {
            graph.edge(
                &format!("module:{owner}"),
                &format!("entity:{data}"),
                "owns",
            );
        }
    }
    for read_model in items(data_architecture, "readModels") {
        if let Some(owner) = field(read_model, "ownerModuleRef") {
            for reference in items(read_model, "dataRefs")
                .iter()
                .filter_map(Value::as_str)
            {
                graph.edge(
                    &format!("module:{owner}"),
                    &format!("entity:{reference}"),
                    "reads",
                );
            }
        }
    }
    for relationship in items(data, "relationships") {
        if let (Some(from), Some(to)) = (
            field(relationship, "fromEntityRef"),
            field(relationship, "toEntityRef"),
        ) {
            graph.edge(
                &format!("entity:{from}"),
                &format!("entity:{to}"),
                field(relationship, "cardinality").unwrap_or("references"),
            );
        }
    }
    for flow in items(architecture, "userFlows") {
        let Some(id) = field(flow, "flowId") else {
            continue;
        };
        for (key, kind) in [
            ("moduleRefs", "module"),
            ("interfaceRefs", "interface"),
            ("entityRefs", "entity"),
        ] {
            for reference in items(flow, key).iter().filter_map(Value::as_str) {
                graph.edge(
                    &format!("flow:{id}"),
                    &format!("{kind}:{reference}"),
                    "involves",
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let suffix = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "loom-delivery-graph-{}-{suffix}",
                std::process::id()
            ));
            fs::create_dir_all(&root).unwrap();
            Self(root)
        }

        fn write(&self, path: &str, value: Value) {
            let target = self.0.join(path);
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, serde_json::to_vec(&value).unwrap()).unwrap();
        }

        fn read(&self) -> DeliveryGraph {
            read(ProjectToolInput {
                project_root: self.0.to_string_lossy().into_owned(),
            })
            .unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn empty_project_has_empty_graph() {
        let fixture = Fixture::new();
        let graph = fixture.read();
        assert!(graph.nodes.is_empty());
        assert!(graph.edges.is_empty());
        assert_eq!(graph.mermaid, "flowchart LR\n");
    }

    #[test]
    fn projects_only_explicit_links_and_tolerates_missing_results() {
        let fixture = Fixture::new();
        fixture.write(".loom/status.json", json!({"activeDeliveryId":"d1"}));
        fixture.write(
            ".loom/deliveries/d1/index.json",
            json!({
                "activePhaseId":"p1", "phases":[{"phaseId":"p1", "latestRefs":{
                    "architectureArtifact":".loom/deliveries/d1/a.json",
                    "taskPlan":".loom/deliveries/d1/t.json",
                    "taskPlanRun":".loom/deliveries/d1/r.json",
                    "reviewResult":".loom/deliveries/d1/v.json"
                }}]
            }),
        );
        fixture.write(".loom/deliveries/d1/a.json", json!({
            "modules":[{"moduleId":"m1"}],
            "acceptanceMatrix":[{"acceptanceId":"a1", "coverage":[{"type":"module","refs":["m1"]}]}]
        }));
        fixture.write(
            ".loom/deliveries/d1/t.json",
            json!({"tasks":[{
                "taskId":"t1", "acceptanceRefs":["a1"],
                "writeBoundary":{"artifactRefs":{"modules":["m1"]}}
            }]}),
        );
        fixture.write(
            ".loom/deliveries/d1/r.json",
            json!({
                "runId":"r1", "taskStates":[{"taskId":"t1","resultId":"res1"}]
            }),
        );
        fixture.write(
            ".loom/deliveries/d1/tasks/p1/results/r1/t1/res1.json",
            json!({
                "taskId":"t1", "taskResultId":"res1", "status":"completed",
                "changedFiles":["src/example.rs"],
                "verificationResults":[{"verificationId":"check1","status":"passed"}]
            }),
        );
        fixture.write(
            ".loom/deliveries/d1/v.json",
            json!({
                "reviewId":"v1", "decision":"approved", "coverageAssessment":{"mustAcceptance":[{
                    "acceptanceRef":"a1", "supportingTaskResults":["res1"]
                }]}
            }),
        );
        let graph = fixture.read();
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.from == "result:res1" && edge.to == "file:src/example.rs"));
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.from == "result:res1" && edge.to == "review:v1"));
        assert!(!graph
            .edges
            .iter()
            .any(|edge| edge.from == "file:src/example.rs" && edge.to == "module:m1"));
        assert_eq!(graph.nodes.len(), 7);

        fs::remove_file(
            fixture
                .0
                .join(".loom/deliveries/d1/tasks/p1/results/r1/t1/res1.json"),
        )
        .unwrap();
        let partial = fixture.read();
        assert!(!partial.nodes.iter().any(|node| node.id == "result:res1"));
        assert!(partial.nodes.iter().any(|node| node.id == "review:v1"));
    }

    #[test]
    fn rejects_refs_outside_loom() {
        let fixture = Fixture::new();
        fixture.write(".loom/status.json", json!({"activeDeliveryId":"d1"}));
        fixture.write(
            ".loom/deliveries/d1/index.json",
            json!({
                "activePhaseId":"p1", "phases":[{"phaseId":"p1", "latestRefs":{
                    "taskPlan":"../outside.json"
                }}]
            }),
        );
        assert!(read(ProjectToolInput {
            project_root: fixture.0.to_string_lossy().into_owned()
        })
        .is_err());
    }

    #[test]
    fn technical_architecture_uses_explicit_ownership_and_interface_references() {
        let mut builder = Builder::default();
        project_architecture(
            &mut builder,
            &json!({
                "engineeringBoundary": {
                    "applications": [{"applicationId":"api", "name":"API"}],
                    "applicationInteractions": [{
                        "interactionId":"create", "protocol":"HTTP", "providerApplicationRef":"api",
                        "providerModuleRef":"orders", "consumerApplicationRefs":["unknown"], "interfaceRefs":["post-order"]
                    }]
                },
                "modules": [{"moduleId":"orders", "name":"Orders", "responsibility":"Manage orders"}, {"moduleId":"billing"}],
                "interfaces": [{"interfaceId":"post-order", "providerModuleRef":"orders", "consumerModuleRefs":["billing"]}],
                "dataModel": {
                    "entities":[{"entityId":"order"},{"entityId":"payment"}],
                    "relationships":[{"fromEntityRef":"order", "toEntityRef":"payment", "cardinality":"one_to_many"}],
                    "dataArchitecture": {"sourceOfTruth":"orders.json", "ownership":[{"dataRef":"order", "ownerModuleRef":"orders"}]}
                },
                "userFlows":[{"flowId":"checkout", "moduleRefs":["orders"]}]
            }),
        );
        let graph = builder.finish(None, None);
        assert!(graph.edges.iter().any(|edge| edge.from == "module:orders"
            && edge.to == "entity:order"
            && edge.relation == "owns"));
        assert!(graph.edges.iter().any(|edge| edge.from == "module:billing"
            && edge.to == "interface:post-order"
            && edge.relation == "consumes"));
        assert!(graph.edges.iter().any(|edge| edge.from == "entity:order"
            && edge.to == "entity:payment"
            && edge.relation == "one_to_many"));
        assert!(!graph
            .edges
            .iter()
            .any(|edge| edge.from == "module:billing" && edge.to == "module:orders"));
        assert!(!graph
            .edges
            .iter()
            .any(|edge| edge.to == "datastore:source-of-truth"));
        assert!(!graph
            .nodes
            .iter()
            .any(|node| node.id == "application:unknown"));
        assert!(graph
            .edges
            .iter()
            .all(|edge| graph.nodes.iter().any(|node| node.id == edge.from)
                && graph.nodes.iter().any(|node| node.id == edge.to)));
        assert_eq!(
            graph
                .nodes
                .iter()
                .find(|node| node.id == "module:orders")
                .unwrap()
                .summary
                .as_deref(),
            Some("Manage orders")
        );
    }

    #[test]
    fn architecture_selects_requested_phase_without_changing_state() {
        let fixture = Fixture::new();
        fixture.write(".loom/status.json", json!({"activeDeliveryId":"d1"}));
        fixture.write(".loom/deliveries/d1/index.json", json!({
            "activePhaseId":"p2", "phases":[
                {"phaseId":"p1", "latestRefs":{"architectureArtifact":".loom/deliveries/d1/a.json"}},
                {"phaseId":"p2", "latestRefs":{}}
            ]
        }));
        fixture.write(
            ".loom/deliveries/d1/a.json",
            json!({"modules":[{"moduleId":"m1"}]}),
        );
        let status_before = fs::read(fixture.0.join(".loom/status.json")).unwrap();
        let input = || ProjectToolInput {
            project_root: fixture.0.to_string_lossy().into_owned(),
        };
        let current = read_architecture(input(), None).unwrap();
        assert_eq!(current.phase_id.as_deref(), Some("p2"));
        assert!(current.nodes.is_empty());
        let historical = read_architecture(input(), Some("p1")).unwrap();
        assert_eq!(historical.nodes[0].id, "module:m1");
        assert!(read_architecture(input(), Some("missing")).is_err());
        assert!(read_architecture(input(), Some("../p1")).is_err());
        assert_eq!(
            status_before,
            fs::read(fixture.0.join(".loom/status.json")).unwrap()
        );
    }

    #[test]
    fn real_delivery_fixture_when_available() {
        let Ok(project_root) = std::env::var("LOOM_DELIVERY_GRAPH_FIXTURE") else {
            return;
        };
        let graph = read(ProjectToolInput { project_root }).unwrap();
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.from == "acceptance:acc_1"
                && edge.to == "task:task-blueprint-validation-001"));
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.from.starts_with("result:")
                && edge.to == "file:src/flask/blueprints.py"));
        assert!(graph
            .edges
            .iter()
            .any(|edge| edge.from.starts_with("result:") && edge.to.starts_with("review:")));
        assert!(!graph
            .edges
            .iter()
            .any(|edge| edge.from.starts_with("file:") && edge.to.starts_with("module:")));
    }
}
