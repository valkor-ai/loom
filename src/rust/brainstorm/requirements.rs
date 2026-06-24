use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use algorithm_client::AlgorithmClient;
use contracts::{
    RequirementContext, RequirementSource, RequirementSourceItem, RequirementSourceItemKind,
    RequirementSourceOrigin, RequirementSourceType, UserFacingLanguageConstraint, UserFacingLocale,
};
use quick_xml::{events::Event, Reader};
use serde_json::json;
use sha2::{Digest, Sha256};
use state::{
    paths::to_project_relative,
    store::{write_json_atomic, write_text_atomic, StateError, StateResult},
};
use zip::ZipArchive;

use crate::paths::{
    requirement_context_file, requirement_input_file, requirement_keyword_hints_file,
    requirement_normalized_text_file,
};

#[derive(Debug, Clone)]
pub struct RequirementArtifacts {
    pub context_ref: String,
    pub normalized_text_ref: Option<String>,
    pub keyword_hints_ref: Option<String>,
    pub user_facing_language: UserFacingLanguageConstraint,
    pub formal_sources: Vec<RequirementSource>,
}

pub fn formal_sources_from_items(items: &[RequirementSourceItem]) -> Vec<RequirementSource> {
    items
        .iter()
        .cloned()
        .map(requirement_source_from_item)
        .collect()
}

pub fn build_requirement_artifacts(
    project_root: &Path,
    delivery_id: &str,
    request_text: &str,
    requirement_files: &[String],
) -> StateResult<RequirementArtifacts> {
    let mut source_items = Vec::new();
    let mut normalized_parts = Vec::new();

    let primary_text = request_text.trim();
    if !primary_text.is_empty() {
        let item_id = "req-001".to_string();
        let text_ref = to_project_relative(
            project_root,
            &requirement_input_file(project_root, delivery_id, &item_id),
        )?;
        write_text_atomic(&project_root.join(&text_ref), &format!("{primary_text}\n"))?;
        let text_digest = sha256_hex(primary_text.as_bytes());
        source_items.push(RequirementSourceItem {
            item_id: item_id.clone(),
            kind: RequirementSourceItemKind::Text,
            origin: RequirementSourceOrigin::UserMessage,
            title: Some("request_text".to_string()),
            path: None,
            text_ref: Some(text_ref.clone()),
            extracted_text_ref: None,
            extraction_status: Some("completed".to_string()),
            extraction_reason: None,
            digest: Some(text_digest.clone()),
            text_digest: Some(text_digest),
            character_count: Some(primary_text.chars().count() as u64),
        });
        normalized_parts.push(NormalizedPart {
            item_id,
            title: Some("request_text".to_string()),
            text: primary_text.to_string(),
        });
    }

    for (index, file) in requirement_files.iter().enumerate() {
        let item_id = format!("req-{:03}", index + 2);
        let absolute = PathBuf::from(file);
        let text = parse_requirement_file(&absolute)?;
        if text.trim().is_empty() {
            continue;
        }
        let text_ref = to_project_relative(
            project_root,
            &requirement_input_file(project_root, delivery_id, &item_id),
        )?;
        write_text_atomic(&project_root.join(&text_ref), &format!("{}\n", text.trim()))?;
        let raw = std::fs::read(&absolute)?;
        let digest = sha256_hex(&raw);
        let text_digest = sha256_hex(text.trim().as_bytes());
        let title = absolute
            .file_stem()
            .map(|value| value.to_string_lossy().into_owned());
        source_items.push(RequirementSourceItem {
            item_id: item_id.clone(),
            kind: RequirementSourceItemKind::File,
            origin: RequirementSourceOrigin::RequestFile,
            title: title.clone(),
            path: Some(absolute.to_string_lossy().into_owned()),
            text_ref: Some(text_ref.clone()),
            extracted_text_ref: Some(text_ref.clone()),
            extraction_status: Some("completed".to_string()),
            extraction_reason: None,
            digest: Some(digest),
            text_digest: Some(text_digest),
            character_count: Some(text.trim().chars().count() as u64),
        });
        normalized_parts.push(NormalizedPart {
            item_id,
            title,
            text,
        });
    }

    let normalized_text = normalized_parts
        .iter()
        .map(|part| {
            let title = part
                .title
                .as_deref()
                .map(|value| format!(" {}", value))
                .unwrap_or_default();
            format!("## {}{}\n\n{}", part.item_id, title, part.text.trim())
        })
        .collect::<Vec<_>>()
        .join("\n\n")
        .trim()
        .to_string();
    let user_facing_language = infer_user_facing_language(&normalized_text);
    let normalized_text_ref = if normalized_text.is_empty() {
        None
    } else {
        let file = requirement_normalized_text_file(project_root, delivery_id);
        write_text_atomic(&file, &format!("{normalized_text}\n"))?;
        Some(to_project_relative(project_root, &file)?)
    };

    let keyword_hints = build_keyword_hints(&normalized_parts, &user_facing_language)?;
    let keyword_hints_file = requirement_keyword_hints_file(project_root, delivery_id);
    write_json_atomic(&keyword_hints_file, &keyword_hints)?;
    let keyword_hints_ref = Some(to_project_relative(project_root, &keyword_hints_file)?);

    let context = RequirementContext {
        schema_version: "1.0".to_string(),
        delivery_id: delivery_id.to_string(),
        created_at: state::store::now_string(),
        source_items: source_items.clone(),
        normalized_text_ref: normalized_text_ref.clone(),
        normalized_text_status: if normalized_text_ref.is_some() {
            "completed".to_string()
        } else {
            "empty".to_string()
        },
        normalized_text_reason: if normalized_text_ref.is_some() {
            None
        } else {
            Some("No requirement text was available after extraction.".to_string())
        },
        keyword_hints_ref: keyword_hints_ref.clone(),
        keyword_hints_status: keyword_hints
            .get("status")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("completed")
            .to_string(),
        keyword_hints_reason: if keyword_hints["status"] == "empty" {
            Some("No stable keyword hints were extracted.".to_string())
        } else {
            None
        },
    };
    let context_file = requirement_context_file(project_root, delivery_id);
    write_json_atomic(&context_file, &context)?;

    Ok(RequirementArtifacts {
        context_ref: to_project_relative(project_root, &context_file)?,
        normalized_text_ref,
        keyword_hints_ref,
        user_facing_language,
        formal_sources: formal_sources_from_items(&source_items),
    })
}

