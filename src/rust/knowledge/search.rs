use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
    fs,
};

use algorithm_client::AlgorithmClient;

use crate::{
    mcp_models::{
        KnowledgeBrainstormContextInput, KnowledgeBrainstormContextResult, KnowledgeChunkCard,
        KnowledgeContextMatchedSource, KnowledgeMatchedLabel, KnowledgeMatchedSource,
        KnowledgeReadPlan, KnowledgeReadPlanChunk, KnowledgeSearchInput, KnowledgeSearchResult,
    },
    models::{BlockAffinity, ChunksFile, KnowledgeChunk, LexicalIndex},
    paths,
    store::{load_registry, read_json, KnowledgeError, KnowledgeResult},
};

const DEFAULT_SEARCH_LIMIT: usize = 8;
const DEFAULT_CONTEXT_SOURCE_LIMIT: usize = 2;
const DEFAULT_CONTEXT_CHUNK_LIMIT_PER_SOURCE: usize = 5;
const MAX_CONTEXT_CHUNKS_PER_BLOCK: usize = 5;
const MIN_CONTEXT_FOCUS_COVERAGE: f64 = 0.01;

const PHASE_SCOPE_RETRIEVAL_INTENT: &str =
    "phase scope boundary include exclude defer dependency ordering next phase 阶段范围 边界 纳入 排除 延后 递延 依赖 顺序 下一阶段";
const CONCEPT_GROUNDING_RETRIEVAL_INTENT: &str =
    "business object operation field state rule invariant precondition validation blocking outcome feedback 业务对象 操作 字段 状态 规则 不变量 前置条件 校验 阻断 成功结果 反馈";
const FRONTEND_EXPERIENCE_RETRIEVAL_INTENT: &str =
    "page operation path workspace entry target discovery query filter pagination selection list detail action entry form input success feedback failure feedback business blocking loading empty state refresh readback 页面办理路径 页面操作路径 工作台 入口 目标定位 查询 筛选 分页 选择 列表 详情 操作入口 表单 输入 成功反馈 失败提示 业务阻断 加载中 空状态 刷新 回读";

#[derive(Debug, Clone, PartialEq, Eq)]
struct BrainstormKnowledgeStepRequirement {
    repeat_mode: Option<String>,
}

pub fn search_knowledge(input: KnowledgeSearchInput) -> KnowledgeResult<KnowledgeSearchResult> {
    let cards = search_cards(
        &input.natural_language_query,
        &input.semantic_focus,
        &input.source_names,
        input.block.as_deref(),
        input.limit.unwrap_or(DEFAULT_SEARCH_LIMIT),
    )?;
    Ok(KnowledgeSearchResult {
        status: if cards.is_empty() {
            "empty".to_string()
        } else {
            "available".to_string()
        },
        cards,
    })
}

pub fn brainstorm_context(
    input: KnowledgeBrainstormContextInput,
) -> KnowledgeResult<KnowledgeBrainstormContextResult> {
    if !matches!(
        input.block.as_str(),
        "phase_scope" | "concept_grounding" | "frontend_experience"
    ) {
        return Err(KnowledgeError::invalid(
            "knowledge brainstorm context block must be phase_scope, concept_grounding, or frontend_experience",
        ));
    }
    if input.request_ref.trim().is_empty()
        || input.step_id.trim().is_empty()
        || input.query_subject.trim().is_empty()
    {
        return Err(KnowledgeError::invalid(
            "requestRef, stepId, and querySubject are required for knowledge brainstorm context",
        ));
    }
    let step_requirement = validate_brainstorm_request_scope(
        &input.project_root,
        &input.request_ref,
        &input.block,
        &input.step_id,
    )?;
    validate_brainstorm_query_id(&input, &step_requirement)?;
    let request_scope = resolve_brainstorm_request_scope(&input.project_root, &input.request_ref)?;
    let natural_query = format!("{} {}", input.query_subject, input.natural_language_query);
    let contextual_query = with_block_retrieval_intent(&natural_query, &input.block);
    let cards = search_cards(
        &contextual_query,
        &input.semantic_focus,
        &[],
        Some(&input.block),
        24,
    )?;
    let matched_sources = aggregate_sources(
        &cards,
        &input.semantic_focus,
        DEFAULT_CONTEXT_CHUNK_LIMIT_PER_SOURCE,
        MAX_CONTEXT_CHUNKS_PER_BLOCK,
    );
    let read_plan = KnowledgeReadPlan {
        mode: "inspect_all_listed_chunks".to_string(),
        chunks: matched_sources
            .iter()
            .flat_map(|source| {
                source
                    .top_chunks
                    .iter()
                    .map(|chunk| KnowledgeReadPlanChunk {
                        source_name: source.source_name.clone(),
                        build_id: source.build_id.clone(),
                        chunk_id: chunk.chunk_id.clone(),
                    })
            })
            .collect(),
    };
    let result = KnowledgeBrainstormContextResult {
        status: if matched_sources.is_empty() {
            "empty".to_string()
        } else {
            "available".to_string()
        },
        matched_sources: context_source_summaries(&matched_sources),
        read_plan,
    };
    persist_brainstorm_context(&input, &request_scope, &result)?;
    Ok(result)
}

