use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use contracts::{
    playwright_reference_load_plan, BrowserAutomationFacts, BrowserAutomationInstallation,
    BrowserBackendMode, BrowserEvidenceEnforcement, BrowserInstallationStatus, BrowserRunnerSource,
    BrowserTargetAvailability, BrowserVerificationCheck, BrowserVerificationMode,
    BrowserVerificationProfile, BrowserVersionResolutionSource, ImplementationAction,
    TaskDefinition, TaskKind, TechnicalBaselineContract,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const SKIPPED_DIRS: &[&str] = &[
    ".git",
    ".loom",
    "node_modules",
    "target",
    "dist",
    "build",
    "coverage",
];
const CONFIG_NAMES: &[&str] = &[
    "playwright.config.ts",
    "playwright.config.js",
    "playwright.config.mts",
    "playwright.config.cts",
    "playwright.config.mjs",
    "playwright.config.cjs",
];

pub(crate) fn scan_browser_automation_facts(
    project_root: &Path,
    baseline: &TechnicalBaselineContract,
) -> BrowserAutomationFacts {
    let mut manifests = Vec::new();
    collect_package_manifests(project_root, project_root, 0, &mut manifests);
    manifests.sort();
    let mut installations = manifests
        .iter()
        .filter_map(|manifest| installation_from_manifest(project_root, manifest))
        .collect::<Vec<_>>();
    installations.sort_by(|left, right| left.package_root.cmp(&right.package_root));
    let baseline_selection = baseline_browser_automation_selection(&baseline.stack);
    BrowserAutomationFacts {
        installations,
        target_availability: browser_target_availability(&baseline.stack, &baseline_selection),
        baseline_selection,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserRuntimeTarget {
    pub target_id: String,
    pub package_root: String,
    pub dependency_name: String,
    pub declared_version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_version: Option<String>,
    pub resolution_source: BrowserVersionResolutionSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BrowserRuntimePreparationState {
    Ready,
    Unavailable,
    NeedsPreparation,
}

impl BrowserRuntimeTarget {
    pub fn package_spec(&self) -> &str {
        self.resolved_version
            .as_deref()
            .unwrap_or(&self.declared_version)
    }
}

pub fn browser_runtime_targets(project_root: &Path) -> Vec<BrowserRuntimeTarget> {
    let mut manifests = Vec::new();
    collect_package_manifests(project_root, project_root, 0, &mut manifests);
    manifests.sort();
    manifests
        .into_iter()
        .filter_map(|manifest| {
            let package = fs::read(&manifest)
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())?;
            let package_root = manifest.parent()?;
            let (dependency_name, declared_version) = dependency_entry(&package)?;
            if !registry_version_spec(&declared_version) {
                return None;
            }
            let package_root_ref = project_relative(project_root, package_root)?;
            let (resolved_version, resolution_source) = resolve_project_playwright_version(
                project_root,
                package_root,
                &package_root_ref,
                &dependency_name,
                &declared_version,
            );
            Some(BrowserRuntimeTarget {
                target_id: format!(
                    "pw-target-{}-{:08x}",
                    stable_id_part(&package_root_ref),
                    stable_hash(&format!("{package_root_ref}:{dependency_name}"))
                ),
                package_root: package_root_ref,
                dependency_name,
                declared_version,
                resolved_version,
                resolution_source,
            })
        })
        .collect()
}

pub(crate) fn browser_runtime_preparation_state(
    project_root: &Path,
) -> BrowserRuntimePreparationState {
    let latest_path = project_root.join(".loom/runtime/browser-automation/latest.json");
    let Some(latest) = fs::read(&latest_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
    else {
        return BrowserRuntimePreparationState::NeedsPreparation;
    };
    let current_targets = serde_json::to_value(browser_runtime_targets(project_root))
        .unwrap_or_else(|_| Value::Array(vec![]));
    if latest.get("projectTargets") != Some(&current_targets) {
        return BrowserRuntimePreparationState::NeedsPreparation;
    }
    match latest.get("status").and_then(Value::as_str) {
        Some("ready" | "partial") => BrowserRuntimePreparationState::Ready,
        Some("unavailable") => BrowserRuntimePreparationState::Unavailable,
        _ => BrowserRuntimePreparationState::NeedsPreparation,
    }
}

pub(crate) fn derive_browser_verification_profiles(
    facts: &BrowserAutomationFacts,
    tasks: &[TaskDefinition],
) -> Vec<BrowserVerificationProfile> {
    tasks
        .iter()
        .filter_map(|task| derive_profile(facts, task))
        .collect()
}

fn collect_package_manifests(
    project_root: &Path,
    directory: &Path,
    depth: usize,
    output: &mut Vec<PathBuf>,
) {
    if depth > 6 {
        return;
    }
    let manifest = directory.join("package.json");
    if manifest.is_file() {
        output.push(manifest);
    }
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() || path.is_symlink() {
            continue;
        }
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if SKIPPED_DIRS.contains(&name.as_ref()) || (name.starts_with('.') && depth > 0) {
            continue;
        }
        if path.starts_with(project_root) {
            collect_package_manifests(project_root, &path, depth + 1, output);
        }
    }
}

fn installation_from_manifest(
    project_root: &Path,
    manifest_path: &Path,
) -> Option<BrowserAutomationInstallation> {
    let package = fs::read(manifest_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())?;
    let package_root_path = manifest_path.parent()?;
    let dependency = dependency_entry(&package);
    let config_ref = CONFIG_NAMES
        .iter()
        .map(|name| package_root_path.join(name))
        .find(|path| path.is_file())
        .and_then(|path| project_relative(project_root, &path));
    let scripts = package
        .get("scripts")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(name, command)| {
            command
                .as_str()
                .filter(|command| command.to_ascii_lowercase().contains("playwright"))
                .map(|_| name.to_string())
        })
        .collect::<Vec<_>>();
    if dependency.is_none() && config_ref.is_none() && scripts.is_empty() {
        return None;
    }
    let package_root = project_relative(project_root, package_root_path)?;
    let package_manager = package_manager(&package, project_root, package_root_path);
    let commands = scripts
        .iter()
        .map(|script| script_command(&package_manager, script))
        .collect::<Vec<_>>();
    let test_roots = playwright_test_roots(project_root, package_root_path);
    let mut evidence_refs = vec![project_relative(project_root, manifest_path)?];
    if let Some(config_ref) = &config_ref {
        evidence_refs.push(config_ref.clone());
    }
    if let Some(lockfile) = lockfile_ref(project_root, package_root_path) {
        evidence_refs.push(lockfile);
    }
    evidence_refs.sort();
    evidence_refs.dedup();
    let (dependency_name, declared_version, resolved_version, version_resolution_source) =
        dependency
            .map(|(name, version)| {
                let package_root_ref = project_relative(project_root, package_root_path)
                    .unwrap_or_else(|| ".".to_string());
                let (resolved, source) = resolve_project_playwright_version(
                    project_root,
                    package_root_path,
                    &package_root_ref,
                    &name,
                    &version,
                );
                (name, Some(version), resolved, Some(source))
            })
            .unwrap_or_else(|| ("@playwright/test".to_string(), None, None, None));
    let status = if resolved_version.is_some() && (!commands.is_empty() || config_ref.is_some()) {
        BrowserInstallationStatus::Ready
    } else {
        BrowserInstallationStatus::Partial
    };
    Some(BrowserAutomationInstallation {
        installation_id: format!(
            "pw-{}-{:08x}",
            stable_id_part(&package_root),
            stable_hash(&package_root)
        ),
        status,
        package_root,
        package_manager,
        dependency_name,
        declared_version,
        resolved_version,
        version_resolution_source,
        config_ref,
        test_roots,
        commands,
        evidence_refs,
    })
}

fn resolve_project_playwright_version(
    project_root: &Path,
    package_root: &Path,
    package_root_ref: &str,
    dependency_name: &str,
    declared_version: &str,
) -> (Option<String>, BrowserVersionResolutionSource) {
    if let Some(version) = installed_dependency_version(package_root, dependency_name) {
        return (
            Some(version),
            BrowserVersionResolutionSource::InstalledPackage,
        );
    }
    if let Some(version) = package_lock_dependency_version(
        project_root,
        package_root,
        package_root_ref,
        dependency_name,
    ) {
        return (Some(version), BrowserVersionResolutionSource::PackageLock);
    }
    if let Some(version) = pnpm_lock_dependency_version(
        project_root,
        package_root,
        package_root_ref,
        dependency_name,
    ) {
        return (Some(version), BrowserVersionResolutionSource::PnpmLock);
    }
    if exact_registry_version(declared_version) {
        return (
            Some(declared_version.trim_start_matches('v').to_string()),
            BrowserVersionResolutionSource::ExactManifest,
        );
    }
    (
        None,
        BrowserVersionResolutionSource::RegistryResolutionRequired,
    )
}

fn package_lock_dependency_version(
    project_root: &Path,
    package_root: &Path,
    package_root_ref: &str,
    dependency_name: &str,
) -> Option<String> {
    for lock_root in unique_roots(package_root, project_root) {
        let Some(lock) = fs::read(lock_root.join("package-lock.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        else {
            continue;
        };
        let workspace_prefix = if lock_root == project_root && package_root_ref != "." {
            format!("{package_root_ref}/")
        } else {
            String::new()
        };
        for package_key in [
            format!("{workspace_prefix}node_modules/{dependency_name}"),
            format!("node_modules/{dependency_name}"),
        ] {
            if let Some(version) = lock
                .pointer(&format!("/packages/{}", json_pointer_escape(&package_key)))
                .and_then(|entry| entry.get("version"))
                .and_then(Value::as_str)
                .filter(|version| exact_registry_version(version))
            {
                return Some(version.to_string());
            }
        }
        if let Some(version) = lock
            .get("dependencies")
            .and_then(|dependencies| dependencies.get(dependency_name))
            .and_then(|entry| entry.get("version"))
            .and_then(Value::as_str)
            .filter(|version| exact_registry_version(version))
        {
            return Some(version.to_string());
        }
    }
    None
}

fn pnpm_lock_dependency_version(
    project_root: &Path,
    package_root: &Path,
    package_root_ref: &str,
    dependency_name: &str,
) -> Option<String> {
    for lock_root in unique_roots(package_root, project_root) {
        let Some(lock) = fs::read_to_string(lock_root.join("pnpm-lock.yaml"))
            .ok()
            .and_then(|text| serde_yaml::from_str::<Value>(&text).ok())
        else {
            continue;
        };
        let importer_key = if lock_root == project_root {
            package_root_ref
        } else {
            "."
        };
        for section in ["devDependencies", "dependencies", "optionalDependencies"] {
            let dependency = lock
                .get("importers")
                .and_then(|value| value.get(importer_key))
                .and_then(|value| value.get(section))
                .and_then(|value| value.get(dependency_name));
            let version = dependency
                .and_then(Value::as_str)
                .or_else(|| {
                    dependency
                        .and_then(|value| value.get("version"))
                        .and_then(Value::as_str)
                })
                .and_then(normalize_pnpm_version);
            if version.is_some() {
                return version;
            }
        }
    }
    None
}

fn normalize_pnpm_version(value: &str) -> Option<String> {
    let version = value
        .trim()
        .trim_start_matches("npm:")
        .split('(')
        .next()
        .unwrap_or_default()
        .trim_start_matches('v');
    exact_registry_version(version).then(|| version.to_string())
}

fn unique_roots<'a>(first: &'a Path, second: &'a Path) -> Vec<&'a Path> {
    if first == second {
        vec![first]
    } else {
        vec![first, second]
    }
}

fn json_pointer_escape(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn exact_registry_version(value: &str) -> bool {
    let value = value.trim().trim_start_matches('v');
    let core = value.split(['-', '+']).next().unwrap_or_default();
    let parts = core.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty() && part.chars().all(|character| character.is_ascii_digit())
        })
}

fn installed_dependency_version(package_root: &Path, dependency_name: &str) -> Option<String> {
    let package_path = dependency_name
        .split('/')
        .fold(package_root.join("node_modules"), |path, part| {
            path.join(part)
        })
        .join("package.json");
    fs::read(package_path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|package| {
            package
                .get("version")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

fn registry_version_spec(value: &str) -> bool {
    let lower = value.trim().to_ascii_lowercase();
    !lower.is_empty()
        && !value.chars().any(char::is_whitespace)
        && !lower.starts_with("file:")
        && !lower.starts_with("link:")
        && !lower.starts_with("workspace:")
        && !lower.starts_with("git")
        && !lower.starts_with("http:")
        && !lower.starts_with("https:")
}

fn dependency_entry(package: &Value) -> Option<(String, String)> {
    for section in ["devDependencies", "dependencies", "peerDependencies"] {
        let Some(dependencies) = package.get(section).and_then(Value::as_object) else {
            continue;
        };
        for name in ["@playwright/test", "playwright"] {
            if let Some(version) = dependencies.get(name).and_then(Value::as_str) {
                return Some((name.to_string(), version.to_string()));
            }
        }
    }
    None
}

fn package_manager(package: &Value, project_root: &Path, package_root: &Path) -> String {
    if let Some(manager) = package
        .get("packageManager")
        .and_then(Value::as_str)
        .and_then(|value| value.split('@').next())
        .filter(|value| matches!(*value, "npm" | "pnpm" | "yarn" | "bun"))
    {
        return manager.to_string();
    }
    for root in [package_root, project_root] {
        for (file, manager) in [
            ("pnpm-lock.yaml", "pnpm"),
            ("yarn.lock", "yarn"),
            ("bun.lockb", "bun"),
            ("bun.lock", "bun"),
            ("package-lock.json", "npm"),
            ("npm-shrinkwrap.json", "npm"),
        ] {
            if root.join(file).is_file() {
                return manager.to_string();
            }
        }
    }
    "npm".to_string()
}

fn script_command(package_manager: &str, script: &str) -> String {
    match package_manager {
        "yarn" => format!("yarn {script}"),
        "pnpm" => format!("pnpm {script}"),
        "bun" => format!("bun run {script}"),
        _ => format!("npm run {script}"),
    }
}

fn lockfile_ref(project_root: &Path, package_root: &Path) -> Option<String> {
    for root in [package_root, project_root] {
        for file in [
            "pnpm-lock.yaml",
            "yarn.lock",
            "bun.lockb",
            "bun.lock",
            "package-lock.json",
            "npm-shrinkwrap.json",
        ] {
            let path = root.join(file);
            if path.is_file() {
                return project_relative(project_root, &path);
            }
        }
    }
    None
}

fn playwright_test_roots(project_root: &Path, package_root: &Path) -> Vec<String> {
    let mut roots = BTreeSet::new();
    for relative in ["e2e", "tests/e2e", "playwright", "tests"] {
        let path = package_root.join(relative);
        if path.is_dir() && directory_contains_playwright_test(&path, 0) {
            if let Some(path) = project_relative(project_root, &path) {
                roots.insert(path);
            }
        }
    }
    roots.into_iter().collect()
}

fn directory_contains_playwright_test(directory: &Path, depth: usize) -> bool {
    if depth > 3 {
        return false;
    }
    let Ok(entries) = fs::read_dir(directory) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if directory_contains_playwright_test(&path, depth + 1) {
                return true;
            }
            continue;
        }
        let is_test = path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.contains(".spec.") || name.contains(".test."));
        if is_test
            && fs::read_to_string(&path)
                .ok()
                .is_some_and(|text| text.contains("@playwright/test"))
        {
            return true;
        }
    }
    false
}

fn baseline_browser_automation_selection(stack: &Value) -> Option<String> {
    [
        stack.pointer("/tracks/qualityAutomation/selection"),
        stack.get("qualityAutomation"),
    ]
    .into_iter()
    .flatten()
    .find_map(|value| value.as_str())
    .map(str::trim)
    .filter(|value| !value.is_empty())
    .map(str::to_string)
}

fn browser_target_availability(
    stack: &Value,
    baseline_selection: &Option<String>,
) -> BrowserTargetAvailability {
    if let Some(status) = stack.pointer("/tracks/web/status").and_then(Value::as_str) {
        return match status {
            "selected" | "user_custom" => BrowserTargetAvailability::Available,
            "not_needed" | "not_applicable" => BrowserTargetAvailability::Unavailable,
            _ => BrowserTargetAvailability::Unknown,
        };
    }
    if stack
        .pointer("/tracks/web/selection")
        .and_then(Value::as_str)
        .is_some_and(|selection| !selection.trim().is_empty())
        || stack
            .get("web")
            .and_then(Value::as_str)
            .is_some_and(|selection| !selection.trim().is_empty())
        || baseline_selection
            .as_deref()
            .is_some_and(|selection| selection.to_ascii_lowercase().contains("playwright"))
    {
        return BrowserTargetAvailability::Available;
    }
    BrowserTargetAvailability::Unknown
}

fn derive_profile(
    facts: &BrowserAutomationFacts,
    task: &TaskDefinition,
) -> Option<BrowserVerificationProfile> {
    if !task_requires_browser_verification(task) {
        return None;
    }
    if facts.target_availability == BrowserTargetAvailability::Unavailable {
        return None;
    }
    if facts
        .baseline_selection
        .as_deref()
        .is_some_and(|selection| !selection.to_ascii_lowercase().contains("playwright"))
    {
        return None;
    }
    let requirement = task.frontend_experience_requirement.as_ref()?;
    let scope = requirement.get("uiTaskScope").unwrap_or(&Value::Null);
    let region_refs = object_id_array(scope, "regionsInScope", "regionId");
    let action_refs = object_id_array(scope, "actionsInContract", "actionId");
    let state_refs = object_id_array(scope, "statesInContract", "state");
    let quality_rule_refs = object_id_array(scope, "qualityRulesInScope", "ruleId");
    let surface_refs = string_array(scope, "surfacesInScope");
    let workflow_refs = string_array(scope, "workflowsInScope");
    let owns_suite_setup = matches!(task.task_kind, TaskKind::VerificationIncrement)
        && task.implementation_actions.iter().any(|action| {
            matches!(
                action,
                ImplementationAction::AddOrUpdateTests | ImplementationAction::AddOrUpdateConfig
            )
        });
    let business_flow = !action_refs.is_empty()
        || !workflow_refs.is_empty()
        || requirement
            .pointer("/uiTaskScope/frontendBackendBindings")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
        || task
            .implementation_actions
            .iter()
            .any(|action| matches!(action, ImplementationAction::WireReferenceInApiOrUi));
    let mode = if owns_suite_setup {
        BrowserVerificationMode::SuiteSetup
    } else if business_flow {
        BrowserVerificationMode::BusinessFlow
    } else {
        BrowserVerificationMode::RenderedInspection
    };
    let (runner_source, installation_id) = select_runner(facts);
    let responsive = scope
        .get("responsiveCoverageRequired")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let viewport_refs = if responsive {
        vec!["desktop_primary", "mobile_primary"]
    } else {
        vec!["desktop_primary"]
    };
    let backend_mode = if business_flow {
        BrowserBackendMode::Real
    } else {
        BrowserBackendMode::NotApplicable
    };
    let explicit_verification_ids = task
        .verification_intents
        .iter()
        .filter(|intent| {
            intent
                .preferred_evidence
                .iter()
                .chain(intent.acceptable_evidence.iter())
                .any(|evidence| {
                    matches!(evidence, contracts::VerificationEvidence::BrowserAutomation)
                })
        })
        .map(|intent| intent.verification_id.clone())
        .collect::<Vec<_>>();
    let verification_ids = if !explicit_verification_ids.is_empty() {
        explicit_verification_ids
    } else if quality_rule_refs
        .iter()
        .any(|rule| rule == "verify.rendered_viewports")
    {
        // Rendered quality is an MCP-owned closure obligation. When the
        // agent has supplied ordinary verification intents but did not pick
        // a browser owner, the closure preserves those behaviors and owns the
        // browser evidence itself.
        task.verification_intents
            .iter()
            .map(|intent| intent.verification_id.clone())
            .collect()
    } else {
        Vec::new()
    };
    if verification_ids.is_empty() {
        return None;
    }
    let mut checks = Vec::new();
    for verification_id in &verification_ids {
        let enforcement = task
            .verification_intents
            .iter()
            .find(|intent| intent.verification_id == *verification_id)
            .map(|intent| {
                if intent.preferred_evidence.iter().any(|evidence| {
                    matches!(evidence, contracts::VerificationEvidence::BrowserAutomation)
                }) || quality_rule_refs
                    .iter()
                    .any(|rule| rule == "verify.rendered_viewports")
                {
                    BrowserEvidenceEnforcement::Required
                } else {
                    BrowserEvidenceEnforcement::Supplemental
                }
            })
            .unwrap_or(BrowserEvidenceEnforcement::Supplemental);
        for viewport_ref in &viewport_refs {
            checks.push(BrowserVerificationCheck {
                check_id: format!(
                    "browser-{}-{}-{}",
                    stable_id_part(&task.task_id),
                    stable_id_part(verification_id),
                    viewport_ref
                ),
                verification_id: verification_id.clone(),
                source_task_id: task.task_id.clone(),
                source_verification_id: verification_id.clone(),
                enforcement,
                viewport_ref: (*viewport_ref).to_string(),
                backend_mode,
            });
        }
    }
    let reference_load_plan = playwright_reference_load_plan(
        mode,
        runner_source,
        &action_refs,
        &state_refs,
        &quality_rule_refs,
    );
    Some(BrowserVerificationProfile {
        profile_id: format!("browser-{}", stable_id_part(&task.task_id)),
        task_id: task.task_id.clone(),
        mode,
        runner_source,
        installation_id,
        verification_ids,
        surface_refs,
        workflow_refs,
        region_refs,
        action_refs,
        state_refs,
        quality_rule_refs,
        checks,
        reference_load_plan,
    })
}

pub(crate) fn task_requires_browser_verification(task: &TaskDefinition) -> bool {
    let explicitly_requests_browser_evidence = task.verification_intents.iter().any(|intent| {
        intent
            .preferred_evidence
            .iter()
            .chain(intent.acceptable_evidence.iter())
            .any(|evidence| matches!(evidence, contracts::VerificationEvidence::BrowserAutomation))
    });
    let owns_browser_suite_setup = matches!(task.task_kind, TaskKind::VerificationIncrement)
        && task.implementation_actions.iter().any(|action| {
            matches!(
                action,
                ImplementationAction::AddOrUpdateTests | ImplementationAction::AddOrUpdateConfig
            )
        });
    let owns_rendered_quality_rule = task
        .frontend_experience_requirement
        .as_ref()
        .and_then(|requirement| requirement.get("uiTaskScope"))
        .is_some_and(|scope| {
            object_id_array(scope, "qualityRulesInScope", "ruleId")
                .iter()
                .any(|rule| rule == "verify.rendered_viewports")
        });
    let owns_browser_verification = explicitly_requests_browser_evidence
        || owns_browser_suite_setup
        || owns_rendered_quality_rule;
    if !owns_browser_verification {
        return false;
    }
    let Some(requirement) = task.frontend_experience_requirement.as_ref() else {
        return false;
    };
    let scope = requirement.get("uiTaskScope").unwrap_or(&Value::Null);
    let owns_suite_setup = matches!(task.task_kind, TaskKind::VerificationIncrement)
        && task.implementation_actions.iter().any(|action| {
            matches!(
                action,
                ImplementationAction::AddOrUpdateTests | ImplementationAction::AddOrUpdateConfig
            )
        });
    owns_suite_setup
        || !object_id_array(scope, "regionsInScope", "regionId").is_empty()
        || !object_id_array(scope, "actionsInContract", "actionId").is_empty()
        || !object_id_array(scope, "statesInContract", "state").is_empty()
        || !string_array(scope, "surfacesInScope").is_empty()
        || !string_array(scope, "workflowsInScope").is_empty()
        || object_id_array(scope, "qualityRulesInScope", "ruleId")
            .iter()
            .any(|rule| rule == "verify.rendered_viewports")
}

fn object_id_array(value: &Value, key: &str, id_key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get(id_key).and_then(Value::as_str).map(str::to_string))
        .collect()
}

fn select_runner(facts: &BrowserAutomationFacts) -> (BrowserRunnerSource, Option<String>) {
    let ready = facts
        .installations
        .iter()
        .filter(|installation| installation.status == BrowserInstallationStatus::Ready)
        .collect::<Vec<_>>();
    if ready.len() == 1 {
        return (
            BrowserRunnerSource::ExistingProject,
            Some(ready[0].installation_id.clone()),
        );
    }
    if let Some(root) = ready
        .iter()
        .find(|installation| installation.package_root == ".")
    {
        return (
            BrowserRunnerSource::ExistingProject,
            Some(root.installation_id.clone()),
        );
    }
    if facts
        .baseline_selection
        .as_deref()
        .is_some_and(|selection| selection.to_ascii_lowercase().contains("playwright"))
    {
        return (BrowserRunnerSource::BaselineSelected, None);
    }
    (BrowserRunnerSource::LoomManaged, None)
}

fn string_array(value: &Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn project_relative(project_root: &Path, path: &Path) -> Option<String> {
    let relative = path.strip_prefix(project_root).ok()?;
    let value = relative.to_string_lossy().replace('\\', "/");
    Some(if value.is_empty() {
        ".".to_string()
    } else {
        value
    })
}

fn stable_id_part(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let normalized = normalized.trim_matches('-');
    if normalized.is_empty() {
        "root".to_string()
    } else {
        normalized.to_string()
    }
}

fn stable_hash(value: &str) -> u32 {
    value.as_bytes().iter().fold(0x811c9dc5, |hash, byte| {
        (hash ^ u32::from(*byte)).wrapping_mul(0x01000193)
    })
}

#[cfg(test)]
mod tests {
    use std::{env, fs};

    use contracts::{
        TaskArtifactRefs, TaskWriteBoundary, TechnicalBaselineApproval,
        TechnicalBaselineApprovalType, TechnicalBaselineScope, TechnicalBaselineSource,
        TechnicalBaselineStatus,
    };
    use serde_json::json;

    use super::*;

    #[test]
    fn scans_existing_playwright_installation_from_structured_files() {
        let root = fixture_root("scan");
        fs::create_dir_all(root.join("web/e2e")).unwrap();
        fs::write(
            root.join("web/package.json"),
            r#"{
                "packageManager": "pnpm@10.0.0",
                "scripts": {"test:e2e": "playwright test"},
                "devDependencies": {"@playwright/test": "1.55.0"}
            }"#,
        )
        .unwrap();
        fs::write(root.join("web/pnpm-lock.yaml"), "lockfileVersion: '9.0'").unwrap();
        fs::write(root.join("web/playwright.config.ts"), "export default {};").unwrap();
        fs::write(
            root.join("web/e2e/workflow.spec.ts"),
            "import { test } from '@playwright/test';",
        )
        .unwrap();

        let facts = scan_browser_automation_facts(&root, &baseline(json!({})));
        assert_eq!(facts.installations.len(), 1);
        let installation = &facts.installations[0];
        assert_eq!(installation.status, BrowserInstallationStatus::Ready);
        assert_eq!(installation.package_root, "web");
        assert_eq!(installation.package_manager, "pnpm");
        assert_eq!(installation.commands, vec!["pnpm test:e2e"]);
        assert_eq!(
            installation.config_ref.as_deref(),
            Some("web/playwright.config.ts")
        );
        assert_eq!(installation.test_roots, vec!["web/e2e"]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn runtime_targets_prefer_installed_exact_version_and_ignore_local_specs() {
        let root = fixture_root("runtime-specs");
        fs::create_dir_all(root.join("web/node_modules/@playwright/test")).unwrap();
        fs::create_dir_all(root.join("local")).unwrap();
        fs::write(
            root.join("web/package.json"),
            r#"{"devDependencies":{"@playwright/test":"^1.55.0"}}"#,
        )
        .unwrap();
        fs::write(
            root.join("web/node_modules/@playwright/test/package.json"),
            r#"{"name":"@playwright/test","version":"1.55.1"}"#,
        )
        .unwrap();
        fs::write(
            root.join("local/package.json"),
            r#"{"devDependencies":{"@playwright/test":"workspace:*"}}"#,
        )
        .unwrap();

        let targets = browser_runtime_targets(&root);
        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].package_root, "web");
        assert_eq!(targets[0].declared_version, "^1.55.0");
        assert_eq!(targets[0].resolved_version.as_deref(), Some("1.55.1"));
        assert_eq!(
            targets[0].resolution_source,
            BrowserVersionResolutionSource::InstalledPackage
        );

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn runtime_target_resolves_workspace_version_from_package_lock() {
        let root = fixture_root("runtime-package-lock");
        fs::create_dir_all(root.join("web")).unwrap();
        fs::write(
            root.join("web/package.json"),
            r#"{"devDependencies":{"@playwright/test":"^1.55.0"}}"#,
        )
        .unwrap();
        fs::write(
            root.join("package-lock.json"),
            r#"{
                "lockfileVersion": 3,
                "packages": {
                    "node_modules/@playwright/test": {"version": "1.55.1"}
                }
            }"#,
        )
        .unwrap();

        let targets = browser_runtime_targets(&root);

        assert_eq!(targets[0].resolved_version.as_deref(), Some("1.55.1"));
        assert_eq!(
            targets[0].resolution_source,
            BrowserVersionResolutionSource::PackageLock
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn runtime_target_resolves_workspace_version_from_pnpm_importer() {
        let root = fixture_root("runtime-pnpm-lock");
        fs::create_dir_all(root.join("web")).unwrap();
        fs::write(
            root.join("web/package.json"),
            r#"{"devDependencies":{"@playwright/test":"^1.55.0"}}"#,
        )
        .unwrap();
        fs::write(
            root.join("pnpm-lock.yaml"),
            r#"lockfileVersion: '9.0'
importers:
  web:
    devDependencies:
      '@playwright/test':
        specifier: ^1.55.0
        version: 1.55.2
"#,
        )
        .unwrap();

        let targets = browser_runtime_targets(&root);

        assert_eq!(targets[0].resolved_version.as_deref(), Some("1.55.2"));
        assert_eq!(
            targets[0].resolution_source,
            BrowserVersionResolutionSource::PnpmLock
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn runtime_preparation_state_requires_fresh_project_targets() {
        let root = fixture_root("runtime-preparation-state");
        fs::create_dir_all(root.join(".loom/runtime/browser-automation")).unwrap();
        fs::write(
            root.join("package.json"),
            r#"{"devDependencies":{"@playwright/test":"1.55.0"}}"#,
        )
        .unwrap();
        assert_eq!(
            browser_runtime_preparation_state(&root),
            BrowserRuntimePreparationState::NeedsPreparation
        );
        let targets = browser_runtime_targets(&root);
        fs::write(
            root.join(".loom/runtime/browser-automation/latest.json"),
            serde_json::to_vec(&json!({
                "status": "ready",
                "projectTargets": targets
            }))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            browser_runtime_preparation_state(&root),
            BrowserRuntimePreparationState::Ready
        );
        fs::write(
            root.join(".loom/runtime/browser-automation/latest.json"),
            serde_json::to_vec(&json!({
                "status": "partial",
                "projectTargets": targets
            }))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            browser_runtime_preparation_state(&root),
            BrowserRuntimePreparationState::Ready
        );
        fs::write(
            root.join("package.json"),
            r#"{"devDependencies":{"@playwright/test":"1.56.0"}}"#,
        )
        .unwrap();
        assert_eq!(
            browser_runtime_preparation_state(&root),
            BrowserRuntimePreparationState::NeedsPreparation
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn derives_profiles_without_agent_authored_runner_fields() {
        let root = fixture_root("profile");
        fs::create_dir_all(&root).unwrap();
        let facts = scan_browser_automation_facts(
            &root,
            &baseline(json!({
                "tracks": {"qualityAutomation": {"selection": "Playwright"}}
            })),
        );
        let mut task = task();
        task.frontend_experience_requirement = Some(json!({
            "uiTaskScope": {
                "surfacesInScope": ["surface-workbench"],
                "workflowsInScope": ["flow-create"],
                "frontendBackendBindings": [{"interfaceRef": "api.create"}]
            },
            "uiTaskScope": {
                "regionsInScope": [{"regionId": "region-main"}],
                "actionsInContract": [{"actionId": "action-create"}],
                "statesInContract": [{"state": "submitting"}, {"state": "success"}],
                "qualityRulesInScope": [{"ruleId": "verify.rendered_viewports"}],
                "responsiveCoverageRequired": true
            }
        }));
        let profiles = derive_browser_verification_profiles(&facts, &[task]);
        assert_eq!(profiles.len(), 1);
        let profile = &profiles[0];
        assert_eq!(profile.runner_source, BrowserRunnerSource::BaselineSelected);
        assert_eq!(profile.mode, BrowserVerificationMode::BusinessFlow);
        assert_eq!(profile.checks.len(), 2);
        assert!(profile
            .checks
            .iter()
            .all(|check| check.backend_mode == BrowserBackendMode::Real));
        let paths = profile
            .reference_load_plan
            .iter()
            .map(|item| item.path.as_str())
            .collect::<Vec<_>>();
        assert!(paths.contains(&"tech/test/playwright/core.md"));
        assert!(paths.contains(&"tech/test/playwright/locators.md"));
        assert!(paths.contains(&"tech/test/playwright/configuration.md"));
        assert!(paths.contains(&"tech/test/playwright/fixtures.md"));
        assert!(paths.contains(&"tech/test/playwright/network.md"));
        assert!(paths.contains(&"tech/test/playwright/visual.md"));
        assert!(!paths.contains(&"tech/test/playwright/reliability.md"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn browser_profiles_and_references_are_not_selected_without_owned_ui_scope() {
        let facts = BrowserAutomationFacts::default();
        assert!(derive_browser_verification_profiles(&facts, &[task()]).is_empty());

        let plan = playwright_reference_load_plan(
            BrowserVerificationMode::RenderedInspection,
            BrowserRunnerSource::LoomManaged,
            &[],
            &[],
            &["verify.rendered_viewports".to_string()],
        );
        let paths = plan
            .iter()
            .map(|item| item.path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                "tech/test/playwright/core.md",
                "tech/test/playwright/locators.md",
                "tech/test/playwright/configuration.md",
                "tech/test/playwright/visual.md",
            ]
        );
    }

    #[test]
    fn ui_scope_without_test_or_browser_evidence_does_not_select_playwright() {
        let mut task = task();
        task.frontend_experience_requirement = Some(json!({
            "uiTaskScope": {"surfacesInScope": ["surface-workbench"], "regionsInScope": [{"regionId": "region-main"}]}
        }));
        task.verification_intents[0].acceptable_evidence =
            vec![contracts::VerificationEvidence::AutomatedTest];

        assert!(!task_requires_browser_verification(&task));
        assert!(
            derive_browser_verification_profiles(&BrowserAutomationFacts::default(), &[task])
                .is_empty()
        );
    }

    #[test]
    fn generic_test_implementation_does_not_imply_browser_ownership() {
        let mut task = task();
        task.frontend_experience_requirement = Some(json!({
            "uiTaskScope": {"surfacesInScope": ["surface-workbench"], "regionsInScope": [{"regionId": "region-main"}]}
        }));
        task.implementation_actions
            .push(ImplementationAction::AddOrUpdateTests);
        task.verification_intents[0].acceptable_evidence =
            vec![contracts::VerificationEvidence::AutomatedTest];

        assert!(!task_requires_browser_verification(&task));
        assert!(
            derive_browser_verification_profiles(&BrowserAutomationFacts::default(), &[task])
                .is_empty()
        );
    }

    #[test]
    fn explicit_non_playwright_baseline_does_not_select_playwright_profile() {
        let root = fixture_root("non-playwright-baseline");
        fs::create_dir_all(&root).unwrap();
        let facts = scan_browser_automation_facts(
            &root,
            &baseline(json!({
                "tracks": {"qualityAutomation": {"selection": "Cypress"}}
            })),
        );
        let mut task = task();
        task.frontend_experience_requirement = Some(json!({
            "uiTaskScope": {"surfacesInScope": ["surface-storefront"], "regionsInScope": [{"regionId": "region-catalog"}]}
        }));

        assert_eq!(facts.baseline_selection.as_deref(), Some("Cypress"));
        assert!(derive_browser_verification_profiles(&facts, &[task]).is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_only_baseline_does_not_select_browser_profile() {
        let root = fixture_root("native-only-baseline");
        fs::create_dir_all(&root).unwrap();
        let facts = scan_browser_automation_facts(
            &root,
            &baseline(json!({
                "tracks": {
                    "web": {"status": "not_needed", "selection": "Not needed"},
                    "app": {"status": "selected", "selection": "Flutter"}
                }
            })),
        );
        let mut task = task();
        task.frontend_experience_requirement = Some(json!({
            "uiTaskScope": {"surfacesInScope": ["surface-mobile-home"], "regionsInScope": [{"regionId": "region-mobile-primary"}]}
        }));

        assert_eq!(
            facts.target_availability,
            BrowserTargetAvailability::Unavailable
        );
        assert!(derive_browser_verification_profiles(&facts, &[task]).is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    fn baseline(stack: Value) -> TechnicalBaselineContract {
        TechnicalBaselineContract {
            schema_version: "1.0".to_string(),
            technical_baseline_id: "baseline".to_string(),
            delivery_id: "delivery".to_string(),
            phase_id: "phase".to_string(),
            status: TechnicalBaselineStatus::Confirmed,
            source: TechnicalBaselineSource::UserConfirmed,
            project_kind: contracts::ProjectKind::ExistingProject,
            scope: TechnicalBaselineScope::Project,
            stack,
            security_profiles: vec![],
            constraints: vec![],
            evidence: vec![],
            approval: TechnicalBaselineApproval {
                r#type: TechnicalBaselineApprovalType::UserConfirmed,
                confirmed_at: Some("2026-07-13T00:00:00Z".to_string()),
                reason: None,
            },
            confidence: contracts::ConfidenceLevel::High,
            requires_user_confirmation: None,
            reasoning_summary: vec![],
            alternatives: vec![],
            created_at: "2026-07-13T00:00:00Z".to_string(),
            updated_at: "2026-07-13T00:00:00Z".to_string(),
        }
    }

    fn task() -> TaskDefinition {
        TaskDefinition {
            task_id: "task-ui".to_string(),
            group_id: "group-ui".to_string(),
            title: "Create workflow".to_string(),
            task_kind: TaskKind::UiFlowIncrement,
            implementation_actions: vec![
                ImplementationAction::CreateOrUpdateUiFlow,
                ImplementationAction::WireReferenceInApiOrUi,
            ],
            objective: "Create the accepted workflow".to_string(),
            depends_on: vec![],
            scope_refs: vec![],
            acceptance_refs: vec![],
            requirement_detail_refs: vec![],
            write_boundary: TaskWriteBoundary {
                forbidden_paths: vec![".loom".to_string()],
                artifact_refs: TaskArtifactRefs::default(),
            },
            verification_intents: vec![contracts::VerificationIntent {
                verification_id: "verify-flow".to_string(),
                acceptance_refs: vec![],
                requirement_detail_refs: vec![],
                behavior: "Create and read back a record".to_string(),
                preferred_evidence: vec![contracts::VerificationEvidence::AutomatedTest],
                acceptable_evidence: vec![contracts::VerificationEvidence::BrowserAutomation],
            }],
            concept_refs: vec![],
            concept_responsibilities: vec![],
            concept_verification_intents: vec![],
            frontend_experience_requirement: None,
            runtime_delivery_requirement: None,
            engineering_quality_requirement_refs: vec![],
            architecture_quality_requirement_refs: vec![],
            api_contract_requirement_refs: vec![],
            code_quality_requirement_refs: vec![],
        }
    }

    fn fixture_root(name: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "loom-browser-{name}-{}-{}",
            std::process::id(),
            state::store::now_millis()
        ))
    }
}
