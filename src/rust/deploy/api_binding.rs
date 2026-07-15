use std::{
    fs,
    path::{Path, PathBuf},
};

use contracts::{
    DeploymentApiContract, DeploymentSourceService, FrontendApiBinding, SourceServiceRole,
};

const KNOWN_FRONTEND_ENV_KEYS: &[&str] = &[
    "VITE_API_BASE_URL",
    "FRONTEND_API_BASE_URL",
    "REACT_APP_API_BASE_URL",
    "NEXT_PUBLIC_API_BASE_URL",
];

pub(crate) fn derive_frontend_api_binding(
    project_root: &Path,
    service: Option<&DeploymentSourceService>,
    api_contract: Option<&DeploymentApiContract>,
    require_contract: bool,
) -> FrontendApiBinding {
    let Some(service) = service else {
        return not_applicable("No frontend service is present in the deploy source model.");
    };
    if !matches!(
        service.role,
        SourceServiceRole::Frontend | SourceServiceRole::App
    ) {
        return not_applicable("The public service does not own a browser surface.");
    }
    let Some(api_contract) = api_contract else {
        if require_contract {
            return unresolved(
                vec![],
                vec![],
                "Deployment has an API surface but no accepted structured API contract is available for frontend binding analysis.",
            );
        }
        return not_applicable(
            "No accepted structured API contract is available for frontend binding analysis.",
        );
    };
    if api_contract.interfaces.is_empty() {
        return not_applicable("The accepted API contract has no HTTP interfaces.");
    }

    let Some(source_root) = frontend_source_root(project_root, service) else {
        return not_applicable("The service has no detectable frontend package root.");
    };
    let files = frontend_source_files(&source_root);
    if files.is_empty() {
        return not_applicable("No frontend source file was available for binding analysis.");
    }

    let contract_paths = api_contract
        .interfaces
        .iter()
        .map(|interface| interface.path.as_str())
        .collect::<Vec<_>>();
    let mut direct_path_files = Vec::new();
    let mut env_candidates = Vec::new();
    let mut used_env_keys = Vec::new();
    let mut request_evidence_files = Vec::new();
    for (path, text) in &files {
        let source_paths = path_literals_in_source(text);
        if source_contains_request_construction(text) {
            request_evidence_files.push(relative_path(project_root, path));
        }
        let direct_paths = contract_paths
            .iter()
            .filter(|contract_path| {
                text.contains(**contract_path)
                    || source_paths
                        .iter()
                        .any(|source_path| path_matches_suffix(contract_path, source_path))
            })
            .map(|path| (*path).to_string())
            .collect::<Vec<_>>();
        if !direct_paths.is_empty() {
            direct_path_files.push(relative_path(project_root, path));
        }
        for key in frontend_env_keys(&text) {
            used_env_keys.push(key.clone());
            let mut suffixes = path_literals_after_key(text, &key);
            for alias in env_aliases(text, &key) {
                suffixes.extend(path_suffixes_after_identifier(text, &alias));
            }
            suffixes.extend(path_literals_in_source(text));
            suffixes.sort();
            suffixes.dedup();
            for suffix in suffixes {
                if let Some(interface_path) = contract_paths
                    .iter()
                    .find(|contract_path| path_matches_suffix(contract_path, &suffix))
                {
                    let mode = if suffix == **interface_path {
                        "full_public_path"
                    } else {
                        "relative_to_public_base"
                    };
                    let injected_value = if mode == "full_public_path" {
                        Some(String::new())
                    } else {
                        Some(public_prefix(interface_path, &suffix))
                    };
                    env_candidates.push((
                        key.clone(),
                        injected_value,
                        mode.to_string(),
                        (*interface_path).to_string(),
                        relative_path(project_root, path),
                    ));
                }
            }
        }
    }
    used_env_keys.sort();
    used_env_keys.dedup();

    if !env_candidates.is_empty() {
        env_candidates.sort();
        env_candidates.dedup();
        let first = &env_candidates[0];
        if env_candidates
            .iter()
            .any(|candidate| candidate.0 != first.0 || candidate.1 != first.1)
        {
            return unresolved(
                used_env_keys,
                direct_path_files,
                "Frontend API environment bindings resolve to conflicting public paths.",
            );
        }
        return FrontendApiBinding {
            status: "resolved".to_string(),
            mode: first.2.clone(),
            environment_key: Some(first.0.clone()),
            injected_value: first.1.clone(),
            public_base_path: api_contract.public_base_path.clone(),
            effective_paths: contract_paths.iter().map(|path| (*path).to_string()).collect(),
            evidence_files: merge_paths(&direct_path_files, &[first.4.clone()]),
            reason: "Frontend request construction matches the accepted API interface paths. Loom owns the single required build-time environment binding.".to_string(),
        };
    }

    if !used_env_keys.is_empty() && !request_evidence_files.is_empty() {
        return unresolved(
            used_env_keys,
            merge_paths(&direct_path_files, &request_evidence_files),
            "Frontend uses an API base environment key, but Loom could not match its request paths to the accepted API contract.",
        );
    }
    if direct_path_files.is_empty() {
        if request_evidence_files.is_empty() {
            return not_applicable(
                "Frontend source has no detectable HTTP request construction for the accepted API contract.",
            );
        }
        return unresolved(
            used_env_keys,
            request_evidence_files,
            "Frontend source does not expose an analyzable request path for the accepted API contract.",
        );
    }
    FrontendApiBinding {
        status: "resolved".to_string(),
        mode: "full_public_path".to_string(),
        environment_key: None,
        injected_value: None,
        public_base_path: api_contract.public_base_path.clone(),
        effective_paths: contract_paths.iter().map(|path| (*path).to_string()).collect(),
        evidence_files: direct_path_files,
        reason: "Frontend source uses the accepted API interface paths directly; no build-time API base injection is required.".to_string(),
    }
}