fn context_source_summaries(
    matched_sources: &[KnowledgeMatchedSource],
) -> Vec<KnowledgeContextMatchedSource> {
    matched_sources
        .iter()
        .map(|source| KnowledgeContextMatchedSource {
            source_name: source.source_name.clone(),
            score: source.score,
            matched_focus_coverage: source.matched_focus_coverage,
            chunk_count: source.top_chunks.len(),
        })
        .collect()
}

fn with_block_retrieval_intent(query: &str, block: &str) -> String {
    let Some(intent) = block_retrieval_intent(block) else {
        return query.to_string();
    };
    if query.contains(intent) {
        return query.to_string();
    }
    [query.trim(), intent]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn block_retrieval_intent(block: &str) -> Option<&'static str> {
    match block {
        "phase_scope" => Some(PHASE_SCOPE_RETRIEVAL_INTENT),
        "concept_grounding" => Some(CONCEPT_GROUNDING_RETRIEVAL_INTENT),
        "frontend_experience" => Some(FRONTEND_EXPERIENCE_RETRIEVAL_INTENT),
        _ => None,
    }
}

fn search_cards(
    query: &str,
    semantic_focus: &[String],
    source_names: &[String],
    block: Option<&str>,
    limit: usize,
) -> KnowledgeResult<Vec<KnowledgeChunkCard>> {
    let registry = load_registry()?;
    let client = algorithm_client()?;
    let allowed_sources = source_names.iter().cloned().collect::<BTreeSet<_>>();
    let mut chunk_candidates = Vec::<SearchChunkCandidate>::new();
    let mut bm25_documents = Vec::<serde_json::Value>::new();
    for source in registry
        .sources
        .iter()
        .filter(|source| source.enabled)
        .filter(|source| allowed_sources.is_empty() || allowed_sources.contains(&source.name))
    {
        let Some(build_id) = source.current_build_id.as_deref() else {
            continue;
        };
        let chunks_file: ChunksFile =
            match read_json(&paths::chunks_file(&source.source_id, build_id)?) {
                Ok(value) => value,
                Err(_) => continue,
            };
        let docs = lexical_documents(&source.source_id, build_id, &chunks_file)?;
        let docs_by_chunk = docs
            .into_iter()
            .map(|document| (document.chunk_id, document.text))
            .collect::<BTreeMap<_, _>>();
        for chunk in &chunks_file.chunks {
            let document_id = lexical_document_id(&source.source_id, build_id, &chunk.chunk_id);
            if let Some(text) = docs_by_chunk.get(&chunk.chunk_id) {
                bm25_documents.push(serde_json::json!({
                    "id": document_id,
                    "text": text
                }));
            }
            chunk_candidates.push(SearchChunkCandidate {
                source_id: source.source_id.clone(),
                source_name: source.name.clone(),
                build_id: build_id.to_string(),
                chunk: chunk.clone(),
            });
        }
    }
    let lexical_scores = global_lexical_scores(&client, query, bm25_documents, limit)?;
    let semantic_focus = parse_semantic_focuses(semantic_focus);
    let mut candidates = Vec::new();
    for candidate in chunk_candidates {
        let document_id = lexical_document_id(
            &candidate.source_id,
            &candidate.build_id,
            &candidate.chunk.chunk_id,
        );
        let lexical = *lexical_scores.get(&document_id).unwrap_or(&0.0);
        let semantic = semantic_match(&candidate.chunk, &semantic_focus);
        let affinity = block_affinity_score(candidate.chunk.block_affinity.as_ref(), block);
        let score =
            lexical * 0.40 + semantic.score * 0.25 + semantic.completeness * 0.20 + affinity * 0.15;
        if score <= 0.0 {
            continue;
        }
        candidates.push(KnowledgeChunkCard {
            source_id: candidate.source_id.clone(),
            source_name: candidate.source_name.clone(),
            build_id: candidate.build_id.clone(),
            chunk_id: candidate.chunk.chunk_id.clone(),
            document_title: candidate.chunk.document_title.clone(),
            heading_path: candidate.chunk.heading_path.clone(),
            summary: candidate.chunk.summary.clone(),
            matched_labels: semantic.matched_labels,
            score: round_score(score),
        });
    }
    Ok(rank_chunk_cards(candidates, &semantic_focus, limit))
}

#[derive(Debug, Clone)]
struct SearchChunkCandidate {
    source_id: String,
    source_name: String,
    build_id: String,
    chunk: KnowledgeChunk,
}

#[derive(Debug, Clone)]
struct LexicalSearchDocument {
    chunk_id: String,
    text: String,
}

