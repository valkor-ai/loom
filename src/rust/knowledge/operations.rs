use std::path::PathBuf;

use sha2::{Digest, Sha256};

use crate::{
    builder::validate_candidate_paths,
    mcp_models::{
        KnowledgeAddInput, KnowledgeDiscardSummary, KnowledgeList, KnowledgeNameInput,
        KnowledgePendingInput, KnowledgeProjectInput, KnowledgeRemoveSummary,
        KnowledgeStatusSummary, KnowledgeSummary, KnowledgeUpdateInput,
    },
    models::{KnowledgeSource, PendingOperation, PendingOperationKind, PendingQueue, SkippedFile},
    paths,
    store::{
        list_pending_records, load_pending, load_pending_by_name, load_registry, local_time,
        local_time_optional, local_time_zone, now_millis, now_string, remove_dir_if_exists,
        remove_file_if_exists, remove_pending_by_name, save_pending, save_registry, KnowledgeError,
        KnowledgeResult,
    },
};

pub fn add_source(input: KnowledgeAddInput) -> KnowledgeResult<KnowledgeSummary> {
    validate_name(&input.name)?;
    if input.paths.is_empty() {
        return Err(KnowledgeError::invalid(
            "knowledgeAdd.paths must not be empty",
        ));
    }
    let warnings = validate_candidate_paths(&input.paths, true)?;
    let mut registry = load_registry()?;
    if registry
        .sources
        .iter()
        .any(|source| source.name == input.name)
    {
        return Err(KnowledgeError::invalid(format!(
            "knowledge source name already exists: {}",
            input.name
        )));
    }
    let now = now_string();
    let source_id = source_id_for_name(&input.name);
    let source = KnowledgeSource {
        source_id: source_id.clone(),
        name: input.name.clone(),
        enabled: true,
        document_paths: vec![],
        current_build_id: None,
        created_at: now.clone(),
        updated_at: now.clone(),
        last_built_at: None,
    };
    registry.sources.push(source.clone());
    registry
        .sources
        .sort_by(|left, right| left.name.cmp(&right.name));
    save_registry(&registry)?;

    let queue = PendingQueue {
        schema_version: 1,
        source_id: source_id.clone(),
        source_name: input.name,
        operations: vec![PendingOperation {
            operation_id: operation_id("kop_add"),
            kind: PendingOperationKind::AddPaths,
            paths: canonicalize_paths(&input.paths)?,
            created_at: now,
        }],
    };
    save_pending(&queue)?;
    Ok(summary(source, Some(queue), warnings))
}

pub fn update_source(input: KnowledgeUpdateInput) -> KnowledgeResult<KnowledgeSummary> {
    let selected = [
        (
            !input.add_paths.is_empty(),
            PendingOperationKind::AddPaths,
            input.add_paths,
        ),
        (
            !input.remove_paths.is_empty(),
            PendingOperationKind::RemovePaths,
            input.remove_paths,
        ),
        (
            !input.replace_paths.is_empty(),
            PendingOperationKind::ReplacePaths,
            input.replace_paths,
        ),
    ];
    let active = selected
        .into_iter()
        .filter(|(present, _, _)| *present)
        .collect::<Vec<_>>();
    if active.len() != 1 {
        return Err(KnowledgeError::invalid(
            "knowledgeUpdate must provide exactly one of addPaths, removePaths, replacePaths",
        ));
    }
    let (_, kind, paths) = active.into_iter().next().ok_or_else(|| {
        KnowledgeError::invalid(
            "knowledgeUpdate must provide exactly one of addPaths, removePaths, replacePaths",
        )
    })?;
    let (operation_paths, warnings) = if matches!(kind, PendingOperationKind::RemovePaths) {
        (normalize_paths_without_fs(&paths)?, vec![])
    } else {
        let warnings = validate_candidate_paths(&paths, true)?;
        (canonicalize_paths(&paths)?, warnings)
    };
    let mut registry = load_registry()?;
    let source = registry_source_mut(&mut registry, &input.name)?;
    source.updated_at = now_string();
    let source_snapshot = source.clone();
    save_registry(&registry)?;

    let mut queue = load_pending(&source_snapshot.source_id, &source_snapshot.name)?;
    queue.operations.push(PendingOperation {
        operation_id: operation_id("kop_update"),
        kind,
        paths: operation_paths,
        created_at: now_string(),
    });
    save_pending(&queue)?;
    Ok(summary(source_snapshot, Some(queue), warnings))
}