fn source_contains_request_construction(text: &str) -> bool {
    [
        "fetch(",
        "axios",
        "XMLHttpRequest",
        "httpClient",
        "apiClient",
        "ky(",
    ]
    .iter()
    .any(|marker| text.contains(marker))
}

fn frontend_source_root(project_root: &Path, service: &DeploymentSourceService) -> Option<PathBuf> {
    if service.role == SourceServiceRole::Frontend {
        return Some(if service.root == "." {
            project_root.to_path_buf()
        } else {
            project_root.join(&service.root)
        });
    }
    service
        .workspace_package_json_paths
        .iter()
        .filter_map(|path| {
            path.strip_suffix("/package.json")
                .or_else(|| (path == "package.json").then_some(""))
        })
        .map(|root| {
            if root.is_empty() {
                project_root.to_path_buf()
            } else {
                project_root.join(root)
            }
        })
        .find(|root| root.is_dir())
}

fn frontend_env_keys(text: &str) -> Vec<String> {
    let mut keys = KNOWN_FRONTEND_ENV_KEYS
        .iter()
        .filter(|key| text.contains(**key))
        .map(|key| (*key).to_string())
        .collect::<Vec<_>>();
    for marker in ["import.meta.env.", "process.env."] {
        let mut offset = 0;
        while let Some(found) = text[offset..].find(marker) {
            let start = offset + found + marker.len();
            let key = text[start..]
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
                .collect::<String>();
            if is_api_environment_key(&key) {
                keys.push(key);
            }
            offset = start;
        }
    }
    for marker in ["import.meta.env[", "process.env["] {
        let mut offset = 0;
        while let Some(found) = text[offset..].find(marker) {
            let start = offset + found + marker.len();
            let remainder = &text[start..];
            let Some(quote) = remainder.chars().next() else {
                break;
            };
            if !matches!(quote, '\'' | '"' | '`') {
                offset = start;
                continue;
            }
            let value_start = quote.len_utf8();
            if let Some(close) = remainder[value_start..].find(quote) {
                let key = &remainder[value_start..value_start + close];
                if is_api_environment_key(key) {
                    keys.push(key.to_string());
                }
                offset = start + value_start + close + 1;
            } else {
                break;
            }
        }
    }
    keys.sort();
    keys.dedup();
    keys
}

fn is_api_environment_key(key: &str) -> bool {
    let normalized = key.to_ascii_uppercase();
    normalized.contains("API")
        || normalized.contains("ENDPOINT")
        || normalized.contains("ORIGIN")
        || normalized.contains("BASE_URL")
        || normalized.contains("SERVER_URL")
        || normalized.contains("BACKEND_URL")
}

fn not_applicable(reason: &str) -> FrontendApiBinding {
    FrontendApiBinding {
        status: "not_applicable".to_string(),
        mode: "unknown".to_string(),
        environment_key: None,
        injected_value: None,
        public_base_path: None,
        effective_paths: vec![],
        evidence_files: vec![],
        reason: reason.to_string(),
    }
}

fn unresolved(
    environment_keys: Vec<String>,
    evidence_files: Vec<String>,
    reason: &str,
) -> FrontendApiBinding {
    FrontendApiBinding {
        status: "unresolved".to_string(),
        mode: "unknown".to_string(),
        environment_key: environment_keys.first().cloned(),
        injected_value: None,
        public_base_path: None,
        effective_paths: vec![],
        evidence_files,
        reason: reason.to_string(),
    }
}