fn global_lexical_scores(
    client: &AlgorithmClient,
    query: &str,
    documents: Vec<serde_json::Value>,
    limit: usize,
) -> KnowledgeResult<BTreeMap<String, f64>> {
    if documents.is_empty() {
        return Ok(BTreeMap::new());
    }
    let bm25_limit = documents.len().min(limit.max(DEFAULT_SEARCH_LIMIT) * 20);
    let bm25 = client
        .call(&serde_json::json!({
            "operation": "bm25",
            "query": query,
            "documents": documents,
            "limit": bm25_limit
        }))
        .map_err(|error| KnowledgeError::invalid(error.to_string()))?;
    let raw_scores = bm25["matches"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            Some((
                item.get("documentId")?.as_str()?.to_string(),
                item.get("score")?.as_f64()?,
            ))
        })
        .collect::<Vec<_>>();
    let max_score = raw_scores
        .iter()
        .map(|(_, score)| *score)
        .fold(0.0_f64, f64::max);
    if max_score <= 0.0 {
        return Ok(BTreeMap::new());
    }
    Ok(raw_scores
        .into_iter()
        .map(|(document_id, score)| (document_id, round_score(score / max_score)))
        .collect())
}

fn lexical_document_id(source_id: &str, build_id: &str, chunk_id: &str) -> String {
    format!("{source_id}/{build_id}/{chunk_id}")
}

fn lexical_documents(
    source_id: &str,
    build_id: &str,
    chunks_file: &ChunksFile,
) -> KnowledgeResult<Vec<LexicalSearchDocument>> {
    if let Ok(lexical) = read_json::<LexicalIndex>(&paths::lexical_index_file(source_id, build_id)?)
    {
        if !lexical.documents.is_empty() {
            return Ok(lexical
                .documents
                .iter()
                .map(|document| LexicalSearchDocument {
                    chunk_id: document.id.clone(),
                    text: document.text.clone(),
                })
                .collect());
        }
    }

    Ok(chunks_file
        .chunks
        .iter()
        .map(|chunk| {
            let body = fs::read_to_string(
                paths::chunk_body_file(source_id, build_id, &chunk.chunk_id).unwrap_or_default(),
            )
            .unwrap_or_default();
            LexicalSearchDocument {
                chunk_id: chunk.chunk_id.clone(),
                text: lexical_text_for_search(chunk, &body),
            }
        })
        .collect())
}

fn lexical_text_for_search(chunk: &KnowledgeChunk, body: &str) -> String {
    weighted_lexical_text(
        &chunk.document_title,
        &chunk.heading_path,
        chunk.summary.as_deref(),
        &chunk
            .semantic_labels
            .iter()
            .map(|label| label.text.clone())
            .collect::<Vec<_>>(),
        &chunk.semantic_aliases,
        body,
    )
}

fn weighted_lexical_text(
    title: &str,
    heading_path: &[String],
    summary: Option<&str>,
    labels: &[String],
    aliases: &[String],
    body: &str,
) -> String {
    let heading = heading_path.join(" ");
    let label_text = labels.join(" ");
    let alias_text = aliases.join(" ");
    let summary = summary.unwrap_or_default();
    [
        repeat_field(title, 4),
        repeat_field(&heading, 4),
        repeat_field(summary, 3),
        repeat_field(&label_text, 5),
        repeat_field(&alias_text, 5),
        body.to_string(),
    ]
    .into_iter()
    .filter(|part| !part.trim().is_empty())
    .collect::<Vec<_>>()
    .join("\n")
}

fn repeat_field(value: &str, times: usize) -> String {
    std::iter::repeat_n(value.trim(), times)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn aggregate_sources(
    cards: &[KnowledgeChunkCard],
    semantic_focus: &[String],
    per_source_chunk_limit: usize,
    total_chunk_limit: usize,
) -> Vec<KnowledgeMatchedSource> {
    let semantic_focus = parse_semantic_focuses(semantic_focus);
    let require_focus_match = semantic_focus
        .iter()
        .any(|focus| !focus.text.trim().is_empty());
    let mut grouped: BTreeMap<(String, String, String), Vec<KnowledgeChunkCard>> = BTreeMap::new();
    for card in cards {
        grouped
            .entry((
                card.source_id.clone(),
                card.source_name.clone(),
                card.build_id.clone(),
            ))
            .or_default()
            .push(card.clone());
    }

    let mut sources = grouped
        .into_iter()
        .map(|((source_id, source_name, build_id), source_cards)| {
            let limited_cards =
                rank_chunk_cards(source_cards, &semantic_focus, per_source_chunk_limit);
            let best_chunk_score = limited_cards.first().map(|card| card.score).unwrap_or(0.0);
            let average_top3_chunk_score = {
                let top3 = limited_cards.iter().take(3).collect::<Vec<_>>();
                if top3.is_empty() {
                    0.0
                } else {
                    top3.iter().map(|card| card.score).sum::<f64>() / top3.len() as f64
                }
            };
            let matched_focus_coverage = focus_coverage(&limited_cards, &semantic_focus);
            let score = best_chunk_score * 0.55
                + average_top3_chunk_score * 0.25
                + matched_focus_coverage * 0.20;
            KnowledgeMatchedSource {
                source_id,
                source_name,
                build_id,
                score: round_score(score),
                best_chunk_score: round_score(best_chunk_score),
                average_top3_chunk_score: round_score(average_top3_chunk_score),
                matched_focus_coverage: round_score(matched_focus_coverage),
                top_chunks: limited_cards,
            }
        })
        .collect::<Vec<_>>();
    sources.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.source_name.cmp(&right.source_name))
            .then_with(|| left.source_id.cmp(&right.source_id))
    });
    if total_chunk_limit == usize::MAX {
        if require_focus_match {
            sources.retain(|source| source.matched_focus_coverage >= MIN_CONTEXT_FOCUS_COVERAGE);
        }
        return sources;
    }

    let mut selected = Vec::new();
    let mut remaining = total_chunk_limit;
    for mut source in sources {
        if require_focus_match && source.matched_focus_coverage < MIN_CONTEXT_FOCUS_COVERAGE {
            continue;
        }
        if selected.len() >= DEFAULT_CONTEXT_SOURCE_LIMIT || remaining == 0 {
            break;
        }
        let take = source.top_chunks.len().min(remaining);
        source.top_chunks.truncate(take);
        if source.top_chunks.is_empty() {
            continue;
        }
        source.best_chunk_score = round_score(source.top_chunks[0].score);
        source.average_top3_chunk_score = round_score(
            source
                .top_chunks
                .iter()
                .take(3)
                .map(|chunk| chunk.score)
                .sum::<f64>()
                / source.top_chunks.len().min(3) as f64,
        );
        source.matched_focus_coverage =
            round_score(focus_coverage(&source.top_chunks, &semantic_focus));
        source.score = round_score(
            source.best_chunk_score * 0.55
                + source.average_top3_chunk_score * 0.25
                + source.matched_focus_coverage * 0.20,
        );
        remaining -= source.top_chunks.len();
        selected.push(source);
    }
    selected
}

