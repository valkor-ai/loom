use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    ImplementationAction, ReferenceLoadPlanItem, TaskDefinition, TaskKind,
    TechnicalBaselineContract,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CodeStackSignal {
    pub source_track: String,
    pub source_path: String,
    pub raw_selection: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub frameworks: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dialects: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roles: Vec<String>,
    pub confidence: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct CodeReferenceSelection {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stack_signals: Vec<CodeStackSignal>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub reference_groups: BTreeMap<String, Vec<String>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub focus_tags: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unmapped_signals: Vec<CodeStackSignal>,
}

pub fn build_code_quality_seed(baseline: &TechnicalBaselineContract) -> Value {
    let signals = code_stack_signals_from_baseline(&baseline.stack);
    if signals.is_empty() {
        return Value::Null;
    }
    let reference_groups = baseline_reference_groups(&signals);
    let reference_load_plan = code_reference_load_plan(&reference_groups);
    json!({
        "required": !reference_groups.is_empty(),
        "qualityLevel": "production_code_implementation",
        "codeStackSignals": signals,
        "unmappedSignals": signals.iter().filter(|signal| signal.language.is_none()).collect::<Vec<_>>(),
        "techReferenceProfile": {
            "loadMode": "mcp_reference_load_plan",
            "groups": {
                "code": reference_groups
            },
            "referenceLoadPlan": reference_load_plan
        },
        "generationRules": [
            "Use TechnicalBaseline.stack only as the source fact for stack signals; do not reselect or reconfirm the technology stack.",
            "Use codeStackSignals as derived signals, then select code references by current task scope.",
            "Read only files listed in techReferenceProfile.referenceLoadPlan; selected code groups are semantic evidence labels, not path maps.",
            "Do not attach SQL references to every backend task merely because a database exists; attach SQL only for schema, migration, query, reporting, dialect, or optimization work.",
            "If a stack signal is low confidence or unmapped, preserve existing repository style and verification instead of guessing a nearby language profile."
        ]
    })
}

pub fn code_quality_enum_refs() -> Value {
    json!({
        "knownReferenceGroups": {
            "code": {
                "java": ["core", "spring", "persistence", "security", "reactive", "testing"],
                "springboot": ["web", "data", "security", "testing", "runtime", "cloud"],
                "django": ["models", "serializers", "views", "security", "testing"],
                "fastapi": ["schemas", "data", "routing", "security", "testing", "migration"],
                "aspnetcore": ["minimal", "architecture", "data", "security", "testing", "runtime"],
                "nestjs": ["controllers", "dtos", "services", "security", "testing", "migration"],
                "typescript": ["core", "types", "guards", "config", "patterns", "testing"],
                "javascript": ["core", "async", "modules", "node", "browser", "testing"],
                "python": ["core", "typing", "async", "packaging", "testing"],
                "go": ["core", "concurrency", "interfaces", "structure", "generics", "testing"],
                "csharp": ["core", "aspnet", "persistence", "blazor", "performance", "testing"],
                "cpp": ["core", "modern", "templates", "performance", "concurrency", "build", "testing"],
                "kotlin": ["core", "coroutines", "ktor", "compose", "multiplatform", "testing"],
                "php": ["core", "laravel", "symfony", "async", "testing"],
                "rust": ["core", "ownership", "traits", "errors", "async", "testing"],
                "swift": ["core", "swiftui", "concurrency", "protocols", "memory", "testing"],
                "sql": ["schema", "queries", "dialects", "optimization", "windows"]
            }
        },
        "focusTag": ["api", "frontend", "persistence", "security", "async", "performance", "configuration", "runtime", "integration", "migration", "architecture", "testing", "sql", "generics", "analytics", "memory"],
        "confidence": ["high", "medium", "low"]
    })
}

pub fn code_reference_selection_for_task(
    baseline: &TechnicalBaselineContract,
    task: &TaskDefinition,
) -> Option<CodeReferenceSelection> {
    let signals = code_stack_signals_from_baseline(&baseline.stack);
    if signals.is_empty() {
        return None;
    }
    let focus_tags = task_focus_tags(task);
    let mut selected_signals = Vec::new();
    let mut reference_groups = BTreeMap::<String, BTreeSet<String>>::new();
    let mut unmapped_signals = Vec::new();

    for signal in signals {
        let Some(language) = &signal.language else {
            unmapped_signals.push(signal);
            continue;
        };
        if !signal_applies_to_task(&signal, &focus_tags) {
            continue;
        }
        let items = reference_items_for_signal(&signal, &focus_tags);
        let backend_items = backend_reference_items_for_signal(&signal, &focus_tags);
        if !items.is_empty() || !backend_items.is_empty() {
            selected_signals.push(signal.clone());
            reference_groups
                .entry(language.clone())
                .or_default()
                .extend(items);
            for (framework, items) in backend_items {
                reference_groups.entry(framework).or_default().extend(items);
            }
        }
    }

    if reference_groups.is_empty() && unmapped_signals.is_empty() {
        return None;
    }
    Some(CodeReferenceSelection {
        stack_signals: selected_signals,
        reference_groups: reference_groups
            .into_iter()
            .map(|(language, items)| (language, items.into_iter().collect()))
            .collect(),
        focus_tags,
        unmapped_signals,
    })
}

pub fn code_reference_load_plan(
    reference_groups: &BTreeMap<String, Vec<String>>,
) -> Vec<ReferenceLoadPlanItem> {
    let mut load_plan = Vec::new();
    if !reference_groups.is_empty() {
        load_plan.push(ReferenceLoadPlanItem {
            ref_id: "tech.code.common".to_string(),
            path: "tech/code/common.md".to_string(),
            reason: "Common Loom code quality rules for repository adaptation, delivery evidence, and verification.".to_string(),
        });
    }
    load_plan.extend(reference_groups.iter().flat_map(|(group_key, groups)| {
        groups
            .iter()
            .map(move |group| reference_load_plan_item(group_key, group))
    }));
    load_plan
}

pub fn code_stack_signals_from_baseline(stack: &Value) -> Vec<CodeStackSignal> {
    let mut signals = Vec::new();
    if let Some(tracks) = stack.get("tracks").and_then(Value::as_object) {
        for (track, value) in tracks {
            if let Some(selection) = stack_track_selection(value) {
                signals.push(signal_from_selection(
                    track,
                    &format!("stack.tracks.{track}.selection"),
                    &selection,
                ));
            }
        }
    }
    for key in [
        "language",
        "languages",
        "backend",
        "web",
        "app",
        "framework",
        "frameworks",
        "persistence",
        "database",
        "databaseProvider",
        "dataAccess",
        "orm",
        "migrationTool",
    ] {
        if let Some(selection) = stack.get(key).and_then(compact_stack_value) {
            signals.push(signal_from_selection(
                key,
                &format!("stack.{key}"),
                &selection,
            ));
        }
    }
    dedupe_signals(signals)
}

fn baseline_reference_groups(signals: &[CodeStackSignal]) -> BTreeMap<String, Vec<String>> {
    let mut groups = BTreeMap::<String, BTreeSet<String>>::new();
    let focus_tags = vec![
        "api".to_string(),
        "frontend".to_string(),
        "persistence".to_string(),
        "configuration".to_string(),
        "testing".to_string(),
    ];
    for signal in signals {
        let Some(language) = &signal.language else {
            continue;
        };
        groups
            .entry(language.clone())
            .or_default()
            .extend(reference_items_for_signal(signal, &focus_tags));
        for (framework, items) in backend_reference_items_for_signal(signal, &focus_tags) {
            groups.entry(framework).or_default().extend(items);
        }
    }
    groups
        .into_iter()
        .map(|(language, items)| (language, items.into_iter().collect()))
        .collect()
}

fn stack_track_selection(value: &Value) -> Option<String> {
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_ascii_lowercase();
    if matches!(
        status.as_str(),
        "not_needed" | "not_applicable" | "none" | "disabled"
    ) {
        return None;
    }
    value
        .get("selection")
        .and_then(compact_stack_value)
        .filter(|selection| !selection_is_absent(selection))
}

fn compact_stack_value(value: &Value) -> Option<String> {
    match value {
        Value::String(text) => {
            let trimmed = text.trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        }
        Value::Array(items) => {
            let values = items
                .iter()
                .filter_map(compact_stack_value)
                .collect::<Vec<_>>();
            (!values.is_empty()).then(|| values.join(", "))
        }
        Value::Object(object) => object
            .get("selection")
            .and_then(compact_stack_value)
            .or_else(|| object.get("name").and_then(compact_stack_value)),
        _ => None,
    }
}

fn signal_from_selection(track: &str, source_path: &str, raw_selection: &str) -> CodeStackSignal {
    let haystack = normalized(raw_selection);
    let mut roles = role_from_track(track);
    let mut frameworks = Vec::new();
    let mut dialects = Vec::new();
    let mut language = None;

    if contains_any(&haystack, &["typescript", "type script", " ts ", "tsx"]) {
        language = Some("typescript".to_string());
        push_if_contains(&haystack, &mut frameworks, "react", &["react", "next"]);
        push_if_contains(&haystack, &mut frameworks, "vue", &["vue", "nuxt"]);
        push_if_contains(&haystack, &mut frameworks, "svelte", &["svelte"]);
        push_if_contains(&haystack, &mut frameworks, "node", &["node"]);
        push_if_contains(&haystack, &mut frameworks, "nestjs", &["nestjs", "nest js"]);
        if contains_any(&haystack, &["react", "next", "vue", "nuxt", "svelte"]) {
            push_unique(&mut roles, "frontend");
        }
        if contains_any(
            &haystack,
            &["node", "express", "nestjs", "nest js", "fastify"],
        ) {
            push_unique(&mut roles, "backend");
        }
    } else if contains_any(
        &haystack,
        &["javascript", " js ", "node", "express", "nestjs", "fastify"],
    ) {
        language = Some("javascript".to_string());
        push_if_contains(&haystack, &mut frameworks, "node", &["node"]);
        push_if_contains(&haystack, &mut frameworks, "express", &["express"]);
        push_if_contains(&haystack, &mut frameworks, "nestjs", &["nestjs", "nest js"]);
        push_if_contains(&haystack, &mut frameworks, "react", &["react", "next"]);
        push_if_contains(&haystack, &mut frameworks, "vue", &["vue", "nuxt"]);
        push_if_contains(&haystack, &mut frameworks, "svelte", &["svelte"]);
        if contains_any(&haystack, &["react", "next", "vue", "nuxt", "svelte"]) {
            push_unique(&mut roles, "frontend");
        }
        if contains_any(
            &haystack,
            &["node", "express", "nestjs", "nest js", "fastify"],
        ) {
            push_unique(&mut roles, "backend");
        }
    } else if contains_any(&haystack, &["java", "spring", "jpa", "hibernate"]) {
        language = Some("java".to_string());
        push_if_contains(
            &haystack,
            &mut frameworks,
            "spring_boot",
            &["spring boot", "springboot", "spring"],
        );
        push_if_contains(
            &haystack,
            &mut frameworks,
            "spring_cloud",
            &[
                "spring cloud",
                "cloud gateway",
                "spring cloud gateway",
                "config server",
                "spring cloud config",
                "eureka",
            ],
        );
        push_if_contains(
            &haystack,
            &mut frameworks,
            "spring_data_jpa",
            &["spring data", "jpa", "hibernate"],
        );
        push_backend_unless_persistence_track(&mut roles);
    } else if contains_any(
        &haystack,
        &[
            "csharp",
            "c#",
            ".net",
            "dotnet",
            "asp.net",
            "ef core",
            "entity framework",
        ],
    ) {
        language = Some("csharp".to_string());
        push_if_contains(
            &haystack,
            &mut frameworks,
            "aspnet_core",
            &[
                "asp.net",
                "aspnet",
                "aspnet core",
                "asp.net core",
                "minimal api",
            ],
        );
        push_if_contains(
            &haystack,
            &mut frameworks,
            "entity_framework",
            &["entity framework", "ef core"],
        );
        push_if_contains(&haystack, &mut frameworks, "blazor", &["blazor"]);
        push_backend_unless_persistence_track(&mut roles);
    } else if contains_any(
        &haystack,
        &["golang", " go ", "gin", "gofiber", "fiber", "grpc"],
    ) {
        language = Some("go".to_string());
        push_if_contains(&haystack, &mut frameworks, "gin", &["gin"]);
        push_if_contains(&haystack, &mut frameworks, "fiber", &["fiber"]);
        push_if_contains(&haystack, &mut frameworks, "grpc", &["grpc"]);
        push_unique(&mut roles, "backend");
    } else if contains_any(
        &haystack,
        &[
            "python",
            "fastapi",
            "django",
            "drf",
            "flask",
            "sqlalchemy",
            "pydantic",
        ],
    ) {
        language = Some("python".to_string());
        push_if_contains(&haystack, &mut frameworks, "fastapi", &["fastapi"]);
        push_if_contains(&haystack, &mut frameworks, "django", &["django"]);
        push_if_contains(
            &haystack,
            &mut frameworks,
            "django_rest_framework",
            &["django rest framework", "drf"],
        );
        push_if_contains(&haystack, &mut frameworks, "pydantic", &["pydantic"]);
        push_if_contains(
            &haystack,
            &mut frameworks,
            "sqlalchemy",
            &["sqlalchemy", "sql alchemy"],
        );
        push_if_contains(&haystack, &mut frameworks, "flask", &["flask"]);
        push_backend_unless_persistence_track(&mut roles);
    } else if contains_any(&haystack, &["rust", "cargo", "tokio", "axum", "actix"]) {
        language = Some("rust".to_string());
        push_if_contains(&haystack, &mut frameworks, "tokio", &["tokio"]);
        push_if_contains(&haystack, &mut frameworks, "axum", &["axum"]);
        push_if_contains(&haystack, &mut frameworks, "actix", &["actix"]);
        push_backend_unless_persistence_track(&mut roles);
    } else if contains_any(&haystack, &["kotlin", "ktor", "android", "compose", "kmp"]) {
        language = Some("kotlin".to_string());
        push_if_contains(&haystack, &mut frameworks, "ktor", &["ktor"]);
        push_if_contains(&haystack, &mut frameworks, "compose", &["compose"]);
        push_if_contains(&haystack, &mut frameworks, "kmp", &["kmp", "multiplatform"]);
    } else if contains_any(&haystack, &["php", "laravel", "symfony"]) {
        language = Some("php".to_string());
        push_if_contains(&haystack, &mut frameworks, "laravel", &["laravel"]);
        push_if_contains(&haystack, &mut frameworks, "symfony", &["symfony"]);
        push_unique(&mut roles, "backend");
    } else if contains_any(&haystack, &["swift", "swiftui", "vapor"]) {
        language = Some("swift".to_string());
        push_if_contains(&haystack, &mut frameworks, "swiftui", &["swiftui"]);
        push_if_contains(&haystack, &mut frameworks, "vapor", &["vapor"]);
    } else if contains_any(&haystack, &["c++", "cpp", "cmake", "clang", "gcc"]) {
        language = Some("cpp".to_string());
        push_if_contains(&haystack, &mut frameworks, "cmake", &["cmake"]);
    } else if contains_any(&haystack, &["react", "next", "vue", "nuxt", "svelte"]) {
        push_if_contains(&haystack, &mut frameworks, "react", &["react", "next"]);
        push_if_contains(&haystack, &mut frameworks, "vue", &["vue", "nuxt"]);
        push_if_contains(&haystack, &mut frameworks, "svelte", &["svelte"]);
        push_unique(&mut roles, "frontend");
    } else if contains_any(
        &haystack,
        &[
            "postgres",
            "postgresql",
            "mysql",
            "sqlite",
            "mariadb",
            "sql server",
            "mssql",
            "oracle",
            "cockroach",
        ],
    ) {
        language = Some("sql".to_string());
        push_if_contains(
            &haystack,
            &mut dialects,
            "postgresql",
            &["postgres", "postgresql"],
        );
        push_if_contains(&haystack, &mut dialects, "mysql", &["mysql"]);
        push_if_contains(&haystack, &mut dialects, "sqlite", &["sqlite"]);
        push_if_contains(
            &haystack,
            &mut dialects,
            "sql_server",
            &["sql server", "mssql"],
        );
        push_if_contains(&haystack, &mut dialects, "oracle", &["oracle"]);
        push_unique(&mut roles, "database");
    }

    if contains_any(
        &haystack,
        &[
            "jpa",
            "hibernate",
            "entity framework",
            "ef core",
            "sqlalchemy",
            "database",
            "postgres",
            "mysql",
            "sqlite",
        ],
    ) {
        push_unique(&mut roles, "persistence");
    }

    let confidence = if language.is_some() { "high" } else { "low" }.to_string();
    let reason = if language.is_some() {
        "Mapped from confirmed TechnicalBaseline stack selection.".to_string()
    } else {
        "No known Loom code reference profile matched this stack selection.".to_string()
    };
    CodeStackSignal {
        source_track: track.to_string(),
        source_path: source_path.to_string(),
        raw_selection: raw_selection.to_string(),
        language,
        frameworks,
        dialects,
        roles,
        confidence,
        reason,
    }
}

fn role_from_track(track: &str) -> Vec<String> {
    let mut roles = Vec::new();
    match normalized(track).as_str() {
        value if value.contains("web") || value.contains("frontend") || value.contains("app") => {
            push_unique(&mut roles, "frontend")
        }
        value if value.contains("backend") || value.contains("api") || value.contains("server") => {
            push_unique(&mut roles, "backend")
        }
        value
            if value.contains("persistence")
                || value.contains("database")
                || value.contains("dataaccess")
                || value.contains("orm") =>
        {
            push_unique(&mut roles, "persistence")
        }
        _ => {}
    }
    roles
}

fn task_focus_tags(task: &TaskDefinition) -> Vec<String> {
    let mut tags = Vec::new();
    push_unique(&mut tags, "testing");
    if task_is_frontend_task(task) {
        push_unique(&mut tags, "frontend");
    }
    if task_owns_api_contract(task) {
        push_unique(&mut tags, "api");
    }
    if task_owns_persistence(task) {
        push_unique(&mut tags, "persistence");
        push_unique(&mut tags, "sql");
    }
    if task_is_backend_task(task) {
        push_unique(&mut tags, "backend");
    }
    if matches!(task.task_kind, TaskKind::ConfigurationSupport) {
        push_unique(&mut tags, "configuration");
    }
    let text = normalized(&format!(
        "{} {} {:?}",
        task.title, task.objective, task.implementation_actions
    ));
    if contains_any(
        &text,
        &[
            "auth",
            "security",
            "login",
            "jwt",
            "oauth",
            "permission",
            "权限",
            "认证",
            "授权",
        ],
    ) {
        push_unique(&mut tags, "security");
    }
    if contains_any(
        &text,
        &[
            "async",
            "concurrent",
            "queue",
            "stream",
            "reactive",
            "websocket",
            "并发",
            "异步",
        ],
    ) {
        push_unique(&mut tags, "async");
    }
    if contains_any(
        &text,
        &[
            "performance",
            "optimi",
            "slow",
            "index",
            "cache",
            "性能",
            "优化",
            "索引",
            "缓存",
        ],
    ) {
        push_unique(&mut tags, "performance");
    }
    if contains_any(
        &text,
        &[
            "runtime",
            "actuator",
            "health",
            "profile",
            "configuration",
            "config",
            "startup",
            "shutdown",
            "logging",
            "tracing",
            "observability",
            "resilience",
            "运行",
            "健康检查",
            "启动",
            "关闭",
            "日志",
            "链路追踪",
        ],
    ) {
        push_unique(&mut tags, "runtime");
    }
    if contains_any(
        &text,
        &[
            "integration",
            "external service",
            "downstream",
            "webclient",
            "spring cloud",
            "cloud gateway",
            "gateway",
            "config server",
            "service discovery",
            "discovery",
            "eureka",
            "circuit breaker",
            "retry",
            "timeout",
            "resilience",
            "集成",
            "外部服务",
            "下游",
            "网关",
            "服务发现",
            "熔断",
            "重试",
            "超时",
        ],
    ) {
        push_unique(&mut tags, "integration");
    }
    if contains_any(
        &text,
        &[
            "migration",
            "migrate",
            "port from",
            "from django",
            "from drf",
            "迁移",
            "迁出",
            "迁到",
        ],
    ) {
        push_unique(&mut tags, "migration");
    }
    if contains_any(
        &text,
        &[
            "architecture",
            "clean architecture",
            "cqrs",
            "mediatr",
            "layer",
            "layers",
            "use case",
            "handler",
            "module boundary",
            "dependency injection",
            "provider",
            "providers",
            "架构",
            "分层",
            "用例",
            "依赖注入",
        ],
    ) {
        push_unique(&mut tags, "architecture");
    }
    if contains_any(
        &text,
        &[
            "generic",
            "generics",
            "type parameter",
            "template",
            "templates",
            "concept",
            "concepts",
            "constraints",
            "泛型",
            "模板",
            "类型参数",
        ],
    ) {
        push_unique(&mut tags, "generics");
    }
    if contains_any(
        &text,
        &[
            "analytics",
            "report",
            "window function",
            "row number",
            "rank",
            "dense rank",
            "lag",
            "lead",
            "running total",
            "cohort",
            "percentile",
            "top n",
            "报表",
            "统计",
            "排名",
            "窗口函数",
        ],
    ) {
        push_unique(&mut tags, "analytics");
    }
    if contains_any(
        &text,
        &[
            "memory",
            "allocation",
            "retain cycle",
            "arc",
            "instruments",
            "leak",
            "内存",
            "泄漏",
        ],
    ) {
        push_unique(&mut tags, "memory");
    }
    tags
}

fn signal_applies_to_task(signal: &CodeStackSignal, focus_tags: &[String]) -> bool {
    let roles = signal
        .roles
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>();
    let has_focus = |tag: &str| focus_tags.iter().any(|item| item == tag);
    match signal.language.as_deref() {
        Some("sql") => {
            has_focus("sql")
                || has_focus("persistence")
                || has_focus("performance")
                || has_focus("analytics")
        }
        Some("typescript") | Some("javascript") => {
            has_focus("frontend")
                || (roles.contains("backend") && (has_focus("api") || has_focus("backend")))
        }
        Some(_) => {
            if roles.contains("frontend") {
                has_focus("frontend")
            } else if roles.contains("backend") {
                has_focus("backend")
                    || has_focus("api")
                    || has_focus("persistence")
                    || has_focus("security")
                    || has_focus("async")
                    || has_focus("performance")
                    || has_focus("configuration")
                    || has_focus("runtime")
                    || has_focus("integration")
                    || has_focus("migration")
                    || has_focus("architecture")
            } else if roles.contains("persistence") {
                has_focus("persistence")
            } else {
                has_focus("api")
                    || has_focus("backend")
                    || has_focus("persistence")
                    || has_focus("security")
                    || has_focus("async")
                    || has_focus("performance")
                    || has_focus("configuration")
            }
        }
        None => false,
    }
}

fn reference_items_for_signal(signal: &CodeStackSignal, focus_tags: &[String]) -> BTreeSet<String> {
    let has_focus = |tag: &str| focus_tags.iter().any(|item| item == tag);
    let mut items = BTreeSet::new();
    match signal.language.as_deref() {
        Some("java") => {
            items.insert("core".to_string());
            if has_focus("api") || signal.frameworks.iter().any(|item| item == "spring_boot") {
                items.insert("spring".to_string());
            }
            if has_focus("persistence") {
                items.insert("persistence".to_string());
            }
            if has_focus("security") {
                items.insert("security".to_string());
            }
            if has_focus("async") {
                items.insert("reactive".to_string());
            }
            items.insert("testing".to_string());
        }
        Some("typescript") => {
            items.extend(["core", "types", "config", "patterns", "testing"].map(str::to_string));
            if has_focus("api") {
                items.insert("guards".to_string());
            }
        }
        Some("javascript") => {
            items.insert("core".to_string());
            items.insert("modules".to_string());
            if has_focus("async") || has_focus("api") {
                items.insert("async".to_string());
            }
            if signal.roles.iter().any(|role| role == "frontend") {
                items.insert("browser".to_string());
            } else {
                items.insert("node".to_string());
            }
            items.insert("testing".to_string());
        }
        Some("python") => {
            items.extend(["core", "typing", "packaging", "testing"].map(str::to_string));
            if has_focus("async") || signal.frameworks.iter().any(|item| item == "fastapi") {
                items.insert("async".to_string());
            }
        }
        Some("go") => {
            items.extend(["core", "interfaces", "structure", "testing"].map(str::to_string));
            if has_focus("async") || has_focus("api") {
                items.insert("concurrency".to_string());
            }
            if has_focus("generics") {
                items.insert("generics".to_string());
            }
        }
        Some("csharp") => {
            items.extend(["core", "testing"].map(str::to_string));
            if has_focus("api") || signal.frameworks.iter().any(|item| item == "aspnet_core") {
                items.insert("aspnet".to_string());
            }
            if has_focus("persistence") {
                items.insert("persistence".to_string());
            }
            if signal.frameworks.iter().any(|item| item == "blazor") {
                items.insert("blazor".to_string());
            }
            if has_focus("performance") {
                items.insert("performance".to_string());
            }
        }
        Some("cpp") => {
            items.extend(["core", "modern", "build", "testing"].map(str::to_string));
            if has_focus("generics") {
                items.insert("templates".to_string());
            }
            if has_focus("performance") {
                items.insert("performance".to_string());
            }
            if has_focus("async") {
                items.insert("concurrency".to_string());
            }
        }
        Some("kotlin") => {
            items.extend(["core", "testing"].map(str::to_string));
            if has_focus("async") {
                items.insert("coroutines".to_string());
            }
            if signal.frameworks.iter().any(|item| item == "ktor") || has_focus("api") {
                items.insert("ktor".to_string());
            }
            if signal.frameworks.iter().any(|item| item == "compose") || has_focus("frontend") {
                items.insert("compose".to_string());
            }
            if signal.frameworks.iter().any(|item| item == "kmp") {
                items.insert("multiplatform".to_string());
            }
        }
        Some("php") => {
            items.extend(["core", "testing"].map(str::to_string));
            if signal.frameworks.iter().any(|item| item == "laravel") {
                items.insert("laravel".to_string());
            }
            if signal.frameworks.iter().any(|item| item == "symfony") {
                items.insert("symfony".to_string());
            }
            if has_focus("async") {
                items.insert("async".to_string());
            }
        }
        Some("rust") => {
            items.extend(["core", "ownership", "traits", "errors", "testing"].map(str::to_string));
            if has_focus("async") || signal.frameworks.iter().any(|item| item == "tokio") {
                items.insert("async".to_string());
            }
        }
        Some("swift") => {
            items.extend(["core", "protocols", "testing"].map(str::to_string));
            if has_focus("async") {
                items.insert("concurrency".to_string());
            }
            if has_focus("performance") || has_focus("memory") {
                items.insert("memory".to_string());
            }
            if signal.frameworks.iter().any(|item| item == "swiftui") || has_focus("frontend") {
                items.insert("swiftui".to_string());
            }
        }
        Some("sql") => {
            if has_focus("persistence") {
                items.insert("schema".to_string());
                items.insert("dialects".to_string());
            }
            if has_focus("performance") {
                items.insert("optimization".to_string());
            }
            if has_focus("analytics") {
                items.insert("windows".to_string());
            }
            if has_focus("api") || has_focus("persistence") || has_focus("analytics") {
                items.insert("queries".to_string());
            }
        }
        _ => {}
    }
    items
}

fn backend_reference_items_for_signal(
    signal: &CodeStackSignal,
    focus_tags: &[String],
) -> BTreeMap<String, BTreeSet<String>> {
    let has_focus = |tag: &str| focus_tags.iter().any(|item| item == tag);
    let mut groups = BTreeMap::<String, BTreeSet<String>>::new();
    if signal.frameworks.iter().any(|item| item == "spring_boot") {
        let items = groups.entry("springboot".to_string()).or_default();
        items.insert("testing".to_string());
        if has_focus("api") {
            items.insert("web".to_string());
        }
        if has_focus("persistence") {
            items.insert("data".to_string());
        }
        if has_focus("security") {
            items.insert("security".to_string());
        }
        if has_focus("configuration") || has_focus("runtime") || has_focus("performance") {
            items.insert("runtime".to_string());
        }
        if has_focus("integration") || signal.frameworks.iter().any(|item| item == "spring_cloud") {
            items.insert("cloud".to_string());
        }
    }
    if signal.frameworks.iter().any(|item| item == "django") {
        let items = groups.entry("django".to_string()).or_default();
        items.insert("testing".to_string());
        if has_focus("api") {
            items.insert("views".to_string());
            items.insert("serializers".to_string());
        }
        if has_focus("persistence") || has_focus("performance") {
            items.insert("models".to_string());
        }
        if has_focus("security") {
            items.insert("security".to_string());
        }
    }
    if signal.frameworks.iter().any(|item| item == "fastapi") {
        let items = groups.entry("fastapi".to_string()).or_default();
        items.insert("testing".to_string());
        if has_focus("api") {
            items.insert("routing".to_string());
            items.insert("schemas".to_string());
        }
        if has_focus("persistence")
            || (has_focus("migration") && signal.frameworks.iter().any(|item| item == "sqlalchemy"))
        {
            items.insert("data".to_string());
        }
        if has_focus("security") {
            items.insert("security".to_string());
        }
        if has_focus("migration") {
            items.insert("migration".to_string());
        }
    }
    if signal.frameworks.iter().any(|item| item == "aspnet_core") {
        let items = groups.entry("aspnetcore".to_string()).or_default();
        items.insert("testing".to_string());
        if has_focus("api") {
            items.insert("minimal".to_string());
        }
        if has_focus("architecture") {
            items.insert("architecture".to_string());
        }
        if has_focus("persistence") {
            items.insert("data".to_string());
        }
        if has_focus("security") {
            items.insert("security".to_string());
        }
        if has_focus("configuration")
            || has_focus("runtime")
            || has_focus("performance")
            || has_focus("integration")
        {
            items.insert("runtime".to_string());
        }
    }
    if signal.frameworks.iter().any(|item| item == "nestjs") {
        let items = groups.entry("nestjs".to_string()).or_default();
        items.insert("testing".to_string());
        if has_focus("api") {
            items.insert("controllers".to_string());
            items.insert("dtos".to_string());
            items.insert("services".to_string());
        }
        if has_focus("backend") || has_focus("persistence") || has_focus("architecture") {
            items.insert("services".to_string());
        }
        if has_focus("security") {
            items.insert("security".to_string());
        }
        if has_focus("migration") {
            items.insert("migration".to_string());
        }
    }
    groups
}

fn reference_load_plan_item(group_key: &str, group: &str) -> ReferenceLoadPlanItem {
    if let Some((ref_prefix, path_group, label)) = match group_key {
        "springboot" => Some(("bk.spring", "springboot", "Spring Boot")),
        "django" => Some(("bk.django", "django", "Django")),
        "fastapi" => Some(("bk.fastapi", "fastapi", "FastAPI")),
        "aspnetcore" => Some(("bk.aspnet", "aspnetcore", "ASP.NET Core")),
        "nestjs" => Some(("bk.nest", "nestjs", "NestJS")),
        _ => None,
    } {
        return ReferenceLoadPlanItem {
            ref_id: format!("{ref_prefix}.{group}"),
            path: format!("tech/backend/{path_group}/{group}.md"),
            reason: format!("Selected {label} {group} framework quality reference for this task."),
        };
    }
    ReferenceLoadPlanItem {
        ref_id: format!("tech.code.{group_key}.{group}"),
        path: format!("tech/code/{group_key}/{group}.md"),
        reason: format!(
            "Selected {group_key}.{group} implementation quality reference for this task."
        ),
    }
}

fn task_is_frontend_task(task: &TaskDefinition) -> bool {
    task.frontend_experience_requirement.is_some()
        || matches!(
            task.task_kind,
            TaskKind::FrontendExperience | TaskKind::UiFlowIncrement
        )
        || task.implementation_actions.iter().any(|action| {
            matches!(
                action,
                ImplementationAction::CreateOrUpdateUiFlow
                    | ImplementationAction::ImplementFrontendExperienceContract
                    | ImplementationAction::CreateEntityAdminPage
            )
        })
}

fn task_is_backend_task(task: &TaskDefinition) -> bool {
    if task_is_frontend_task(task) {
        return false;
    }
    matches!(
        task.task_kind,
        TaskKind::FeatureIncrement
            | TaskKind::DataModelIncrement
            | TaskKind::InterfaceIncrement
            | TaskKind::IntegrationIncrement
            | TaskKind::RefactorSupport
    ) || task.implementation_actions.iter().any(|action| {
        matches!(
            action,
            ImplementationAction::CreateOrUpdateEntity
                | ImplementationAction::CreateOrUpdatePersistence
                | ImplementationAction::CreateOrUpdateInterface
                | ImplementationAction::CreateOrUpdateStateMachine
                | ImplementationAction::CreateOrUpdateBusinessRule
                | ImplementationAction::AddReferenceField
                | ImplementationAction::ValidateReferenceFormat
                | ImplementationAction::WireReferenceInApiOrUi
                | ImplementationAction::CreateEntityCrud
                | ImplementationAction::CreateEntityRepository
                | ImplementationAction::ImplementEntityLifecycle
                | ImplementationAction::RefactorSupportingCode
        )
    })
}

fn task_owns_api_contract(task: &TaskDefinition) -> bool {
    matches!(task.task_kind, TaskKind::InterfaceIncrement)
        || task.implementation_actions.iter().any(|action| {
            matches!(
                action,
                ImplementationAction::CreateOrUpdateInterface
                    | ImplementationAction::CreateEntityCrud
                    | ImplementationAction::WireReferenceInApiOrUi
            )
        })
}

fn task_owns_persistence(task: &TaskDefinition) -> bool {
    matches!(task.task_kind, TaskKind::DataModelIncrement)
        || task.implementation_actions.iter().any(|action| {
            matches!(
                action,
                ImplementationAction::CreateOrUpdateEntity
                    | ImplementationAction::CreateOrUpdatePersistence
                    | ImplementationAction::CreateEntityMigration
                    | ImplementationAction::CreateEntityRepository
                    | ImplementationAction::CreateEntityCrud
            )
        })
}

fn dedupe_signals(signals: Vec<CodeStackSignal>) -> Vec<CodeStackSignal> {
    let mut seen = BTreeSet::new();
    signals
        .into_iter()
        .filter(|signal| {
            seen.insert((
                signal.source_path.clone(),
                signal.raw_selection.to_ascii_lowercase(),
                signal.language.clone(),
            ))
        })
        .collect()
}

fn selection_is_absent(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "none",
        "not needed",
        "not applicable",
        "n/a",
        "no backend",
        "no frontend",
        "no persistence",
        "no database",
        "无",
        "不需要",
        "无需",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

fn normalized(value: &str) -> String {
    format!(
        " {} ",
        value
            .to_ascii_lowercase()
            .replace(['/', ',', '-', '_'], " ")
    )
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

fn push_if_contains(haystack: &str, output: &mut Vec<String>, value: &str, needles: &[&str]) {
    if contains_any(haystack, needles) {
        push_unique(output, value);
    }
}

fn push_backend_unless_persistence_track(roles: &mut Vec<String>) {
    if !roles
        .iter()
        .any(|role| role == "persistence" || role == "database")
    {
        push_unique(roles, "backend");
    }
}

fn push_unique(output: &mut Vec<String>, value: &str) {
    if !output.iter().any(|item| item == value) {
        output.push(value.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ConfidenceLevel, ProjectKind, TaskArtifactRefs, TaskWriteBoundary,
        TechnicalBaselineApproval, TechnicalBaselineApprovalType, TechnicalBaselineScope,
        TechnicalBaselineSource, TechnicalBaselineStatus, VerificationIntent,
    };

    fn baseline(stack: Value) -> TechnicalBaselineContract {
        TechnicalBaselineContract {
            schema_version: "1.0".to_string(),
            technical_baseline_id: "tb-1".to_string(),
            delivery_id: "delivery-1".to_string(),
            phase_id: "phase-1".to_string(),
            status: TechnicalBaselineStatus::Confirmed,
            source: TechnicalBaselineSource::AgentRecommendedForGreenfield,
            project_kind: ProjectKind::Greenfield,
            scope: TechnicalBaselineScope::Project,
            stack,
            constraints: vec![],
            evidence: vec![],
            approval: TechnicalBaselineApproval {
                r#type: TechnicalBaselineApprovalType::UserConfirmed,
                confirmed_at: Some("2026-07-06T00:00:00Z".to_string()),
                reason: Some("confirmed".to_string()),
            },
            confidence: ConfidenceLevel::High,
            requires_user_confirmation: None,
            reasoning_summary: vec![],
            alternatives: vec![],
            created_at: "2026-07-06T00:00:00Z".to_string(),
            updated_at: "2026-07-06T00:00:00Z".to_string(),
        }
    }

    fn task(kind: TaskKind, actions: Vec<ImplementationAction>) -> TaskDefinition {
        TaskDefinition {
            task_id: "task-1".to_string(),
            group_id: "group-1".to_string(),
            title: "Implement purchase request persistence".to_string(),
            task_kind: kind,
            implementation_actions: actions,
            objective: "Create backend code and tests.".to_string(),
            depends_on: vec![],
            scope_refs: vec![],
            acceptance_refs: vec![],
            requirement_detail_refs: vec![],
            write_boundary: TaskWriteBoundary {
                forbidden_paths: vec![".loom".to_string()],
                artifact_refs: TaskArtifactRefs::default(),
            },
            verification_intents: vec![VerificationIntent {
                verification_id: "verify-1".to_string(),
                acceptance_refs: vec![],
                requirement_detail_refs: vec![],
                behavior: "test".to_string(),
                preferred_evidence: vec![],
                acceptable_evidence: vec![],
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

    #[test]
    fn maps_spring_jpa_and_postgres_to_task_scoped_refs() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Java + Spring Boot"},
                "dataAccess": {"selection": "Spring Data JPA"},
                "persistence": {"selection": "PostgreSQL"}
            }
        }));
        let task = task(
            TaskKind::DataModelIncrement,
            vec![
                ImplementationAction::CreateOrUpdateEntity,
                ImplementationAction::CreateEntityRepository,
            ],
        );
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups["java"].contains(&"persistence".to_string()));
        assert!(selection.reference_groups["java"].contains(&"testing".to_string()));
        assert!(selection.reference_groups["springboot"].contains(&"data".to_string()));
        assert!(selection.reference_groups["springboot"].contains(&"testing".to_string()));
        assert!(selection.reference_groups["sql"].contains(&"schema".to_string()));
        assert!(selection.reference_groups["sql"].contains(&"dialects".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan
            .iter()
            .any(|item| item.path == "tech/code/java/persistence.md"));
        assert!(load_plan
            .iter()
            .any(|item| item.path == "tech/code/common.md"));
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "bk.spring.data" && item.path == "tech/backend/springboot/data.md"
        }));
        assert!(!load_plan
            .iter()
            .any(|item| item.path == "tech/code/java/security.md"));
    }

    #[test]
    fn maps_react_typescript_only_for_frontend_task() {
        let baseline = baseline(json!({
            "tracks": {
                "web": {"selection": "React + TypeScript"},
                "backend": {"selection": "Java + Spring Boot"}
            }
        }));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups.contains_key("typescript"));
        assert!(!selection.reference_groups.contains_key("java"));
    }

    #[test]
    fn maps_dotnet_go_and_python_backend_signals() {
        for (raw, language) in [
            (".NET + ASP.NET Core", "csharp"),
            ("Go + Gin", "go"),
            ("Python + FastAPI", "python"),
        ] {
            let baseline = baseline(json!({"tracks": {"backend": {"selection": raw}}}));
            let task = task(
                TaskKind::InterfaceIncrement,
                vec![ImplementationAction::CreateOrUpdateInterface],
            );
            let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
            assert!(
                selection.reference_groups.contains_key(language),
                "{raw} should map to {language}"
            );
        }
    }

    #[test]
    fn maps_cpp_without_losing_plus_signs() {
        let baseline = baseline(json!({"tracks": {"backend": {"selection": "C++20 + CMake"}}}));
        let task = task(
            TaskKind::FeatureIncrement,
            vec![ImplementationAction::CreateOrUpdateBusinessRule],
        );
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups.contains_key("cpp"));
        assert!(selection.reference_groups["cpp"].contains(&"build".to_string()));
    }

    #[test]
    fn maps_task_specific_language_reference_groups() {
        let cpp_baseline = baseline(json!({"tracks": {"backend": {"selection": "C++20 + CMake"}}}));
        let mut cpp_task = task(
            TaskKind::FeatureIncrement,
            vec![ImplementationAction::CreateOrUpdateBusinessRule],
        );
        cpp_task.objective = "Implement a template-based generic rules registry.".to_string();
        let cpp_selection = code_reference_selection_for_task(&cpp_baseline, &cpp_task).unwrap();
        assert!(cpp_selection.reference_groups["cpp"].contains(&"templates".to_string()));

        let go_baseline = baseline(json!({"tracks": {"backend": {"selection": "Go + Gin"}}}));
        let mut go_task = task(
            TaskKind::FeatureIncrement,
            vec![ImplementationAction::CreateOrUpdateBusinessRule],
        );
        go_task.objective = "Create generic collection helpers with type parameters.".to_string();
        let go_selection = code_reference_selection_for_task(&go_baseline, &go_task).unwrap();
        assert!(go_selection.reference_groups["go"].contains(&"generics".to_string()));

        let swift_baseline = baseline(json!({"tracks": {"app": {"selection": "Swift + SwiftUI"}}}));
        let mut swift_task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        swift_task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        swift_task.objective =
            "Fix memory leak and retain cycle in SwiftUI detail flow.".to_string();
        let swift_selection =
            code_reference_selection_for_task(&swift_baseline, &swift_task).unwrap();
        assert!(swift_selection.reference_groups["swift"].contains(&"memory".to_string()));

        let sql_baseline =
            baseline(json!({"tracks": {"persistence": {"selection": "PostgreSQL"}}}));
        let mut sql_task = task(
            TaskKind::FeatureIncrement,
            vec![ImplementationAction::CreateOrUpdateBusinessRule],
        );
        sql_task.objective =
            "Add reporting query with window function ranking and running total.".to_string();
        let sql_selection = code_reference_selection_for_task(&sql_baseline, &sql_task).unwrap();
        assert!(sql_selection.reference_groups["sql"].contains(&"queries".to_string()));
        assert!(sql_selection.reference_groups["sql"].contains(&"windows".to_string()));
    }

    #[test]
    fn java_security_reference_is_task_scoped() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Java + Spring Boot"},
                "dataAccess": {"selection": "Spring Data JPA"}
            }
        }));
        let mut task = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::CreateOrUpdateInterface],
        );
        task.objective = "Add JWT login endpoint and role-based permission checks.".to_string();
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups["java"].contains(&"security".to_string()));
        assert!(selection.reference_groups["springboot"].contains(&"security".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan
            .iter()
            .any(|item| item.ref_id == "tech.code.common"));
        assert!(load_plan
            .iter()
            .any(|item| item.path == "tech/code/java/security.md"));
        assert!(load_plan
            .iter()
            .any(|item| item.ref_id == "tech.code.java.security"));
        assert!(load_plan
            .iter()
            .any(|item| item.ref_id == "bk.spring.security"));
    }

    #[test]
    fn spring_boot_api_task_loads_web_without_persistence_or_sql_references() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Java + Spring Boot + Spring Data JPA"},
                "persistence": {"selection": "PostgreSQL"}
            }
        }));
        let task = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::CreateOrUpdateInterface],
        );
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups["java"].contains(&"spring".to_string()));
        assert!(selection.reference_groups["springboot"].contains(&"web".to_string()));
        assert!(selection.reference_groups["springboot"].contains(&"testing".to_string()));
        assert!(!selection.reference_groups["springboot"].contains(&"data".to_string()));
        assert!(!selection.reference_groups["java"].contains(&"persistence".to_string()));
        assert!(!selection.reference_groups.contains_key("sql"));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "bk.spring.web" && item.path == "tech/backend/springboot/web.md"
        }));
        assert!(!load_plan
            .iter()
            .any(|item| item.path == "tech/code/java/persistence.md"));
        assert!(!load_plan
            .iter()
            .any(|item| item.path.starts_with("tech/code/sql/")));
        assert!(!load_plan
            .iter()
            .any(|item| item.path == "tech/backend/springboot/data.md"));
    }

    #[test]
    fn spring_boot_runtime_reference_is_configuration_scoped() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Java + Spring Boot"}
            }
        }));
        let mut task = task(
            TaskKind::ConfigurationSupport,
            vec![ImplementationAction::RefactorSupportingCode],
        );
        task.objective =
            "Add actuator health startup logging and typed runtime configuration.".to_string();
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups["springboot"].contains(&"runtime".to_string()));
        assert!(selection.reference_groups["springboot"].contains(&"testing".to_string()));
        assert!(!selection.reference_groups["springboot"].contains(&"web".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "bk.spring.runtime" && item.path == "tech/backend/springboot/runtime.md"
        }));
    }

    #[test]
    fn spring_boot_cloud_reference_is_integration_scoped() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Java + Spring Boot + Spring Cloud"}
            }
        }));
        let mut task = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::CreateOrUpdateInterface],
        );
        task.objective =
            "Add WebClient downstream integration with timeout retry and gateway fallback."
                .to_string();
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups["springboot"].contains(&"cloud".to_string()));
        assert!(selection.reference_groups["springboot"].contains(&"testing".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "bk.spring.cloud" && item.path == "tech/backend/springboot/cloud.md"
        }));
    }

    #[test]
    fn django_api_task_loads_views_and_serializers_without_models() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Python + Django + Django REST Framework"}
            }
        }));
        let task = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::CreateOrUpdateInterface],
        );
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups["python"].contains(&"core".to_string()));
        assert!(selection.reference_groups["django"].contains(&"views".to_string()));
        assert!(selection.reference_groups["django"].contains(&"serializers".to_string()));
        assert!(selection.reference_groups["django"].contains(&"testing".to_string()));
        assert!(!selection.reference_groups["django"].contains(&"models".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "bk.django.views" && item.path == "tech/backend/django/views.md"
        }));
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "bk.django.serializers"
                && item.path == "tech/backend/django/serializers.md"
        }));
        assert!(!load_plan
            .iter()
            .any(|item| item.path == "tech/backend/django/models.md"));
    }

    #[test]
    fn django_persistence_task_loads_models_without_security() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Python + Django"},
                "persistence": {"selection": "PostgreSQL"}
            }
        }));
        let task = task(
            TaskKind::DataModelIncrement,
            vec![
                ImplementationAction::CreateOrUpdateEntity,
                ImplementationAction::CreateEntityMigration,
            ],
        );
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups["django"].contains(&"models".to_string()));
        assert!(selection.reference_groups["django"].contains(&"testing".to_string()));
        assert!(!selection.reference_groups["django"].contains(&"security".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "bk.django.models" && item.path == "tech/backend/django/models.md"
        }));
    }

    #[test]
    fn fastapi_api_task_loads_routing_and_schemas_without_data() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Python + FastAPI + SQLAlchemy"}
            }
        }));
        let task = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::CreateOrUpdateInterface],
        );
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups["python"].contains(&"async".to_string()));
        assert!(selection.reference_groups["fastapi"].contains(&"routing".to_string()));
        assert!(selection.reference_groups["fastapi"].contains(&"schemas".to_string()));
        assert!(selection.reference_groups["fastapi"].contains(&"testing".to_string()));
        assert!(!selection.reference_groups["fastapi"].contains(&"data".to_string()));
        assert!(!selection.reference_groups["fastapi"].contains(&"migration".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "bk.fastapi.routing" && item.path == "tech/backend/fastapi/routing.md"
        }));
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "bk.fastapi.schemas" && item.path == "tech/backend/fastapi/schemas.md"
        }));
        assert!(!load_plan
            .iter()
            .any(|item| item.path == "tech/backend/fastapi/data.md"));
    }

    #[test]
    fn fastapi_migration_reference_is_explicitly_scoped() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Python + FastAPI + SQLAlchemy"}
            }
        }));
        let mut task = task(
            TaskKind::IntegrationIncrement,
            vec![ImplementationAction::RefactorSupportingCode],
        );
        task.objective = "Migrate the existing Django REST Framework order endpoint to FastAPI while preserving response behavior.".to_string();
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups["fastapi"].contains(&"migration".to_string()));
        assert!(selection.reference_groups["fastapi"].contains(&"data".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "bk.fastapi.migration"
                && item.path == "tech/backend/fastapi/migration.md"
        }));
    }

    #[test]
    fn aspnet_core_api_task_loads_minimal_without_data_or_architecture() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": ".NET 8 + ASP.NET Core + Entity Framework Core"}
            }
        }));
        let task = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::CreateOrUpdateInterface],
        );
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups["csharp"].contains(&"aspnet".to_string()));
        assert!(selection.reference_groups["aspnetcore"].contains(&"minimal".to_string()));
        assert!(selection.reference_groups["aspnetcore"].contains(&"testing".to_string()));
        assert!(!selection.reference_groups["aspnetcore"].contains(&"data".to_string()));
        assert!(!selection.reference_groups["aspnetcore"].contains(&"architecture".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "bk.aspnet.minimal" && item.path == "tech/backend/aspnetcore/minimal.md"
        }));
        assert!(!load_plan
            .iter()
            .any(|item| item.path == "tech/backend/aspnetcore/data.md"));
    }

    #[test]
    fn aspnet_core_architecture_reference_is_task_scoped() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": ".NET 8 + ASP.NET Core"}
            }
        }));
        let mut task = task(
            TaskKind::RefactorSupport,
            vec![ImplementationAction::RefactorSupportingCode],
        );
        task.objective =
            "Introduce clean architecture CQRS handlers and dependency injection boundaries."
                .to_string();
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups["aspnetcore"].contains(&"architecture".to_string()));
        assert!(selection.reference_groups["aspnetcore"].contains(&"testing".to_string()));
        assert!(!selection.reference_groups["aspnetcore"].contains(&"minimal".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "bk.aspnet.architecture"
                && item.path == "tech/backend/aspnetcore/architecture.md"
        }));
    }

    #[test]
    fn nestjs_api_task_loads_controller_dto_service_without_migration() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "TypeScript + NestJS + Prisma"}
            }
        }));
        let task = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::CreateOrUpdateInterface],
        );
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups["typescript"].contains(&"core".to_string()));
        assert!(selection.reference_groups["nestjs"].contains(&"controllers".to_string()));
        assert!(selection.reference_groups["nestjs"].contains(&"dtos".to_string()));
        assert!(selection.reference_groups["nestjs"].contains(&"services".to_string()));
        assert!(selection.reference_groups["nestjs"].contains(&"testing".to_string()));
        assert!(!selection.reference_groups["nestjs"].contains(&"migration".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "bk.nest.controllers"
                && item.path == "tech/backend/nestjs/controllers.md"
        }));
        assert!(load_plan.iter().any(
            |item| item.ref_id == "bk.nest.dtos" && item.path == "tech/backend/nestjs/dtos.md"
        ));
    }

    #[test]
    fn nestjs_express_migration_reference_is_explicitly_scoped() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "TypeScript + NestJS"}
            }
        }));
        let mut task = task(
            TaskKind::IntegrationIncrement,
            vec![ImplementationAction::RefactorSupportingCode],
        );
        task.objective =
            "Migrate existing Express routers and middleware into NestJS modules and controllers."
                .to_string();
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups["nestjs"].contains(&"migration".to_string()));
        assert!(selection.reference_groups["nestjs"].contains(&"services".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "bk.nest.migration" && item.path == "tech/backend/nestjs/migration.md"
        }));
    }

    #[test]
    fn maps_react_javascript_without_assuming_typescript() {
        let baseline = baseline(json!({"tracks": {"web": {"selection": "React + JavaScript"}}}));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups.contains_key("javascript"));
        assert!(!selection.reference_groups.contains_key("typescript"));
    }

    #[test]
    fn framework_only_react_signal_stays_unmapped() {
        let baseline = baseline(json!({"tracks": {"web": {"selection": "React"}}}));
        let seed = build_code_quality_seed(&baseline);
        assert_eq!(seed["required"], false);
        assert_eq!(seed["unmappedSignals"][0]["rawSelection"], "React");
    }

    #[test]
    fn unknown_stack_is_unmapped_without_blocking() {
        let baseline = baseline(json!({"tracks": {"backend": {"selection": "MysteryFlow"}}}));
        let seed = build_code_quality_seed(&baseline);
        assert_eq!(seed["required"], false);
        assert_eq!(seed["unmappedSignals"][0]["confidence"], "low");
    }
}