fn frontend_source_files(root: &Path) -> Vec<(PathBuf, String)> {
    let mut files = Vec::new();
    collect_source_files(root, &mut files, 0);
    files
}

fn collect_source_files(root: &Path, files: &mut Vec<(PathBuf, String)>, depth: usize) {
    if depth > 6 || files.len() >= 128 {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if path.is_dir() {
            if matches!(name, "node_modules" | "dist" | "build" | ".git" | ".loom") {
                continue;
            }
            collect_source_files(&path, files, depth + 1);
        } else if matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("js" | "jsx" | "ts" | "tsx" | "mjs" | "cjs" | "vue" | "svelte" | "astro")
        ) {
            if let Ok(text) = fs::read_to_string(&path) {
                files.push((path, text));
            }
        }
        if files.len() >= 128 {
            return;
        }
    }
}

fn path_literals_after_key(text: &str, key: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut offset = 0;
    while let Some(found) = text[offset..].find(key) {
        let start = offset + found + key.len();
        let end = (start + 240).min(text.len());
        let window = &text[start..end];
        let bytes = window.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            if !matches!(bytes[index], b'\'' | b'"' | b'`') {
                index += 1;
                continue;
            }
            let quote = bytes[index];
            let value_start = index + 1;
            let Some(close) = window[value_start..].find(quote as char) else {
                break;
            };
            let value = &window[value_start..value_start + close];
            if value.starts_with('/') {
                result.push(value.to_string());
            }
            index = value_start + close + 1;
        }
        offset = start;
    }
    result.sort();
    result.dedup();
    result
}

fn env_aliases(text: &str, key: &str) -> Vec<String> {
    let mut aliases = Vec::new();
    let mut offset = 0;
    while let Some(found) = text[offset..].find(key) {
        let key_start = offset + found;
        let statement_start = text[..key_start]
            .rfind([';', '\n'])
            .map(|index| index + 1)
            .unwrap_or(0);
        let statement = &text[statement_start..key_start];
        let Some(equal) = statement.rfind('=') else {
            offset = key_start + key.len();
            continue;
        };
        let left = statement[..equal].trim();
        let alias = left
            .rsplit_once(' ')
            .map(|(_, value)| value.trim())
            .unwrap_or(left)
            .trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_');
        if !alias.is_empty()
            && alias
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
        {
            aliases.push(alias.to_string());
        }
        offset = key_start + key.len();
    }
    aliases.sort();
    aliases.dedup();
    aliases
}

fn path_suffixes_after_identifier(text: &str, identifier: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut offset = 0;
    while let Some(found) = text[offset..].find(identifier) {
        let start = offset + found + identifier.len();
        let end = (start + 180).min(text.len());
        let window = &text[start..end];
        let Some(close_expression) = window.find('}') else {
            offset = start;
            continue;
        };
        let remainder = &window[close_expression + 1..];
        let end_path = remainder
            .find(['`', '\'', '"', ' ', ')', ','])
            .unwrap_or(remainder.len());
        let suffix = remainder[..end_path].trim();
        if suffix.starts_with('/') {
            result.push(suffix.to_string());
        }
        offset = start;
    }
    result.sort();
    result.dedup();
    result
}

fn path_literals_in_source(text: &str) -> Vec<String> {
    let mut result = Vec::new();
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if !matches!(bytes[index], b'\'' | b'"' | b'`') {
            index += 1;
            continue;
        }
        let quote = bytes[index] as char;
        let value_start = index + 1;
        let Some(close) = text[value_start..].find(quote) else {
            break;
        };
        let value = &text[value_start..value_start + close];
        if value.starts_with('/') {
            result.push(value.to_string());
        } else if let Some(close_expression) = value.rfind('}') {
            let suffix = value[close_expression + 1..].trim();
            if suffix.starts_with('/') {
                result.push(suffix.to_string());
            }
        }
        index = value_start + close + 1;
    }
    result.sort();
    result.dedup();
    result
}

fn path_matches_suffix(contract_path: &str, suffix: &str) -> bool {
    let contract_parts = path_segments(contract_path);
    let suffix_parts = path_segments(suffix);
    if contract_parts.len() == suffix_parts.len() {
        return contract_parts
            .iter()
            .zip(&suffix_parts)
            .all(|(contract, actual)| route_segment_matches(contract, actual));
    }
    suffix_parts.len() < contract_parts.len()
        && contract_parts[contract_parts.len() - suffix_parts.len()..]
            .iter()
            .zip(&suffix_parts)
            .all(|(contract, actual)| route_segment_matches(contract, actual))
}