fn semantic_match(chunk: &KnowledgeChunk, semantic_focus: &[SemanticFocus]) -> SemanticChunkMatch {
    let expanded_focus = semantic_focus
        .iter()
        .flat_map(SemanticFocus::expanded)
        .collect::<Vec<_>>();
    if expanded_focus.is_empty() {
        return SemanticChunkMatch::empty();
    }

    let entries = semantic_entries(chunk);
    let mut focus_scores = BTreeMap::new();
    let mut matched_labels: Vec<KnowledgeMatchedLabel> = Vec::new();
    for focus in &expanded_focus {
        let mut best_score = 0.0f64;
        let mut best_entry: Option<&SemanticEntry> = None;
        for entry in &entries {
            if !focus.matches_kind(&entry.kind) {
                continue;
            }
            let score = entry.match_score(&focus.term);
            if score > best_score {
                best_score = score;
                best_entry = Some(entry);
            }
        }
        if let Some(entry) = best_entry {
            if best_score > 0.0 {
                focus_scores.insert(focus.key(), best_score);
                let matched = KnowledgeMatchedLabel {
                    kind: entry.kind.clone(),
                    text: entry.text.clone(),
                    match_source: entry.match_source.clone(),
                };
                if !matched_labels.iter().any(|item| {
                    item.kind == matched.kind
                        && item.text == matched.text
                        && item.match_source == matched.match_source
                }) {
                    matched_labels.push(matched);
                }
            }
        }
    }
    let score = focus_scores.values().sum::<f64>() / expanded_focus.len() as f64;
    let completeness = focus_scores.len() as f64 / expanded_focus.len() as f64;
    SemanticChunkMatch {
        score,
        completeness,
        matched_labels,
    }
}

fn block_affinity_score(affinity: Option<&BlockAffinity>, block: Option<&str>) -> f64 {
    let Some(affinity) = affinity else {
        return 0.0;
    };
    match block {
        Some("phase_scope") => affinity.phase_scope,
        Some("concept_grounding") => affinity.concept_grounding,
        Some("frontend_experience") => affinity.frontend_experience,
        _ => affinity
            .phase_scope
            .max(affinity.concept_grounding)
            .max(affinity.frontend_experience),
    }
}

fn semantic_entries(chunk: &KnowledgeChunk) -> Vec<SemanticEntry> {
    let object_labels = chunk
        .semantic_labels
        .iter()
        .filter(|label| matches!(label.kind.as_str(), "object" | "page" | "flow"))
        .map(|label| label.text.clone())
        .collect::<Vec<_>>();
    let mut entries = Vec::new();
    for label in &chunk.semantic_labels {
        let mut terms = expand_focus(&label.text);
        if let Some(normalized) = label.normalized_text.as_deref() {
            terms.extend(expand_focus(normalized));
            terms.push(normalize_focus(normalized));
        }
        for alias in &label.aliases {
            terms.extend(expand_focus(alias));
            terms.push(normalize_focus(alias));
        }
        terms.push(normalize_focus(&format!("{}:{}", label.kind, label.text)));
        if matches!(
            label.kind.as_str(),
            "operation" | "page_operation" | "rule" | "state"
        ) {
            for object in &object_labels {
                let normalized_object = normalize_focus(object);
                let normalized_text = normalize_focus(&label.text);
                if normalized_text.starts_with(&normalized_object)
                    && normalized_text.len() > normalized_object.len()
                {
                    terms.push(
                        normalized_text[normalized_object.len()..]
                            .trim()
                            .to_string(),
                    );
                } else {
                    terms.push(format!("{normalized_object}{normalized_text}"));
                }
            }
        }
        terms.sort();
        terms.dedup();
        entries.push(SemanticEntry {
            kind: label.kind.clone(),
            text: label.text.clone(),
            match_source: "text".to_string(),
            terms,
        });
    }
    for alias in &chunk.semantic_aliases {
        let mut terms = expand_focus(alias);
        terms.push(normalize_focus(alias));
        terms.sort();
        terms.dedup();
        entries.push(SemanticEntry {
            kind: "alias".to_string(),
            text: alias.clone(),
            match_source: "alias".to_string(),
            terms,
        });
    }
    if let Some(summary) = &chunk.summary {
        let mut terms = expand_focus(summary);
        terms.push(normalize_focus(summary));
        terms.sort();
        terms.dedup();
        entries.push(SemanticEntry {
            kind: "summary".to_string(),
            text: summary.clone(),
            match_source: "summary".to_string(),
            terms,
        });
    }
    entries
}