fn infer_user_facing_language(text: &str) -> UserFacingLanguageConstraint {
    let locale = infer_locale(text);
    UserFacingLanguageConstraint {
        default_locale: locale,
        source: if matches!(locale, UserFacingLocale::Und) {
            contracts::UserFacingLanguageSource::Fallback
        } else {
            contracts::UserFacingLanguageSource::RequirementPrimaryLanguage
        },
        applies_to: vec![
            "navigation labels".to_string(),
            "page titles and headings".to_string(),
            "form labels and placeholders".to_string(),
            "button and action labels".to_string(),
            "table/list/search labels".to_string(),
            "success, validation, error, and business-blocking messages".to_string(),
            "visible status/result text".to_string(),
        ],
        does_not_apply_to: vec![
            "source code identifiers".to_string(),
            "API paths and payload field names".to_string(),
            "database tables, columns, and enum values".to_string(),
            "package names and framework conventions".to_string(),
            "internal artifact names or technical ids".to_string(),
        ],
        rule: user_facing_language_rule(locale),
    }
}

fn infer_locale(text: &str) -> UserFacingLocale {
    let han = text
        .chars()
        .filter(|ch| matches!(*ch, '\u{3400}'..='\u{9fff}'))
        .count();
    let latin = text.chars().filter(|ch| ch.is_ascii_alphabetic()).count();
    if han > 0 && (han >= 20 || han.saturating_mul(3) >= latin) {
        return UserFacingLocale::ZhCn;
    }
    if latin > 0 && han == 0 {
        return UserFacingLocale::En;
    }
    if han > 0 {
        return UserFacingLocale::ZhCn;
    }
    UserFacingLocale::Und
}

fn user_facing_language_rule(locale: UserFacingLocale) -> String {
    match locale {
        UserFacingLocale::ZhCn => "User-visible UI copy must default to Chinese. Apply this to labels, navigation, form text, buttons, search/list labels, visible status text, success messages, validation errors, and business-blocking feedback. Do not translate code identifiers, API paths, database fields, enum values, package names, framework names, or internal artifact ids.".to_string(),
        UserFacingLocale::En => "User-visible UI copy must default to English. Apply this to labels, navigation, form text, buttons, search/list labels, visible status text, success messages, validation errors, and business-blocking feedback. Do not translate code identifiers, API paths, database fields, enum values, package names, framework names, or internal artifact ids.".to_string(),
        UserFacingLocale::Und => "No explicit user-facing language was inferred. Keep user-visible copy aligned with the confirmed requirement wording or product baseline; do not translate technical identifiers.".to_string(),
    }
}