fn public_prefix(contract_path: &str, suffix: &str) -> String {
    let contract_parts = path_segments(contract_path);
    let suffix_parts = path_segments(suffix);
    let prefix_len = contract_parts.len().saturating_sub(suffix_parts.len());
    if prefix_len == 0 {
        return String::new();
    }
    format!("/{}", contract_parts[..prefix_len].join("/"))
}

fn path_segments(path: &str) -> Vec<&str> {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

fn route_segment_matches(contract: &str, actual: &str) -> bool {
    (contract.starts_with('{') && contract.ends_with('}'))
        || (actual.starts_with("${") && actual.ends_with('}'))
        || contract == actual
}

fn relative_path(project_root: &Path, path: &Path) -> String {
    path.strip_prefix(project_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn merge_paths(left: &[String], right: &[String]) -> Vec<String> {
    let mut values = left.to_vec();
    values.extend(right.iter().cloned());
    values.sort();
    values.dedup();
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    use contracts::{PackageManager, RuntimeKind};

    #[test]
    fn detects_template_suffix_after_env_alias() {
        let source = r#"const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || "";
fetch(`${API_BASE_URL}/accounts`);
"#;
        assert_eq!(
            env_aliases(source, "VITE_API_BASE_URL"),
            vec!["API_BASE_URL".to_string()]
        );
        assert_eq!(
            path_suffixes_after_identifier(source, "API_BASE_URL"),
            vec!["/accounts".to_string()]
        );
        assert_eq!(
            path_literals_in_source(source),
            vec!["/accounts".to_string()]
        );
    }

    #[test]
    fn detects_framework_specific_and_custom_api_environment_keys() {
        let source = r#"
const first = import.meta.env.VITE_API_URL;
const second = process.env.PUBLIC_BACKEND_ORIGIN;
const third = import.meta.env["CUSTOM_ENDPOINT"];
"#;
        assert_eq!(
            frontend_env_keys(source),
            vec![
                "CUSTOM_ENDPOINT".to_string(),
                "PUBLIC_BACKEND_ORIGIN".to_string(),
                "VITE_API_URL".to_string()
            ]
        );
    }

    #[test]
    fn matches_dynamic_interface_paths_without_duplicating_the_public_prefix() {
        assert!(path_matches_suffix(
            "/api/tickets/{ticketId}",
            "/tickets/${ticketId}"
        ));
        assert_eq!(
            public_prefix("/api/tickets/{ticketId}", "/tickets/${ticketId}"),
            "/api"
        );
    }

    #[test]
    fn derives_relative_binding_for_nested_frontend_root() {
        let root = std::env::temp_dir().join(format!(
            "loom-api-binding-unit-{}",
            state::store::now_millis()
        ));
        let source = root.join("apps/frontend/src/lib/api.js");
        std::fs::create_dir_all(source.parent().expect("source parent")).expect("source dir");
        std::fs::write(
            &source,
            "const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || \"\"; fetch(`${API_BASE_URL}/accounts`);",
        )
        .expect("source");
        let service = DeploymentSourceService {
            service_id: "frontend".to_string(),
            role: SourceServiceRole::Frontend,
            root: "apps/frontend".to_string(),
            working_directory: None,
            workspace_package_json_paths: vec![],
            manifest_refs: vec![],
            lockfile_refs: vec![],
            artifact_refs: vec![],
            runtime_kind: RuntimeKind::Node,
            package_manager: Some(PackageManager::Npm),
            has_lockfile: false,
            framework: None,
            runtime_version: None,
            runtime_version_source: None,
            build_command: None,
            start_command: None,
            output_directory: Some("apps/frontend/dist".to_string()),
            port: 80,
            healthcheck_path: None,
        };
        let contract = DeploymentApiContract {
            source_ref: "aac#/interfaces".to_string(),
            status: "resolved".to_string(),
            interfaces: vec![contracts::DeploymentApiInterface {
                interface_id: "api.accounts.list".to_string(),
                method: "GET".to_string(),
                path: "/api/accounts".to_string(),
            }],
            public_base_path: Some("/api".to_string()),
            preserve_path: true,
            browser_mode: "same_origin".to_string(),
            browser_base_url: None,
        };
        let binding = derive_frontend_api_binding(&root, Some(&service), Some(&contract), false);
        assert_eq!(binding.status, "resolved", "{binding:?}");
        assert_eq!(binding.mode, "relative_to_public_base", "{binding:?}");
        assert_eq!(binding.injected_value.as_deref(), Some("/api"));
        let _ = std::fs::remove_dir_all(root);
    }
}