fn focus_coverage(cards: &[KnowledgeChunkCard], semantic_focus: &[SemanticFocus]) -> f64 {
    let focus_count = semantic_focus
        .iter()
        .filter(|focus| !focus.text.trim().is_empty())
        .count();
    if focus_count == 0 {
        return 0.0;
    }
    let matched = cards
        .iter()
        .flat_map(|card| card_focus_hits(card, semantic_focus))
        .collect::<BTreeSet<_>>();
    matched.len() as f64 / focus_count as f64
}

fn rank_chunk_cards(
    mut cards: Vec<KnowledgeChunkCard>,
    semantic_focus: &[SemanticFocus],
    limit: usize,
) -> Vec<KnowledgeChunkCard> {
    cards.sort_by(compare_chunk_cards);
    if semantic_focus
        .iter()
        .all(|focus| focus.text.trim().is_empty())
    {
        cards.truncate(limit);
        return cards;
    }

    let mut selected = Vec::new();
    let mut covered_focuses = BTreeSet::<usize>::new();
    while selected.len() < limit {
        let mut best_index = None;
        let mut best_hits = BTreeSet::<usize>::new();
        for (index, card) in cards.iter().enumerate() {
            let hits = card_focus_hits(card, semantic_focus);
            let new_hits = hits
                .difference(&covered_focuses)
                .copied()
                .collect::<BTreeSet<_>>();
            let best_new_hits = best_hits
                .difference(&covered_focuses)
                .copied()
                .collect::<BTreeSet<_>>();
            let new_hit_count = new_hits.len();
            let best_new_hit_count = best_new_hits.len();
            let first_new_hit = new_hits.iter().next().copied().unwrap_or(usize::MAX);
            let best_first_new_hit = best_new_hits.iter().next().copied().unwrap_or(usize::MAX);
            let better = new_hit_count > 0
                && (best_index.is_none()
                    || new_hit_count > best_new_hit_count
                    || (new_hit_count == best_new_hit_count && first_new_hit < best_first_new_hit)
                    || (new_hit_count == best_new_hit_count
                        && first_new_hit == best_first_new_hit
                        && (hits.len() > best_hits.len()
                            || (hits.len() == best_hits.len()
                                && best_index
                                    .map(|best| {
                                        compare_chunk_cards(card, &cards[best]) == Ordering::Less
                                    })
                                    .unwrap_or(false)))));
            if better {
                best_index = Some(index);
                best_hits = hits;
            }
        }

        let Some(index) = best_index else {
            break;
        };
        covered_focuses.extend(best_hits);
        selected.push(cards.remove(index));
    }

    let (mut focused, mut fallback): (Vec<_>, Vec<_>) = cards
        .into_iter()
        .partition(|card| !card_focus_hits(card, semantic_focus).is_empty());
    focused.sort_by(|left, right| compare_focused_chunk_cards(left, right, semantic_focus));
    fallback.sort_by(compare_chunk_cards);
    selected.extend(focused);
    selected.extend(fallback);
    selected.truncate(limit);
    selected
}

fn card_focus_hits(card: &KnowledgeChunkCard, semantic_focus: &[SemanticFocus]) -> BTreeSet<usize> {
    let mut hits = BTreeSet::new();
    for (index, focus) in semantic_focus.iter().enumerate() {
        if best_label_focus_hit(card, focus).covers_focus() {
            hits.insert(index);
        }
    }
    hits
}

fn compare_focused_chunk_cards(
    left: &KnowledgeChunkCard,
    right: &KnowledgeChunkCard,
    semantic_focus: &[SemanticFocus],
) -> Ordering {
    let left_quality = card_focus_quality(left, semantic_focus);
    let right_quality = card_focus_quality(right, semantic_focus);
    right_quality
        .exact_hits
        .cmp(&left_quality.exact_hits)
        .then_with(|| right_quality.strong_hits.cmp(&left_quality.strong_hits))
        .then_with(|| {
            right_quality
                .covered_hits()
                .cmp(&left_quality.covered_hits())
        })
        .then_with(|| compare_chunk_cards(left, right))
}

fn card_focus_quality(card: &KnowledgeChunkCard, semantic_focus: &[SemanticFocus]) -> FocusQuality {
    let mut quality = FocusQuality::default();
    for focus in semantic_focus {
        match best_label_focus_hit(card, focus) {
            FocusHitKind::Exact => quality.exact_hits += 1,
            FocusHitKind::Strong => quality.strong_hits += 1,
            FocusHitKind::Broad => quality.broad_hits += 1,
            FocusHitKind::None => {}
        }
    }
    quality
}