fn requirement_source_from_item(item: RequirementSourceItem) -> RequirementSource {
    let source_type = source_type_from_item(&item);
    RequirementSource {
        source_id: item.item_id,
        r#type: source_type,
        path: item.path,
        title: item.title,
        text_digest: item.text_digest,
        digest: item.digest,
        extracted: Some(true),
    }
}

fn source_type_from_item(item: &RequirementSourceItem) -> RequirementSourceType {
    let Some(path) = item.path.as_deref() else {
        return RequirementSourceType::UserText;
    };
    match extension(path).as_deref() {
        Some("pdf") => RequirementSourceType::Pdf,
        Some("doc" | "docx") => RequirementSourceType::Word,
        Some("md" | "markdown") => RequirementSourceType::Markdown,
        Some("txt" | "json" | "yaml" | "yml") => RequirementSourceType::Text,
        Some("csv" | "tsv" | "xlsx" | "xls") => RequirementSourceType::Spreadsheet,
        Some("ts" | "tsx" | "js" | "jsx" | "java" | "py" | "go" | "rs") => {
            RequirementSourceType::Code
        }
        _ => RequirementSourceType::Unknown,
    }
}

fn parse_requirement_file(path: &Path) -> StateResult<String> {
    let extension = extension_for_path(path).unwrap_or_default();
    match extension.as_str() {
        "md" | "markdown" | "txt" | "json" | "yaml" | "yml" => Ok(std::fs::read_to_string(path)?),
        "pdf" => pdf_extract::extract_text(path).map_err(|error| {
            StateError::InvalidArgument(format!(
                "failed to extract requirement PDF {}: {error}",
                path.display()
            ))
        }),
        "docx" => extract_docx_text(path),
        other => Err(StateError::InvalidArgument(format!(
            "unsupported requirementFile extension: {}",
            if other.is_empty() {
                path.display().to_string()
            } else {
                other.to_string()
            }
        ))),
    }
}

fn build_keyword_hints(
    parts: &[NormalizedPart],
    language: &UserFacingLanguageConstraint,
) -> StateResult<serde_json::Value> {
    if parts.is_empty() {
        return Ok(empty_keyword_hints(language.default_locale));
    }
    let client = algorithm_client()?;
    let documents = parts
        .iter()
        .map(|part| json!({ "id": part.item_id, "text": part.text }))
        .collect::<Vec<_>>();
    let tfidf = client
        .call(&json!({
            "operation": "tfidf",
            "documents": documents,
            "limit": 32
        }))
        .map_err(|error| StateError::InvalidArgument(error.to_string()))?;
    let global_keywords = tfidf["keywords"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|item| {
            let keyword = item.get("term")?.as_str()?.trim().to_string();
            if !keyword_allowed(&keyword) {
                return None;
            }
            Some(json!({
                "keyword": keyword,
                "occurrences": approximate_occurrences(parts, &keyword),
                "sourceItemIds": item.get("documentIds").cloned().unwrap_or_else(|| json!([])),
            }))
        })
        .take(16)
        .collect::<Vec<_>>();
    if global_keywords.is_empty() {
        return Ok(empty_keyword_hints(language.default_locale));
    }

    let mut section_keywords = Vec::new();
    for part in parts {
        let token_response = client
            .call(&json!({"operation": "tokenize", "text": part.text}))
            .map_err(|error| StateError::InvalidArgument(error.to_string()))?;
        let mut counts = BTreeMap::<String, u64>::new();
        for token in token_response["tokens"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|item| item.as_str())
        {
            if !keyword_allowed(token) {
                continue;
            }
            *counts.entry(token.to_string()).or_default() += 1;
        }
        let keywords = counts
            .into_iter()
            .filter(|(_, count)| *count > 0)
            .collect::<Vec<_>>();
        if keywords.is_empty() {
            continue;
        }
        let top = keywords
            .into_iter()
            .map(|(keyword, count)| (count, keyword))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .rev()
            .take(6)
            .map(|(_, keyword)| json!({ "keyword": keyword }))
            .collect::<Vec<_>>();
        section_keywords.push(json!({
            "sectionId": part.item_id,
            "sourceItemId": part.item_id,
            "title": part.title,
            "keywords": top,
        }));
    }

    let top_keywords = global_keywords.iter().take(12).cloned().collect::<Vec<_>>();
    Ok(json!({
        "schemaVersion": "1.0",
        "usage": "advisory_only",
        "status": "completed",
        "languageHints": language_hints(language.default_locale),
        "globalKeywords": global_keywords,
        "sectionKeywords": section_keywords,
        "rules": {
            "mustNotTreatAsScope": true,
            "mustNotTreatAsAcceptance": true,
            "mustNotTreatAsConfirmedConcept": true,
            "ignoreWhenIrrelevant": true,
        },
        "compact": {
            "usage": "advisory_only",
            "status": "completed",
            "languageHints": language_hints(language.default_locale),
            "topKeywords": top_keywords,
            "sectionKeywords": section_keywords,
            "rules": {
                "advisoryOnly": true,
                "mustNotTreatAsScope": true,
                "mustNotTreatAsAcceptance": true,
                "ignoreWhenIrrelevant": true,
            }
        }
    }))
}