pub fn pending_sources(input: KnowledgePendingInput) -> KnowledgeResult<KnowledgeList> {
    let registry = load_registry()?;
    if let Some(name) = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
    {
        let source = registry
            .sources
            .iter()
            .find(|source| source.name == name)
            .cloned();
        let queue = match &source {
            Some(source) => load_pending(&source.source_id, &source.name)?,
            None => load_pending_by_name(name)?.unwrap_or_else(|| PendingQueue::empty("", name)),
        };
        return Ok(KnowledgeList {
            sources: if queue.operations.is_empty() {
                vec![]
            } else {
                vec![summary(
                    source.unwrap_or_else(|| pending_only_source(&queue)),
                    Some(queue),
                    vec![],
                )]
            },
        });
    }
    let mut sources = Vec::new();
    let mut registry_source_ids = Vec::new();
    for source in registry.sources {
        registry_source_ids.push(source.source_id.clone());
        let queue = load_pending(&source.source_id, &source.name)?;
        if !queue.operations.is_empty() {
            sources.push(summary(source, Some(queue), vec![]));
        }
    }
    for record in list_pending_records()? {
        if registry_source_ids
            .iter()
            .any(|source_id| source_id == &record.queue.source_id)
        {
            continue;
        }
        if record.queue.operations.is_empty() {
            continue;
        }
        sources.push(summary(
            pending_only_source(&record.queue),
            Some(record.queue),
            vec![],
        ));
    }
    Ok(KnowledgeList { sources })
}

pub fn discard_pending(input: KnowledgeNameInput) -> KnowledgeResult<KnowledgeDiscardSummary> {
    validate_name(&input.name)?;
    let registry = load_registry()?;
    let mut discarded = false;
    if let Some(source) = registry
        .sources
        .iter()
        .find(|source| source.name == input.name)
    {
        let pending_file = paths::pending_file(&source.source_id)?;
        discarded = pending_file.exists();
        remove_file_if_exists(&pending_file)?;
    }
    discarded |= remove_pending_by_name(&input.name)?;
    Ok(KnowledgeDiscardSummary {
        name: input.name,
        discarded,
    })
}

pub fn list_sources(_input: KnowledgeProjectInput) -> KnowledgeResult<KnowledgeList> {
    let registry = load_registry()?;
    let mut sources = Vec::new();
    let mut registry_source_ids = Vec::new();
    for source in registry.sources {
        registry_source_ids.push(source.source_id.clone());
        let queue = load_pending(&source.source_id, &source.name)?;
        let pending = if queue.operations.is_empty() {
            None
        } else {
            Some(queue)
        };
        sources.push(summary(source, pending, vec![]));
    }
    for record in list_pending_records()? {
        if registry_source_ids
            .iter()
            .any(|source_id| source_id == &record.queue.source_id)
        {
            continue;
        }
        if record.queue.operations.is_empty() {
            continue;
        }
        sources.push(summary(
            pending_only_source(&record.queue),
            Some(record.queue),
            vec![],
        ));
    }
    Ok(KnowledgeList { sources })
}

pub fn source_status(input: KnowledgeNameInput) -> KnowledgeResult<KnowledgeStatusSummary> {
    validate_name(&input.name)?;
    let registry = load_registry()?;
    let source = registry
        .sources
        .iter()
        .find(|source| source.name == input.name)
        .cloned();
    let queue = match &source {
        Some(source) => load_pending(&source.source_id, &source.name)?,
        None => load_pending_by_name(&input.name)?
            .unwrap_or_else(|| PendingQueue::empty("", &input.name)),
    };
    let pending = if queue.operations.is_empty() {
        None
    } else {
        Some(queue)
    };
    Ok(KnowledgeStatusSummary {
        name: input.name,
        created_at_local: source.as_ref().map(|source| local_time(&source.created_at)),
        updated_at_local: source.as_ref().map(|source| local_time(&source.updated_at)),
        last_built_at_local: source
            .as_ref()
            .and_then(|source| local_time_optional(&source.last_built_at)),
        source,
        pending,
        time_zone: local_time_zone(),
    })
}

pub fn remove_source(input: KnowledgeNameInput) -> KnowledgeResult<KnowledgeRemoveSummary> {
    validate_name(&input.name)?;
    let mut registry = load_registry()?;
    let source = registry
        .sources
        .iter()
        .position(|source| source.name == input.name)
        .map(|index| registry.sources.remove(index));
    if source.is_some() {
        save_registry(&registry)?;
    }
    let mut removed_pending = false;
    if let Some(source) = &source {
        let pending_file = paths::pending_file(&source.source_id)?;
        removed_pending = pending_file.exists();
        remove_file_if_exists(&pending_file)?;
        remove_dir_if_exists(&paths::source_dir(&source.source_id)?)?;
    }
    removed_pending |= remove_pending_by_name(&input.name)?;
    Ok(KnowledgeRemoveSummary {
        name: input.name,
        removed_source: source.is_some(),
        removed_pending,
    })
}

pub fn enable_source(input: KnowledgeNameInput) -> KnowledgeResult<KnowledgeSummary> {
    set_enabled(input, true)
}

pub fn disable_source(input: KnowledgeNameInput) -> KnowledgeResult<KnowledgeSummary> {
    set_enabled(input, false)
}