fn best_label_focus_hit(card: &KnowledgeChunkCard, focus: &SemanticFocus) -> FocusHitKind {
    card.matched_labels
        .iter()
        .map(|label| label_focus_hit(label, focus))
        .max()
        .unwrap_or(FocusHitKind::None)
}

fn label_focus_hit(label: &KnowledgeMatchedLabel, focus: &SemanticFocus) -> FocusHitKind {
    if !focus.matches_kind(&label.kind) {
        return FocusHitKind::None;
    }
    let focus_terms = focus
        .expanded_terms()
        .into_iter()
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();
    if focus_terms.is_empty() {
        return FocusHitKind::None;
    }
    let mut label_terms = expand_focus(&label.text);
    label_terms.push(normalize_focus(&format!("{}:{}", label.kind, label.text)));
    label_terms.sort();
    label_terms.dedup();
    if label_terms.iter().any(|label_term| {
        focus_terms
            .iter()
            .any(|focus_term| label_term == focus_term)
    }) {
        return FocusHitKind::Exact;
    }
    let label_contains_focus = label_terms.iter().any(|label_term| {
        focus_terms
            .iter()
            .any(|focus_term| label_term.contains(focus_term))
    });
    if label_contains_focus {
        return if is_focus_covering_kind(&label.kind) {
            FocusHitKind::Strong
        } else {
            FocusHitKind::Broad
        };
    }
    let focus_contains_label = label_terms.iter().any(|label_term| {
        focus_terms
            .iter()
            .any(|focus_term| focus_term.contains(label_term))
    });
    if focus_contains_label {
        return if is_compact_focus_covering_kind(&label.kind) {
            FocusHitKind::Strong
        } else {
            FocusHitKind::Broad
        };
    }
    FocusHitKind::None
}

fn is_focus_covering_kind(kind: &str) -> bool {
    matches!(
        kind,
        "operation" | "page_operation" | "rule" | "flow" | "alias"
    )
}

fn is_compact_focus_covering_kind(kind: &str) -> bool {
    matches!(kind, "operation" | "page_operation" | "rule" | "flow")
}

fn parse_semantic_focuses(values: &[String]) -> Vec<SemanticFocus> {
    values
        .iter()
        .filter_map(|value| SemanticFocus::parse(value))
        .collect()
}

fn validate_brainstorm_request_scope(
    project_root: &str,
    request_ref: &str,
    block: &str,
    step_id: &str,
) -> KnowledgeResult<BrainstormKnowledgeStepRequirement> {
    state::inspect_request(delivery_core::InspectRequestInput {
        project_root: project_root.to_string(),
        request_ref: request_ref.to_string(),
    })
    .map_err(|error| KnowledgeError::invalid(error.to_string()))?;
    let request_id = parse_request_id(request_ref)?;
    let request_index = state::request_index::get_request_index_entry(project_root, &request_id)
        .map_err(|error| KnowledgeError::invalid(error.to_string()))?;
    let project_paths = state::paths::project_paths(project_root)
        .map_err(|error| KnowledgeError::invalid(error.to_string()))?;
    let request_file =
        state::paths::from_project_relative(&project_paths.root, &request_index.request_file)
            .map_err(|error| KnowledgeError::invalid(error.to_string()))?;
    let request_root = state::store::read_json_value(&request_file)
        .map_err(|error| KnowledgeError::invalid(error.to_string()))?;
    let execution_order = request_root
        .pointer(&format!(
            "/knowledgeQueryPlan/blocks/{block}/executionOrder"
        ))
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            KnowledgeError::invalid(format!(
                "knowledgeQueryPlan.blocks.{block}.executionOrder is missing from request"
            ))
        })?;
    let Some(step) = execution_order.iter().find(|step| {
        step.get("stepId")
            .and_then(serde_json::Value::as_str)
            .map(|value| value == step_id)
            .unwrap_or(false)
    }) else {
        return Err(KnowledgeError::invalid(format!(
            "stepId {step_id} does not belong to request knowledgeQueryPlan block {block}"
        )));
    };
    Ok(BrainstormKnowledgeStepRequirement {
        repeat_mode: step
            .get("repeatMode")
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string),
    })
}

fn validate_brainstorm_query_id(
    input: &KnowledgeBrainstormContextInput,
    step_requirement: &BrainstormKnowledgeStepRequirement,
) -> KnowledgeResult<()> {
    if step_requirement.repeat_mode.as_deref() != Some("per_candidate_phase_cut") {
        return Ok(());
    }
    let query_id = input
        .query_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            KnowledgeError::invalid(
                "queryId is required for per_candidate_phase_cut knowledge steps",
            )
        })?;
    validate_query_id_segment(query_id)?;
    if query_id == "atomic_scope"
        && input
            .atomic_scope_reason
            .as_deref()
            .map(str::trim)
            .unwrap_or_default()
            .is_empty()
    {
        return Err(KnowledgeError::invalid(
            "atomicScopeReason is required when queryId is atomic_scope",
        ));
    }
    Ok(())
}

