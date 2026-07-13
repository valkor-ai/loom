use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ReferenceLoadPlanItem;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BrowserInstallationStatus {
    Ready,
    Partial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BrowserRunnerSource {
    ExistingProject,
    BaselineSelected,
    LoomManaged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BrowserVerificationMode {
    SuiteSetup,
    BusinessFlow,
    RenderedInspection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BrowserCheckStatus {
    Passed,
    Failed,
    Blocked,
    NotRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BrowserBackendMode {
    Real,
    NotApplicable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case")]
pub enum BrowserTargetAvailability {
    Available,
    Unavailable,
    #[default]
    Unknown,
}

impl BrowserTargetAvailability {
    pub fn is_unknown(&self) -> bool {
        matches!(self, Self::Unknown)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BrowserAutomationInstallation {
    pub installation_id: String,
    pub status: BrowserInstallationStatus,
    pub package_root: String,
    pub package_manager: String,
    pub dependency_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependency_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub test_roots: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct BrowserAutomationFacts {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub installations: Vec<BrowserAutomationInstallation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub baseline_selection: Option<String>,
    #[serde(default, skip_serializing_if = "BrowserTargetAvailability::is_unknown")]
    pub target_availability: BrowserTargetAvailability,
}

impl BrowserAutomationFacts {
    pub fn is_empty(&self) -> bool {
        self.installations.is_empty()
            && self.baseline_selection.is_none()
            && self.target_availability.is_unknown()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BrowserVerificationCheck {
    pub check_id: String,
    pub verification_id: String,
    pub viewport_ref: String,
    pub backend_mode: BrowserBackendMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BrowserCheckResult {
    pub check_id: String,
    pub status: BrowserCheckStatus,
    pub command: String,
    pub attempts: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_refs: Vec<String>,
    pub observed_outcome: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocked_reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BrowserVerificationProfile {
    pub profile_id: String,
    pub task_id: String,
    pub mode: BrowserVerificationMode,
    pub runner_source: BrowserRunnerSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub verification_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub surface_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub workflow_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub region_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub action_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub state_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub quality_rule_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub checks: Vec<BrowserVerificationCheck>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reference_load_plan: Vec<ReferenceLoadPlanItem>,
}

pub fn playwright_reference_load_plan(
    mode: BrowserVerificationMode,
    runner_source: BrowserRunnerSource,
    action_refs: &[String],
    state_refs: &[String],
    quality_rule_refs: &[String],
) -> Vec<ReferenceLoadPlanItem> {
    let mut plan = vec![
        reference(
            "test.pw.core",
            "tech/test/playwright/core.md",
            "Playwright task boundary, runner adaptation, and deterministic execution rules.",
        ),
        reference(
            "test.pw.locators",
            "tech/test/playwright/locators.md",
            "Playwright locator, assertion, and auto-waiting rules for browser checks.",
        ),
    ];
    if matches!(mode, BrowserVerificationMode::SuiteSetup)
        || !matches!(runner_source, BrowserRunnerSource::ExistingProject)
    {
        plan.push(reference(
            "test.pw.config",
            "tech/test/playwright/configuration.md",
            "Playwright project configuration, web server, artifact, and CI setup rules.",
        ));
    }
    if matches!(mode, BrowserVerificationMode::BusinessFlow) {
        plan.push(reference(
            "test.pw.fixtures",
            "tech/test/playwright/fixtures.md",
            "Task-scoped fixture, authentication, test-data, and reusable workflow rules.",
        ));
    }
    if !action_refs.is_empty()
        || state_refs.iter().any(|state| {
            matches!(
                state.as_str(),
                "loading" | "error" | "business_blocking" | "submitting"
            )
        })
    {
        plan.push(reference(
            "test.pw.network",
            "tech/test/playwright/network.md",
            "Request synchronization, API-backed state control, and mock-versus-real boundary rules.",
        ));
    }
    if matches!(mode, BrowserVerificationMode::RenderedInspection)
        || quality_rule_refs
            .iter()
            .any(|rule| rule == "verify.rendered_viewports")
    {
        plan.push(reference(
            "test.pw.visual",
            "tech/test/playwright/visual.md",
            "Rendered viewport, screenshot, visual comparison, and layout stability rules.",
        ));
    }
    if quality_rule_refs
        .iter()
        .any(|rule| rule == "web.semantic_accessibility")
    {
        plan.push(reference(
            "test.pw.a11y",
            "tech/test/playwright/accessibility.md",
            "Browser-level semantic, keyboard, focus, and accessibility evidence rules.",
        ));
    }
    plan
}

fn reference(ref_id: &str, path: &str, reason: &str) -> ReferenceLoadPlanItem {
    ReferenceLoadPlanItem {
        ref_id: ref_id.to_string(),
        path: path.to_string(),
        reason: reason.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeSet, fs, path::Path};

    use super::*;

    #[test]
    fn playwright_references_exist_and_keep_operational_depth() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        for relative in [
            "core.md",
            "locators.md",
            "fixtures.md",
            "network.md",
            "configuration.md",
            "visual.md",
            "accessibility.md",
            "reliability.md",
        ] {
            let path = root
                .join("plugins/shared/loom/references/tech/test/playwright")
                .join(relative);
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            assert!(
                content.lines().count() >= 80,
                "{} is too thin to guide implementation",
                path.display()
            );
            for weak_heading in [
                "## Load Boundary",
                "## Source Coverage",
                "## Implementation Focus",
            ] {
                assert!(
                    !content.contains(weak_heading),
                    "{} contains generic metadata instead of operational guidance",
                    path.display()
                );
            }
        }
    }

    #[test]
    fn reference_plan_is_scenario_scoped_and_has_unique_ids() {
        let plan = playwright_reference_load_plan(
            BrowserVerificationMode::BusinessFlow,
            BrowserRunnerSource::LoomManaged,
            &["action.submit".to_string()],
            &["submitting".to_string()],
            &[
                "verify.rendered_viewports".to_string(),
                "web.semantic_accessibility".to_string(),
            ],
        );
        let ids = plan
            .iter()
            .map(|item| item.ref_id.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(ids.len(), plan.len());
        let paths = plan
            .iter()
            .map(|item| item.path.as_str())
            .collect::<BTreeSet<_>>();
        for expected in [
            "tech/test/playwright/core.md",
            "tech/test/playwright/locators.md",
            "tech/test/playwright/configuration.md",
            "tech/test/playwright/fixtures.md",
            "tech/test/playwright/network.md",
            "tech/test/playwright/visual.md",
            "tech/test/playwright/accessibility.md",
        ] {
            assert!(paths.contains(expected), "missing {expected}: {paths:#?}");
        }
        assert!(!paths.contains("tech/test/playwright/reliability.md"));
    }
}