pub(crate) fn registry_source<'a>(
    registry: &'a crate::models::KnowledgeRegistry,
    name: &str,
) -> KnowledgeResult<&'a KnowledgeSource> {
    registry
        .sources
        .iter()
        .find(|source| source.name == name)
        .ok_or_else(|| KnowledgeError::invalid(format!("knowledge source not found: {name}")))
}

pub(crate) fn registry_source_mut<'a>(
    registry: &'a mut crate::models::KnowledgeRegistry,
    name: &str,
) -> KnowledgeResult<&'a mut KnowledgeSource> {
    registry
        .sources
        .iter_mut()
        .find(|source| source.name == name)
        .ok_or_else(|| KnowledgeError::invalid(format!("knowledge source not found: {name}")))
}

pub(crate) fn summary(
    source: KnowledgeSource,
    pending: Option<PendingQueue>,
    warnings: Vec<SkippedFile>,
) -> KnowledgeSummary {
    KnowledgeSummary {
        created_at_local: local_time(&source.created_at),
        updated_at_local: local_time(&source.updated_at),
        last_built_at_local: local_time_optional(&source.last_built_at),
        source,
        pending,
        time_zone: local_time_zone(),
        warnings,
    }
}

fn pending_only_source(queue: &PendingQueue) -> KnowledgeSource {
    let created_at = queue
        .operations
        .first()
        .map(|operation| operation.created_at.clone())
        .unwrap_or_else(now_string);
    let updated_at = queue
        .operations
        .last()
        .map(|operation| operation.created_at.clone())
        .unwrap_or_else(|| created_at.clone());
    KnowledgeSource {
        source_id: queue.source_id.clone(),
        name: queue.source_name.clone(),
        enabled: true,
        document_paths: vec![],
        current_build_id: None,
        created_at,
        updated_at,
        last_built_at: None,
    }
}

fn set_enabled(input: KnowledgeNameInput, enabled: bool) -> KnowledgeResult<KnowledgeSummary> {
    let mut registry = load_registry()?;
    let source = registry_source_mut(&mut registry, &input.name)?;
    source.enabled = enabled;
    source.updated_at = now_string();
    let source_snapshot = source.clone();
    save_registry(&registry)?;
    let queue = load_pending(&source_snapshot.source_id, &source_snapshot.name)?;
    let pending = if queue.operations.is_empty() {
        None
    } else {
        Some(queue)
    };
    Ok(summary(source_snapshot, pending, vec![]))
}

fn validate_name(name: &str) -> KnowledgeResult<()> {
    if !(2..=80).contains(&name.len()) {
        return Err(KnowledgeError::invalid(
            "knowledge name length must be between 2 and 80",
        ));
    }
    if !name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-'))
    {
        return Err(KnowledgeError::invalid(
            "knowledge name may only contain letters, numbers, dot, underscore, and dash",
        ));
    }
    Ok(())
}

fn source_id_for_name(name: &str) -> String {
    let safe = name
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let mut hasher = Sha256::new();
    hasher.update(name.as_bytes());
    let digest = format!("{:x}", hasher.finalize());
    format!("ksrc_{}_{}", safe, &digest[..8])
}

fn operation_id(prefix: &str) -> String {
    format!("{prefix}_{}", now_millis())
}

fn canonicalize_paths(paths: &[String]) -> KnowledgeResult<Vec<String>> {
    let mut result = Vec::new();
    for path in paths {
        let canonical = PathBuf::from(path)
            .expand_tilde()
            .canonicalize()
            .map_err(|error| KnowledgeError::invalid(format!("invalid path {path}: {error}")))?;
        result.push(canonical.to_string_lossy().to_string());
    }
    result.sort();
    result.dedup();
    Ok(result)
}

fn normalize_paths_without_fs(paths: &[String]) -> KnowledgeResult<Vec<String>> {
    let cwd = std::env::current_dir()?;
    let mut result = Vec::new();
    for path in paths {
        let expanded = PathBuf::from(path).expand_tilde();
        let absolute = if expanded.is_absolute() {
            expanded
        } else {
            cwd.join(expanded)
        };
        let normalized = if absolute.exists() {
            absolute.canonicalize().unwrap_or(absolute)
        } else {
            normalize_path_components(absolute)
        };
        result.push(normalized.to_string_lossy().to_string());
    }
    result.sort();
    result.dedup();
    if result.is_empty() {
        return Err(KnowledgeError::invalid(
            "knowledgeUpdate removePaths must not be empty",
        ));
    }
    Ok(result)
}

fn normalize_path_components(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    normalized
}

trait ExpandTilde {
    fn expand_tilde(self) -> PathBuf;
}

impl ExpandTilde for PathBuf {
    fn expand_tilde(self) -> PathBuf {
        let Some(raw) = self.to_str() else {
            return self;
        };
        if raw == "~" || raw.starts_with("~/") {
            if let Ok(home) = std::env::var("HOME") {
                return PathBuf::from(home).join(raw.trim_start_matches("~/"));
            }
        }
        self
    }
}