fn validate_query_id_segment(query_id: &str) -> KnowledgeResult<()> {
    let valid = query_id.len() <= 80
        && query_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-');
    if valid {
        return Ok(());
    }
    Err(KnowledgeError::invalid(
        "queryId must be 1-80 ASCII letters, numbers, underscores, or hyphens",
    ))
}

fn resolve_brainstorm_request_scope(
    project_root: &str,
    request_ref: &str,
) -> KnowledgeResult<BrainstormRequestScope> {
    let request_id = parse_request_id(request_ref)?;
    let request_index = state::request_index::get_request_index_entry(project_root, &request_id)
        .map_err(|error| KnowledgeError::invalid(error.to_string()))?;
    let delivery_id = request_index.delivery_id.ok_or_else(|| {
        KnowledgeError::invalid(format!(
            "requestRef {request_ref} is missing deliveryId in request index"
        ))
    })?;
    let phase_id = request_index.phase_id.ok_or_else(|| {
        KnowledgeError::invalid(format!(
            "requestRef {request_ref} is missing phaseId in request index"
        ))
    })?;
    Ok(BrainstormRequestScope {
        request_id,
        delivery_id,
        phase_id,
    })
}

fn persist_brainstorm_context(
    input: &KnowledgeBrainstormContextInput,
    scope: &BrainstormRequestScope,
    result: &KnowledgeBrainstormContextResult,
) -> KnowledgeResult<()> {
    let project_paths = state::paths::project_paths(&input.project_root)
        .map_err(|error| KnowledgeError::invalid(error.to_string()))?;
    let locator = state::paths::DeliveryPhaseLocator {
        delivery_id: scope.delivery_id.clone(),
        phase_id: scope.phase_id.clone(),
    };
    let step_dir = state::paths::workspace_dir(&project_paths.root, &locator)
        .join("brainstorm-knowledge")
        .join(&scope.request_id)
        .join(&input.block)
        .join(&input.step_id);
    let output_dir = match input.query_id.as_deref().map(str::trim) {
        Some(query_id) if !query_id.is_empty() => {
            validate_query_id_segment(query_id)?;
            step_dir.join(query_id)
        }
        _ => step_dir,
    };
    state::store::ensure_dir(&output_dir)
        .map_err(|error| KnowledgeError::invalid(error.to_string()))?;
    state::store::write_json_atomic(
        &output_dir.join("query.json"),
        &serde_json::json!({
            "schemaVersion": "1.0",
            "requestRef": input.request_ref,
            "requestId": scope.request_id,
            "deliveryId": scope.delivery_id,
            "phaseId": scope.phase_id,
            "block": input.block,
            "stepId": input.step_id,
            "queryId": input.query_id,
            "atomicScopeReason": input.atomic_scope_reason,
            "querySubject": input.query_subject,
            "naturalLanguageQuery": input.natural_language_query,
            "semanticFocus": input.semantic_focus,
        }),
    )
    .map_err(|error| KnowledgeError::invalid(error.to_string()))?;
    state::store::write_json_atomic(&output_dir.join("result.json"), result)
        .map_err(|error| KnowledgeError::invalid(error.to_string()))?;
    Ok(())
}

fn parse_request_id(request_ref: &str) -> KnowledgeResult<String> {
    let rest = request_ref
        .strip_prefix("loom://projects/")
        .ok_or_else(|| KnowledgeError::invalid(format!("invalid requestRef: {request_ref}")))?;
    let mut parts = rest.split("/requests/");
    let _project_id = parts
        .next()
        .ok_or_else(|| KnowledgeError::invalid(format!("invalid requestRef: {request_ref}")))?;
    let request_id = parts
        .next()
        .ok_or_else(|| KnowledgeError::invalid(format!("invalid requestRef: {request_ref}")))?;
    if request_id.is_empty() {
        return Err(KnowledgeError::invalid(format!(
            "invalid requestRef: {request_ref}"
        )));
    }
    Ok(request_id.to_string())
}

fn expand_focus(value: &str) -> Vec<String> {
    let normalized = normalize_focus(value);
    let mut values = vec![normalized.clone()];
    values.extend(split_compound_focus(&normalized));
    values.extend(strip_common_semantic_suffixes(&normalized));
    for part in split_compound_focus(&normalized) {
        values.extend(strip_common_semantic_suffixes(&part));
    }
    values.sort();
    values.dedup();
    values
}

fn split_compound_focus(value: &str) -> Vec<String> {
    value
        .split(|ch: char| {
            matches!(
                ch,
                ':' | '/' | '-' | '_' | ' ' | '，' | ',' | '、' | '>' | '|'
            )
        })
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_string)
        .collect()
}

fn strip_common_semantic_suffixes(value: &str) -> Vec<String> {
    let suffixes = [
        "前置条件",
        "条件",
        "能力",
        "规则",
        "约束",
        "路径",
        "流程",
        "状态",
        "操作",
        "办理",
        "结果",
        "信息",
        "记录",
        "关系",
        "限制",
        "要求",
        "conditions",
        "condition",
        "rules",
        "rule",
        "workflow",
        "status",
        "operation",
        "result",
        "path",
    ];
    let mut values = Vec::new();
    for suffix in suffixes {
        let Some(stripped) = value.strip_suffix(suffix) else {
            continue;
        };
        if stripped.chars().count() >= 2 {
            values.push(stripped.to_string());
        }
    }
    values
}