fn empty_keyword_hints(locale: UserFacingLocale) -> serde_json::Value {
    json!({
        "schemaVersion": "1.0",
        "usage": "advisory_only",
        "status": "empty",
        "languageHints": language_hints(locale),
        "globalKeywords": [],
        "sectionKeywords": [],
        "rules": {
            "mustNotTreatAsScope": true,
            "mustNotTreatAsAcceptance": true,
            "mustNotTreatAsConfirmedConcept": true,
            "ignoreWhenIrrelevant": true,
        },
        "compact": {
            "usage": "advisory_only",
            "status": "empty",
            "languageHints": language_hints(locale),
            "topKeywords": [],
            "sectionKeywords": [],
            "rules": {
                "advisoryOnly": true,
                "mustNotTreatAsScope": true,
                "mustNotTreatAsAcceptance": true,
                "ignoreWhenIrrelevant": true,
            }
        }
    })
}

fn language_hints(locale: UserFacingLocale) -> Vec<String> {
    match locale {
        UserFacingLocale::ZhCn => vec!["zh-CN".to_string(), "zh-Hans".to_string()],
        UserFacingLocale::En => vec!["en".to_string()],
        UserFacingLocale::Und => vec!["und".to_string()],
    }
}

fn approximate_occurrences(parts: &[NormalizedPart], keyword: &str) -> u64 {
    parts
        .iter()
        .map(|part| part.text.matches(keyword).count() as u64)
        .sum::<u64>()
        .max(1)
}

fn keyword_allowed(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() < 2 {
        return false;
    }
    if trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        return false;
    }
    !matches!(
        trimmed,
        "the" | "and" | "for" | "with" | "this" | "that" | "into" | "from"
    )
}

fn extension(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .map(|value| value.to_string_lossy().to_ascii_lowercase())
}

fn extension_for_path(path: &Path) -> Option<String> {
    path.extension()
        .map(|value| value.to_string_lossy().to_ascii_lowercase())
}

fn extract_docx_text(path: &Path) -> StateResult<String> {
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)
        .map_err(|error| StateError::InvalidArgument(format!("invalid docx archive: {error}")))?;
    let mut document = archive.by_name("word/document.xml").map_err(|error| {
        StateError::InvalidArgument(format!("missing word/document.xml in docx: {error}"))
    })?;
    let mut xml = String::new();
    document.read_to_string(&mut xml)?;
    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(true);
    let mut output = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Text(text)) => {
                output.push_str(&text.decode().unwrap_or_default());
                output.push(' ');
            }
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(StateError::InvalidArgument(format!(
                    "failed to parse docx text: {error}"
                )))
            }
            _ => {}
        }
    }
    Ok(output.trim().to_string())
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

fn algorithm_client() -> StateResult<AlgorithmClient> {
    let python = std::env::var("PYTHON").unwrap_or_else(|_| "python3".to_string());
    let candidates = [
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../python/algorithms/worker.py"),
        PathBuf::from("src/python/algorithms/worker.py"),
        PathBuf::from("../python/algorithms/worker.py"),
        PathBuf::from("../../src/python/algorithms/worker.py"),
    ];
    let worker = candidates
        .into_iter()
        .find(|path| path.exists())
        .ok_or_else(|| {
            StateError::InvalidArgument("Python algorithm worker not found".to_string())
        })?;
    Ok(AlgorithmClient::new(python, worker))
}

#[derive(Debug, Clone)]
struct NormalizedPart {
    item_id: String,
    title: Option<String>,
    text: String,
}