fn normalize_focus(value: &str) -> String {
    value
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>()
        .to_ascii_lowercase()
}

fn compare_chunk_cards(left: &KnowledgeChunkCard, right: &KnowledgeChunkCard) -> Ordering {
    right
        .score
        .partial_cmp(&left.score)
        .unwrap_or(Ordering::Equal)
        .then_with(|| left.source_name.cmp(&right.source_name))
        .then_with(|| left.chunk_id.cmp(&right.chunk_id))
}

fn round_score(value: f64) -> f64 {
    (value * 100_000.0).round() / 100_000.0
}

fn algorithm_client() -> KnowledgeResult<AlgorithmClient> {
    AlgorithmClient::from_environment().map_err(|error| KnowledgeError::invalid(error.to_string()))
}

#[derive(Debug, Clone)]
struct BrainstormRequestScope {
    request_id: String,
    delivery_id: String,
    phase_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SemanticFocus {
    kind: Option<String>,
    text: String,
}

impl SemanticFocus {
    fn parse(value: &str) -> Option<Self> {
        let value = value.trim();
        if value.is_empty() {
            return None;
        }
        let Some((kind, text)) = value.split_once(':') else {
            return Some(Self {
                kind: None,
                text: value.to_string(),
            });
        };
        let kind = normalize_focus_kind(kind)?;
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        Some(Self {
            kind: Some(kind),
            text: text.to_string(),
        })
    }

    fn expanded_terms(&self) -> Vec<String> {
        let mut values = expand_focus(&self.text);
        if let Some(kind) = &self.kind {
            values.push(normalize_focus(&format!("{kind}:{}", self.text)));
        }
        values.sort();
        values.dedup();
        values
    }

    fn expanded(&self) -> Vec<ExpandedSemanticFocus> {
        self.expanded_terms()
            .into_iter()
            .map(|term| ExpandedSemanticFocus {
                kind: self.kind.clone(),
                term,
            })
            .collect()
    }

    fn matches_kind(&self, label_kind: &str) -> bool {
        let Some(focus_kind) = self.kind.as_deref() else {
            return true;
        };
        semantic_focus_kind_matches(label_kind, focus_kind)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpandedSemanticFocus {
    kind: Option<String>,
    term: String,
}

impl ExpandedSemanticFocus {
    fn key(&self) -> String {
        match &self.kind {
            Some(kind) => format!("{kind}:{}", self.term),
            None => self.term.clone(),
        }
    }

    fn matches_kind(&self, label_kind: &str) -> bool {
        let Some(focus_kind) = self.kind.as_deref() else {
            return true;
        };
        semantic_focus_kind_matches(label_kind, focus_kind)
    }
}

fn normalize_focus_kind(kind: &str) -> Option<String> {
    match kind.trim() {
        "object" | "operation" | "state" | "rule" | "field" | "page" | "flow" | "other" => {
            Some(kind.trim().to_string())
        }
        _ => None,
    }
}

fn semantic_focus_kind_matches(label_kind: &str, focus_kind: &str) -> bool {
    label_kind == focus_kind
        || (matches!(label_kind, "flow" | "page_operation") && focus_kind == "operation")
        || (label_kind == "operation" && focus_kind == "flow")
        || (label_kind == "state" && focus_kind == "rule")
        || (label_kind == "rule" && focus_kind == "state")
}

#[derive(Debug, Clone)]
struct SemanticEntry {
    kind: String,
    text: String,
    match_source: String,
    terms: Vec<String>,
}

impl SemanticEntry {
    fn match_score(&self, focus: &str) -> f64 {
        if self.terms.iter().any(|term| term == focus) {
            return 1.0;
        }
        if self.terms.iter().any(|term| term.contains(focus)) {
            return if self.kind == "summary" {
                0.35
            } else if is_focus_covering_kind(&self.kind) {
                0.80
            } else {
                0.45
            };
        }
        if self.terms.iter().any(|term| focus.contains(term)) {
            return if is_compact_focus_covering_kind(&self.kind) {
                0.55
            } else {
                0.20
            };
        }
        0.0
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct FocusQuality {
    exact_hits: usize,
    strong_hits: usize,
    broad_hits: usize,
}

impl FocusQuality {
    fn covered_hits(&self) -> usize {
        self.exact_hits + self.strong_hits
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FocusHitKind {
    None,
    Broad,
    Strong,
    Exact,
}

impl FocusHitKind {
    fn covers_focus(self) -> bool {
        matches!(self, FocusHitKind::Exact | FocusHitKind::Strong)
    }
}

#[derive(Debug, Clone)]
struct SemanticChunkMatch {
    score: f64,
    completeness: f64,
    matched_labels: Vec<KnowledgeMatchedLabel>,
}

impl SemanticChunkMatch {
    fn empty() -> Self {
        Self {
            score: 0.0,
            completeness: 0.0,
            matched_labels: vec![],
        }
    }
}
