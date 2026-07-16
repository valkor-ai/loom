use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

use crate::{
    CodePackageNamingPolicy, ImplementationAction, ReferenceLoadPlanItem, TaskDefinition, TaskKind,
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

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CodeReferenceTaskContext {
    pub application_architecture: bool,
    pub security: bool,
    pub async_processing: bool,
    pub integration: bool,
    pub resilience: bool,
    pub observability: bool,
}

pub fn build_code_quality_seed(baseline: &TechnicalBaselineContract) -> Value {
    let signals = code_stack_signals_from_baseline(&baseline.stack);
    if signals.is_empty() {
        return Value::Null;
    }
    let required = signals.iter().any(|signal| !signal_is_unmapped(signal));
    json!({
        "required": required,
        "qualityLevel": "production_code_implementation",
        "codeStackSignals": signals,
        "unmappedSignals": signals
            .iter()
            .filter(|signal| signal_is_unmapped(signal))
            .collect::<Vec<_>>()
    })
}

pub fn code_quality_enum_refs() -> Value {
    json!({
        "knownReferenceGroups": {
            "code": {
                "java": ["core", "spring", "persistence", "security", "reactive", "testing"],
                "springboot": ["web", "data", "security", "testing", "runtime", "async", "cache", "integration", "resilience", "cloud", "observability"],
                "django": ["models", "serializers", "views", "security", "testing"],
                "fastapi": ["schemas", "data", "routing", "security", "testing", "migration"],
                "aspnetcore": ["minimal", "architecture", "data", "security", "testing", "runtime"],
                "nestjs": ["controllers", "dtos", "services", "security", "testing", "migration"],
                "react": ["core", "hooks", "state", "performance", "testing", "server-components", "react19", "migration"],
                "nextjs": ["core", "app-router", "data", "actions", "server-components", "runtime", "testing"],
                "vue": ["core", "components", "state", "typescript", "nuxt", "build", "mobile", "testing"],
                "angular": ["core", "components", "routing", "rxjs", "ngrx", "testing"],
                "reactnative": ["core", "structure", "navigation", "platform", "lists", "storage", "testing"],
                "flutter": ["core", "structure", "widgets", "navigation", "riverpod", "bloc", "performance", "testing"],
                "typescript": ["core", "types", "guards", "config", "patterns", "testing"],
                "javascript": ["core", "async", "modules", "node", "browser", "testing"],
                "python": ["core", "typing", "async", "packaging", "testing"],
                "go": ["core", "concurrency", "interfaces", "structure", "generics", "testing"],
                "csharp": ["core", "modern", "persistence", "blazor", "performance", "testing"],
                "cpp": ["core", "modern", "templates", "performance", "concurrency", "build", "testing"],
                "kotlin": ["core", "coroutines", "ktor", "compose", "multiplatform", "dsl", "testing"],
                "php": ["core", "modern", "laravel", "symfony", "async", "testing"],
                "rust": ["core", "ownership", "traits", "errors", "async", "testing"],
                "swift": ["core", "swiftui", "concurrency", "protocols", "memory", "testing"],
                "sql": [
                    "schema", "queries", "dialects", "optimization", "windows",
                    "mysql.schema", "mysql.queries", "mysql.transactions",
                    "postgresql.schema", "postgresql.queries", "postgresql.transactions",
                    "sqlserver.schema", "sqlserver.queries", "sqlserver.transactions",
                    "oracle.schema", "oracle.queries", "oracle.transactions"
                ]
            }
        },
        "focusTag": ["api", "api_client", "frontend", "persistence", "security", "async", "reactive", "cache", "performance", "configuration", "runtime", "integration", "resilience", "observability", "cloud", "migration", "architecture", "testing", "sql", "sql_schema", "sql_query", "sql_transaction", "sql_test", "generics", "analytics", "memory", "hooks", "state", "server_components", "react19", "app_router", "server_actions", "data_fetching", "build_tooling", "mobile", "nuxt", "routing", "rxjs", "ngrx", "riverpod", "bloc", "list_performance", "storage"],
        "confidence": ["high", "medium", "low"]
    })
}

pub fn code_reference_selection_for_task(
    baseline: &TechnicalBaselineContract,
    task: &TaskDefinition,
) -> Option<CodeReferenceSelection> {
    code_reference_selection_for_task_with_context(
        baseline,
        task,
        &CodeReferenceTaskContext::default(),
    )
}

pub fn code_reference_selection_for_task_with_context(
    baseline: &TechnicalBaselineContract,
    task: &TaskDefinition,
    context: &CodeReferenceTaskContext,
) -> Option<CodeReferenceSelection> {
    let signals = code_stack_signals_from_baseline(&baseline.stack);
    if signals.is_empty() {
        return None;
    }
    let stack_frameworks = signals
        .iter()
        .flat_map(|signal| signal.frameworks.iter().cloned())
        .collect::<BTreeSet<_>>();
    let mut focus_tags = task_focus_tags(task);
    extend_focus_tags_from_context(&mut focus_tags, context);
    let mut selected_signals = Vec::new();
    let mut reference_groups = BTreeMap::<String, BTreeSet<String>>::new();
    let mut unmapped_signals = Vec::new();

    for signal in signals {
        if !signal_applies_to_task(&signal, &focus_tags) {
            if signal.language.is_none() {
                unmapped_signals.push(signal);
            }
            continue;
        }
        let items = if signal.language.is_some() {
            reference_items_for_signal(&signal, &focus_tags, task, &stack_frameworks)
        } else {
            BTreeSet::new()
        };
        let backend_items =
            backend_reference_items_for_signal(&signal, &stack_frameworks, task, context);
        let frontend_items = frontend_reference_items_for_signal(&signal, &focus_tags, task);
        if !items.is_empty() || !backend_items.is_empty() || !frontend_items.is_empty() {
            selected_signals.push(signal.clone());
            if let Some(language) = &signal.language {
                reference_groups
                    .entry(language.clone())
                    .or_default()
                    .extend(items);
            }
            for (framework, items) in backend_items {
                reference_groups.entry(framework).or_default().extend(items);
            }
            for (framework, items) in frontend_items {
                reference_groups.entry(framework).or_default().extend(items);
            }
        } else if signal.language.is_none() {
            unmapped_signals.push(signal);
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
        push_frontend_frameworks_from_haystack(&haystack, &mut frameworks);
        push_if_contains(&haystack, &mut frameworks, "node", &["node"]);
        push_if_contains(&haystack, &mut frameworks, "nestjs", &["nestjs", "nest js"]);
        if selection_mentions_frontend_framework(&haystack) {
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
        push_frontend_frameworks_from_haystack(&haystack, &mut frameworks);
        if selection_mentions_frontend_framework(&haystack) {
            push_unique(&mut roles, "frontend");
        }
        if contains_any(
            &haystack,
            &["node", "express", "nestjs", "nest js", "fastify"],
        ) {
            push_unique(&mut roles, "backend");
        }
    } else if contains_any(&haystack, &["java", "spring", "jpa", "hibernate"])
        && !contains_any(&haystack, &["kotlin", "ktor", "android", "kmp"])
    {
        language = Some("java".to_string());
        push_spring_frameworks_from_haystack(&haystack, &mut frameworks);
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
            "minimal_api",
            &["minimal api", "minimal-api"],
        );
        push_if_contains(
            &haystack,
            &mut frameworks,
            "entity_framework",
            &["entity framework", "ef core"],
        );
        push_if_contains(&haystack, &mut frameworks, "blazor", &["blazor"]);
        push_backend_unless_persistence_track(&mut roles);
        if frameworks.iter().any(|item| item == "blazor") {
            push_unique(&mut roles, "frontend");
        }
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
    } else if selection_mentions_flutter_framework(&haystack) {
        push_frontend_frameworks_from_haystack(&haystack, &mut frameworks);
        push_unique(&mut roles, "frontend");
    } else if contains_any(&haystack, &["kotlin", "ktor", "android", "compose", "kmp"]) {
        language = Some("kotlin".to_string());
        if contains_any(&haystack, &["ktor client", "ktor-client"]) {
            push_unique(&mut frameworks, "ktor_client");
        } else {
            push_if_contains(&haystack, &mut frameworks, "ktor", &["ktor"]);
        }
        push_if_contains(&haystack, &mut frameworks, "compose", &["compose"]);
        push_if_contains(&haystack, &mut frameworks, "kmp", &["kmp", "multiplatform"]);
        push_spring_frameworks_from_haystack(&haystack, &mut frameworks);
        if frameworks.iter().any(|framework| {
            matches!(
                framework.as_str(),
                "spring_boot"
                    | "spring_framework"
                    | "spring_cloud"
                    | "spring_data_jpa"
                    | "jpa_orm"
                    | "spring_webflux"
                    | "project_reactor"
                    | "r2dbc"
            )
        }) {
            push_backend_unless_persistence_track(&mut roles);
        }
    } else if contains_any(&haystack, &["php", "laravel", "symfony"]) {
        language = Some("php".to_string());
        push_if_contains(&haystack, &mut frameworks, "laravel", &["laravel"]);
        push_if_contains(&haystack, &mut frameworks, "symfony", &["symfony"]);
        push_if_contains(&haystack, &mut frameworks, "swoole", &["swoole"]);
        push_if_contains(
            &haystack,
            &mut frameworks,
            "reactphp",
            &["reactphp", "react php"],
        );
        push_if_contains(&haystack, &mut frameworks, "amphp", &["amphp", "amp php"]);
        push_if_contains(&haystack, &mut frameworks, "fibers", &["fiber", "fibers"]);
        push_unique(&mut roles, "backend");
    } else if contains_any(&haystack, &["swift", "swiftui", "vapor"]) {
        language = Some("swift".to_string());
        push_if_contains(&haystack, &mut frameworks, "swiftui", &["swiftui"]);
        push_if_contains(&haystack, &mut frameworks, "vapor", &["vapor"]);
    } else if contains_any(&haystack, &["c++", "cpp", "cmake", "clang", "gcc"]) {
        language = Some("cpp".to_string());
        push_if_contains(&haystack, &mut frameworks, "cmake", &["cmake"]);
    } else if selection_mentions_frontend_framework(&haystack) {
        push_frontend_frameworks_from_haystack(&haystack, &mut frameworks);
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
        push_if_contains(&haystack, &mut dialects, "mariadb", &["mariadb"]);
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

    let mapped = language.is_some() || !frameworks.is_empty() || !dialects.is_empty();
    let confidence = if mapped { "high" } else { "low" }.to_string();
    let reason = if language.is_some() {
        "Mapped language from confirmed TechnicalBaseline stack selection.".to_string()
    } else if !frameworks.is_empty() {
        "Mapped framework from confirmed TechnicalBaseline stack selection.".to_string()
    } else if !dialects.is_empty() {
        "Mapped storage dialect from confirmed TechnicalBaseline stack selection.".to_string()
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

fn signal_is_unmapped(signal: &CodeStackSignal) -> bool {
    signal.language.is_none() && signal.frameworks.is_empty() && signal.dialects.is_empty()
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
    if task_owns_test_implementation(task) {
        push_unique(&mut tags, "testing");
    }
    if task_is_frontend_task(task) {
        push_unique(&mut tags, "frontend");
    }
    if task_owns_api_contract(task) {
        push_unique(&mut tags, "api");
    }
    if task_uses_api_client_binding(task) {
        push_unique(&mut tags, "api_client");
        push_unique(&mut tags, "data_fetching");
    }
    if task_owns_persistence(task) {
        push_unique(&mut tags, "persistence");
        push_unique(&mut tags, "sql");
    }
    if task_owns_sql_schema(task) {
        push_unique(&mut tags, "sql_schema");
    }
    if task_owns_sql_query(task) {
        push_unique(&mut tags, "sql_query");
    }
    if task_owns_sql_transaction(task) {
        push_unique(&mut tags, "sql_transaction");
    }
    if task_owns_sql_performance(task) {
        push_unique(&mut tags, "performance");
    }
    if task_owns_sql_analytics(task) {
        push_unique(&mut tags, "analytics");
    }
    if task_owns_sql_tests(task) {
        push_unique(&mut tags, "sql_test");
        push_unique(&mut tags, "sql");
    }
    if task_is_backend_task(task) {
        push_unique(&mut tags, "backend");
    }
    if matches!(task.task_kind, TaskKind::ConfigurationSupport) {
        push_unique(&mut tags, "configuration");
    }
    for action in &task.implementation_actions {
        match action {
            ImplementationAction::AddOrUpdateConfig => push_unique(&mut tags, "configuration"),
            ImplementationAction::CreateOrUpdateFrontendNavigation => {
                push_unique(&mut tags, "routing")
            }
            ImplementationAction::ImplementReactiveClientFlow => push_unique(&mut tags, "async"),
            ImplementationAction::ImplementSharedClientState => push_unique(&mut tags, "state"),
            ImplementationAction::OptimizeFrontendPerformance => {
                push_unique(&mut tags, "performance")
            }
            ImplementationAction::ImplementServerRenderedComponent => {
                push_unique(&mut tags, "server_components")
            }
            ImplementationAction::ImplementServerMutation => {
                push_unique(&mut tags, "server_actions")
            }
            ImplementationAction::ImplementAuthenticationOrAuthorization => {
                push_unique(&mut tags, "security")
            }
            ImplementationAction::ImplementAsyncProcessing => push_unique(&mut tags, "async"),
            ImplementationAction::ImplementCachePolicy => push_unique(&mut tags, "cache"),
            ImplementationAction::ImplementExternalServiceIntegration => {
                push_unique(&mut tags, "integration")
            }
            ImplementationAction::ImplementResiliencePolicy => push_unique(&mut tags, "resilience"),
            ImplementationAction::ConfigureServiceRoutingOrDiscovery => {
                push_unique(&mut tags, "cloud")
            }
            ImplementationAction::ImplementObservability => push_unique(&mut tags, "observability"),
            ImplementationAction::MigrateFrameworkImplementation => {
                push_unique(&mut tags, "migration")
            }
            ImplementationAction::ImplementRuntimeDeliveryContract => {
                push_unique(&mut tags, "runtime")
            }
            _ => {}
        }
    }
    let text = normalized(&format!(
        "{} {} {:?}",
        task.title, task.objective, task.implementation_actions
    ));
    if contains_any(
        &text,
        &[
            "state",
            "store",
            "context",
            "reducer",
            "zustand",
            "redux",
            "tanstack",
            "query client",
            "selected record",
            "form draft",
            "状态",
            "表单草稿",
            "选中记录",
        ],
    ) {
        push_unique(&mut tags, "state");
    }
    if contains_any(
        &text,
        &[
            "hook",
            "hooks",
            "useeffect",
            "usememo",
            "usecallback",
            "useref",
            "custom hook",
            "debounce",
            "localstorage",
            "effect cleanup",
            "钩子",
            "副作用",
            "防抖",
        ],
    ) {
        push_unique(&mut tags, "hooks");
    }
    if contains_any(
        &text,
        &[
            "server component",
            "server components",
            "react server component",
            "rsc",
            "suspense",
            "streaming",
            "use client",
            "hydration",
            "服务端组件",
            "服务端渲染",
            "水合",
        ],
    ) {
        push_unique(&mut tags, "server_components");
    }
    if contains_any(
        &text,
        &[
            "react 19",
            "useactionstate",
            "useformstatus",
            "useoptimistic",
            "use()",
            "ref as prop",
            "action state",
            "optimistic",
            "乐观更新",
        ],
    ) {
        push_unique(&mut tags, "react19");
    }
    if contains_any(
        &text,
        &[
            "app router",
            "layout.tsx",
            "page.tsx",
            "loading.tsx",
            "error.tsx",
            "not-found.tsx",
            "route group",
            "route handler",
            "generate metadata",
            "metadata api",
            "dynamic route",
            "parallel route",
            "intercepting route",
        ],
    ) {
        push_unique(&mut tags, "app_router");
    }
    if contains_any(
        &text,
        &[
            "server action",
            "server actions",
            "use server",
            "form action",
            "revalidatepath",
            "revalidatetag",
            "useformstatus",
            "useactionstate",
        ],
    ) {
        push_unique(&mut tags, "server_actions");
    }
    if contains_any(
        &text,
        &[
            "data fetching",
            "fetch",
            "cache",
            "revalidate",
            "isr",
            "swr",
            "usefetch",
            "useasyncdata",
            "uselazyfetch",
            "数据获取",
            "缓存",
        ],
    ) {
        push_unique(&mut tags, "data_fetching");
    }
    if contains_any(
        &text,
        &[
            "nuxt",
            "nitro",
            "definepagemeta",
            "runtimeconfig",
            "clientonly",
            "usehead",
            "useseometa",
            "hydration",
        ],
    ) {
        push_unique(&mut tags, "nuxt");
    }
    if contains_any(
        &text,
        &[
            "vite",
            "vite config",
            "build tooling",
            "bundle",
            "chunk",
            "sourcemap",
            "dev server",
            "proxy",
            "tree shaking",
            "构建",
            "打包",
        ],
    ) {
        push_unique(&mut tags, "build_tooling");
    }
    if contains_any(
        &text,
        &[
            "mobile",
            "native",
            "quasar",
            "capacitor",
            "pwa",
            "service worker",
            "offline",
            "push notification",
            "geolocation",
            "camera",
            "移动端",
            "离线",
        ],
    ) {
        push_unique(&mut tags, "mobile");
    }
    if contains_any(
        &text,
        &[
            "route",
            "router",
            "routing",
            "navigation",
            "navigate",
            "deep link",
            "deeplink",
            "guard",
            "resolver",
            "tab",
            "stack",
            "drawer",
            "query param",
            "route param",
            "路由",
            "导航",
            "守卫",
            "深链",
        ],
    ) {
        push_unique(&mut tags, "routing");
    }
    if contains_any(
        &text,
        &[
            "rxjs",
            "observable",
            "subscription",
            "subject",
            "behaviorsubject",
            "httpclient",
            "http client",
            "api call",
            "switchmap",
            "mergemap",
            "concatmap",
            "exhaustmap",
            "takeuntildestroyed",
            "marble",
            "流式",
            "订阅",
            "可观察",
        ],
    ) {
        push_unique(&mut tags, "rxjs");
    }
    if contains_any(
        &text,
        &[
            "ngrx",
            "store",
            "action group",
            "entity adapter",
            "selector",
            "effect",
            "effects",
            "reducer",
            "store devtools",
            "facade",
        ],
    ) {
        push_unique(&mut tags, "ngrx");
    }
    if contains_any(
        &text,
        &[
            "riverpod",
            "consumerwidget",
            "consumer widget",
            "widgetref",
            "provider scope",
            "providerscope",
            "state notifier",
            "statenotifier",
            "async notifier",
            "asyncnotifier",
        ],
    ) {
        push_unique(&mut tags, "riverpod");
    }
    if contains_any(
        &text,
        &[
            " bloc ",
            "cubit",
            "blocbuilder",
            "bloc builder",
            "bloclistener",
            "bloc listener",
            "blocconsumer",
            "bloc consumer",
            "blocprovider",
            "bloc provider",
            "event driven",
        ],
    ) {
        push_unique(&mut tags, "bloc");
    }
    if contains_any(
        &text,
        &[
            "list",
            "feed",
            "flatlist",
            "sectionlist",
            "flashlist",
            "virtualized",
            "infinite scroll",
            "pull to refresh",
            "refreshcontrol",
            "onendreached",
            "list performance",
            "列表性能",
            "列表",
            "动态列表",
            "下拉刷新",
            "无限滚动",
        ],
    ) {
        push_unique(&mut tags, "list_performance");
    }
    if contains_any(
        &text,
        &[
            "asyncstorage",
            "mmkv",
            "securestore",
            "secure store",
            "storage",
            "persist",
            "persistent",
            "local cache",
            "localstorage",
            "缓存持久化",
            "本地存储",
        ],
    ) {
        push_unique(&mut tags, "storage");
    }
    if contains_any(
        &text,
        &[
            "async",
            "concurrent",
            "queue",
            "stream",
            "websocket",
            "并发",
            "异步",
        ],
    ) {
        push_unique(&mut tags, "async");
    }
    if task_is_backend_task(task)
        && contains_any(
            &text,
            &[
                "spring cache",
                "cachemanager",
                "cache manager",
                "cacheable",
                "cacheevict",
                "caffeine",
                "redis cache",
                "application cache",
                "in-memory cache",
                "cache aside",
                "cache-aside",
                "cache hit",
                "cache miss",
                "缓存",
                "缓存键",
                "缓存失效",
                "缓存策略",
            ],
        )
    {
        push_unique(&mut tags, "cache");
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
    tags
}

fn extend_focus_tags_from_context(tags: &mut Vec<String>, context: &CodeReferenceTaskContext) {
    for (selected, tag) in [
        (context.security, "security"),
        (context.async_processing, "async"),
        (context.integration, "integration"),
        (context.resilience, "resilience"),
        (context.observability, "observability"),
    ] {
        if selected {
            push_unique(tags, tag);
        }
    }
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
        Some("typescript") => {
            has_focus("frontend")
                || has_focus("configuration")
                || has_focus("runtime")
                || has_focus("migration")
                || (roles.contains("backend")
                    && (has_focus("api") || has_focus("backend") || has_focus("testing")))
        }
        Some("javascript") => {
            has_focus("frontend")
                || (roles.contains("backend")
                    && (has_focus("api") || has_focus("backend") || has_focus("testing")))
        }
        Some(_) => {
            let frontend_applies = roles.contains("frontend") && has_focus("frontend");
            let backend_applies = roles.contains("backend")
                && (has_focus("backend")
                    || has_focus("api")
                    || has_focus("persistence")
                    || has_focus("security")
                    || has_focus("configuration")
                    || has_focus("runtime")
                    || has_focus("integration")
                    || has_focus("migration")
                    || has_focus("architecture")
                    || has_focus("testing"));
            let persistence_applies = roles.contains("persistence") && has_focus("persistence");
            let unclassified_applies = roles.is_empty()
                && (has_focus("api")
                    || has_focus("backend")
                    || has_focus("persistence")
                    || has_focus("security")
                    || has_focus("configuration")
                    || has_focus("runtime"));
            frontend_applies || backend_applies || persistence_applies || unclassified_applies
        }
        None => roles.contains("frontend") && has_focus("frontend"),
    }
}

fn reference_items_for_signal(
    signal: &CodeStackSignal,
    focus_tags: &[String],
    task: &TaskDefinition,
    stack_frameworks: &BTreeSet<String>,
) -> BTreeSet<String> {
    let has_focus = |tag: &str| focus_tags.iter().any(|item| item == tag);
    let mut items = BTreeSet::new();
    match signal.language.as_deref() {
        Some("java") => {
            items.insert("core".to_string());
            if signal
                .frameworks
                .iter()
                .any(|item| item == "spring_framework")
            {
                items.insert("spring".to_string());
            }
            if has_focus("persistence") && signal.frameworks.iter().any(|item| item == "jpa_orm") {
                items.insert("persistence".to_string());
            }
            if has_focus("security") {
                items.insert("security".to_string());
            }
            if has_focus("reactive")
                || (has_focus("api")
                    && signal
                        .frameworks
                        .iter()
                        .any(|item| item == "spring_webflux"))
                || ((has_focus("api") || has_focus("persistence"))
                    && signal
                        .frameworks
                        .iter()
                        .any(|item| matches!(item.as_str(), "project_reactor" | "r2dbc")))
            {
                items.insert("reactive".to_string());
            }
            if has_focus("testing") {
                items.insert("testing".to_string());
            }
        }
        Some("typescript") => {
            items.insert("core".to_string());
            if task_owns_typescript_type_modeling(task) {
                items.insert("types".to_string());
            }
            if task_owns_typescript_configuration(task) {
                items.insert("config".to_string());
            }
            if task_owns_typescript_pattern(task) {
                items.insert("patterns".to_string());
            }
            if has_focus("testing") {
                items.insert("testing".to_string());
            }
            if has_focus("api") || has_focus("api_client") {
                items.insert("guards".to_string());
            }
        }
        Some("javascript") => {
            items.insert("core".to_string());
            items.insert("modules".to_string());
            if has_focus("async") {
                items.insert("async".to_string());
            }
            if signal.roles.iter().any(|role| role == "frontend") {
                items.insert("browser".to_string());
            } else {
                items.insert("node".to_string());
            }
            if has_focus("testing") {
                items.insert("testing".to_string());
            }
        }
        Some("python") => {
            items.extend(["core", "typing"].map(str::to_string));
            if matches!(task.task_kind, TaskKind::ConfigurationSupport)
                || task_has_action(task, ImplementationAction::ImplementLanguageVersionFeature)
                || task_has_action(task, ImplementationAction::RefactorModuleStructure)
            {
                items.insert("packaging".to_string());
            }
            if has_focus("testing") {
                items.insert("testing".to_string());
            }
            if task_has_action(task, ImplementationAction::ImplementAsyncProcessing) {
                items.insert("async".to_string());
            }
        }
        Some("go") => {
            if matches!(
                task.task_kind,
                TaskKind::FeatureIncrement
                    | TaskKind::DataModelIncrement
                    | TaskKind::InterfaceIncrement
                    | TaskKind::IntegrationIncrement
                    | TaskKind::RefactorSupport
            ) {
                items.insert("core".to_string());
            }
            if task_owns_test_implementation(task) {
                items.insert("testing".to_string());
            }
            if task_has_action(task, ImplementationAction::ImplementAsyncProcessing) {
                items.insert("concurrency".to_string());
            }
            if task_has_action(task, ImplementationAction::ImplementGenericTypeAbstraction) {
                items.insert("generics".to_string());
            }
            if task_has_action(task, ImplementationAction::ImplementDependencyAbstraction) {
                items.insert("interfaces".to_string());
            }
            if matches!(task.task_kind, TaskKind::ConfigurationSupport)
                || task_has_action(task, ImplementationAction::AddOrUpdateConfig)
                || task_has_action(task, ImplementationAction::MigrateFrameworkImplementation)
                || task_has_action(task, ImplementationAction::RefactorModuleStructure)
            {
                items.insert("structure".to_string());
            }
        }
        Some("csharp") => {
            if matches!(
                task.task_kind,
                TaskKind::FeatureIncrement
                    | TaskKind::DataModelIncrement
                    | TaskKind::InterfaceIncrement
                    | TaskKind::IntegrationIncrement
                    | TaskKind::RefactorSupport
            ) || (stack_frameworks.contains("blazor") && task_owns_frontend_implementation(task))
            {
                items.insert("core".to_string());
            }
            if task_has_action(task, ImplementationAction::ImplementLanguageVersionFeature) {
                items.insert("modern".to_string());
            }
            if task_owns_test_implementation(task)
                && (!stack_frameworks.contains("aspnet_core") || task_is_frontend_task(task))
            {
                items.insert("testing".to_string());
            }
            if task_owns_persistence(task)
                && stack_frameworks.contains("entity_framework")
                && !stack_frameworks.contains("aspnet_core")
            {
                items.insert("persistence".to_string());
            }
            if stack_frameworks.contains("blazor") && task_owns_frontend_implementation(task) {
                items.insert("blazor".to_string());
            }
            if task_has_action(task, ImplementationAction::OptimizeRuntimePerformance) {
                items.insert("performance".to_string());
            }
        }
        Some("cpp") => {
            if matches!(
                task.task_kind,
                TaskKind::FeatureIncrement
                    | TaskKind::DataModelIncrement
                    | TaskKind::InterfaceIncrement
                    | TaskKind::IntegrationIncrement
                    | TaskKind::RefactorSupport
            ) {
                items.insert("core".to_string());
            }
            if task_has_action(task, ImplementationAction::ImplementLanguageVersionFeature) {
                items.insert("modern".to_string());
                items.insert("build".to_string());
            }
            if task_has_action(task, ImplementationAction::AddOrUpdateConfig)
                || task_has_action(task, ImplementationAction::MigrateFrameworkImplementation)
            {
                items.insert("build".to_string());
            }
            if task_owns_test_implementation(task) {
                items.insert("testing".to_string());
            }
            if task_has_action(task, ImplementationAction::ImplementGenericTypeAbstraction) {
                items.insert("templates".to_string());
            }
            if task_has_action(task, ImplementationAction::OptimizeRuntimePerformance) {
                items.insert("performance".to_string());
            }
            if task_has_action(task, ImplementationAction::ImplementAsyncProcessing) {
                items.insert("concurrency".to_string());
            }
        }
        Some("kotlin") => {
            items.insert("core".to_string());
            if has_focus("testing") {
                items.insert("testing".to_string());
            }
            if has_focus("async") {
                items.insert("coroutines".to_string());
            }
            if signal.frameworks.iter().any(|item| item == "ktor") {
                items.insert("ktor".to_string());
            }
            if signal.frameworks.iter().any(|item| item == "compose") {
                items.insert("compose".to_string());
            }
            if signal.frameworks.iter().any(|item| item == "kmp") {
                items.insert("multiplatform".to_string());
            }
            if task_has_action(task, ImplementationAction::ImplementGenericTypeAbstraction) {
                items.insert("dsl".to_string());
            }
        }
        Some("php") => {
            items.insert("core".to_string());
            if task_has_action(task, ImplementationAction::ImplementLanguageVersionFeature) {
                items.insert("modern".to_string());
            }
            if has_focus("testing") {
                items.insert("testing".to_string());
            }
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
            items.insert("core".to_string());
            if task_has_action(task, ImplementationAction::ImplementDependencyAbstraction)
                || task_has_action(task, ImplementationAction::ImplementGenericTypeAbstraction)
            {
                items.insert("traits".to_string());
            }
            if task_has_action(task, ImplementationAction::ImplementDependencyAbstraction)
                || task_has_action(task, ImplementationAction::ImplementGenericTypeAbstraction)
                || task_has_action(task, ImplementationAction::ImplementAsyncProcessing)
                || task_has_action(task, ImplementationAction::OptimizeRuntimePerformance)
                || task_has_action(task, ImplementationAction::RefactorModuleStructure)
            {
                items.insert("ownership".to_string());
            }
            if task_has_action(task, ImplementationAction::CreateOrUpdateInterface)
                || task_has_action(task, ImplementationAction::CreateOrUpdateBusinessRule)
                || task_has_action(
                    task,
                    ImplementationAction::ImplementExternalServiceIntegration,
                )
                || task_has_action(task, ImplementationAction::ImplementResiliencePolicy)
            {
                items.insert("errors".to_string());
            }
            if has_focus("testing") {
                items.insert("testing".to_string());
            }
            if task_has_action(task, ImplementationAction::ImplementAsyncProcessing) {
                items.insert("async".to_string());
            }
        }
        Some("swift") => {
            items.insert("core".to_string());
            if task_has_action(task, ImplementationAction::ImplementDependencyAbstraction)
                || task_has_action(task, ImplementationAction::ImplementGenericTypeAbstraction)
            {
                items.insert("protocols".to_string());
            }
            if has_focus("testing") {
                items.insert("testing".to_string());
            }
            if task_has_action(task, ImplementationAction::ImplementAsyncProcessing) {
                items.insert("concurrency".to_string());
            }
            if task_has_action(task, ImplementationAction::OptimizeRuntimePerformance) {
                items.insert("memory".to_string());
            }
            if signal.frameworks.iter().any(|item| item == "swiftui") {
                items.insert("swiftui".to_string());
            }
        }
        Some("sql") => {
            let owns_schema = task_owns_sql_schema(task);
            let owns_query = task_owns_sql_query(task);
            let owns_transaction = task_owns_sql_transaction(task);
            let owns_tests = task_owns_sql_tests(task);
            let owns_performance = task_owns_sql_performance(task);
            let owns_analytics = task_owns_sql_analytics(task);
            let task_scoped_sql = owns_schema || owns_query || owns_transaction || owns_tests;
            if owns_schema || (task_owns_persistence(task) && !task_scoped_sql) {
                items.insert("schema".to_string());
            }
            if owns_schema
                || owns_query
                || owns_transaction
                || owns_tests
                || owns_performance
                || owns_analytics
            {
                items.insert("dialects".to_string());
            }
            if owns_query {
                items.insert("queries".to_string());
            }
            if owns_performance {
                items.insert("optimization".to_string());
            }
            if owns_analytics {
                items.insert("windows".to_string());
            }
            if owns_query || owns_analytics || owns_tests {
                items.insert("queries".to_string());
            }
            if owns_schema || owns_query || owns_transaction || owns_tests {
                if signal.dialects.iter().any(|dialect| dialect == "mysql") {
                    if owns_schema || owns_tests {
                        items.insert("mysql.schema".to_string());
                    }
                    if owns_query || owns_performance || owns_analytics || owns_tests {
                        items.insert("mysql.queries".to_string());
                    }
                    if owns_transaction || owns_tests {
                        items.insert("mysql.transactions".to_string());
                    }
                }
                if signal
                    .dialects
                    .iter()
                    .any(|dialect| dialect == "postgresql")
                {
                    if owns_schema || owns_tests {
                        items.insert("postgresql.schema".to_string());
                    }
                    if owns_query || owns_performance || owns_analytics || owns_tests {
                        items.insert("postgresql.queries".to_string());
                    }
                    if owns_transaction || owns_tests {
                        items.insert("postgresql.transactions".to_string());
                    }
                }
                if signal
                    .dialects
                    .iter()
                    .any(|dialect| dialect == "sql_server")
                {
                    if owns_schema || owns_tests {
                        items.insert("sqlserver.schema".to_string());
                    }
                    if owns_query || owns_performance || owns_analytics || owns_tests {
                        items.insert("sqlserver.queries".to_string());
                    }
                    if owns_transaction || owns_tests {
                        items.insert("sqlserver.transactions".to_string());
                    }
                }
                if signal.dialects.iter().any(|dialect| dialect == "oracle") {
                    if owns_schema || owns_tests {
                        items.insert("oracle.schema".to_string());
                    }
                    if owns_query || owns_performance || owns_analytics || owns_tests {
                        items.insert("oracle.queries".to_string());
                    }
                    if owns_transaction || owns_tests {
                        items.insert("oracle.transactions".to_string());
                    }
                }
            }
        }
        _ => {}
    }
    items
}

fn backend_reference_items_for_signal(
    signal: &CodeStackSignal,
    stack_frameworks: &BTreeSet<String>,
    task: &TaskDefinition,
    context: &CodeReferenceTaskContext,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut groups = BTreeMap::<String, BTreeSet<String>>::new();
    if signal.frameworks.iter().any(|item| item == "spring_boot") {
        let mut items = BTreeSet::new();
        extend_spring_boot_task_references(&mut items, stack_frameworks, task, context);
        if !items.is_empty() {
            groups.insert("springboot".to_string(), items);
        }
    }
    if signal.frameworks.iter().any(|item| item == "django") {
        let mut items = BTreeSet::new();
        if task_owns_test_implementation(task) {
            items.insert("testing".to_string());
        }
        if task_owns_api_contract(task) && stack_frameworks.contains("django_rest_framework") {
            items.insert("views".to_string());
            items.insert("serializers".to_string());
        }
        if task_owns_persistence(task) {
            items.insert("models".to_string());
        }
        if context.security
            || task_has_action(
                task,
                ImplementationAction::ImplementAuthenticationOrAuthorization,
            )
        {
            items.insert("security".to_string());
        }
        if !items.is_empty() {
            groups.insert("django".to_string(), items);
        }
    }
    if signal.frameworks.iter().any(|item| item == "fastapi") {
        let mut items = BTreeSet::new();
        if task_owns_test_implementation(task) {
            items.insert("testing".to_string());
        }
        if task_owns_api_contract(task) {
            items.insert("routing".to_string());
            items.insert("schemas".to_string());
        }
        if task_owns_persistence(task) && stack_frameworks.contains("sqlalchemy") {
            items.insert("data".to_string());
        }
        if context.security
            || task_has_action(
                task,
                ImplementationAction::ImplementAuthenticationOrAuthorization,
            )
        {
            items.insert("security".to_string());
        }
        if task_has_action(task, ImplementationAction::MigrateFrameworkImplementation) {
            items.insert("migration".to_string());
        }
        if !items.is_empty() {
            groups.insert("fastapi".to_string(), items);
        }
    }
    if signal.frameworks.iter().any(|item| item == "aspnet_core") {
        let mut items = BTreeSet::new();
        if task_owns_test_implementation(task) && !task_is_frontend_task(task) {
            items.insert("testing".to_string());
        }
        if task_owns_api_contract(task) && stack_frameworks.contains("minimal_api") {
            items.insert("minimal".to_string());
        }
        if context.application_architecture && task_is_backend_task(task) {
            items.insert("architecture".to_string());
        }
        if task_owns_persistence(task) && stack_frameworks.contains("entity_framework") {
            items.insert("data".to_string());
        }
        if context.security
            || task_has_action(
                task,
                ImplementationAction::ImplementAuthenticationOrAuthorization,
            )
        {
            items.insert("security".to_string());
        }
        if matches!(task.task_kind, TaskKind::ConfigurationSupport)
            || task_has_action(task, ImplementationAction::AddOrUpdateConfig)
            || task_has_action(task, ImplementationAction::ImplementRuntimeDeliveryContract)
            || task_has_action(task, ImplementationAction::ImplementAsyncProcessing)
            || task_has_action(task, ImplementationAction::ImplementCachePolicy)
            || task_has_action(
                task,
                ImplementationAction::ImplementExternalServiceIntegration,
            )
            || task_has_action(task, ImplementationAction::ImplementResiliencePolicy)
            || task_has_action(
                task,
                ImplementationAction::ConfigureServiceRoutingOrDiscovery,
            )
            || task_has_action(task, ImplementationAction::ImplementObservability)
            || context.integration
            || context.resilience
            || context.observability
        {
            items.insert("runtime".to_string());
        }
        if !items.is_empty() {
            groups.insert("aspnetcore".to_string(), items);
        }
    }
    if signal.frameworks.iter().any(|item| item == "nestjs") {
        let mut items = BTreeSet::new();
        if task_owns_test_implementation(task) {
            items.insert("testing".to_string());
        }
        if task_owns_api_contract(task) {
            items.insert("controllers".to_string());
            items.insert("dtos".to_string());
            items.insert("services".to_string());
        }
        if task_owns_nest_service_boundary(task) {
            items.insert("services".to_string());
        }
        if context.security
            || task_has_action(
                task,
                ImplementationAction::ImplementAuthenticationOrAuthorization,
            )
        {
            items.insert("security".to_string());
        }
        if task_has_action(task, ImplementationAction::MigrateFrameworkImplementation) {
            items.insert("migration".to_string());
        }
        if !items.is_empty() {
            groups.insert("nestjs".to_string(), items);
        }
    }
    groups
}

fn extend_spring_boot_task_references(
    items: &mut BTreeSet<String>,
    stack_frameworks: &BTreeSet<String>,
    task: &TaskDefinition,
    context: &CodeReferenceTaskContext,
) {
    if task_owns_test_implementation(task) {
        items.insert("testing".to_string());
    }
    if task_owns_api_contract(task) {
        items.insert("web".to_string());
    }
    if task_owns_persistence(task) && stack_frameworks.contains("spring_data_jpa") {
        items.insert("data".to_string());
    }
    if context.security
        || task_has_action(
            task,
            ImplementationAction::ImplementAuthenticationOrAuthorization,
        )
    {
        items.insert("security".to_string());
    }
    if matches!(task.task_kind, TaskKind::ConfigurationSupport)
        || task_has_action(task, ImplementationAction::AddOrUpdateConfig)
        || task_has_action(task, ImplementationAction::ImplementRuntimeDeliveryContract)
    {
        items.insert("runtime".to_string());
    }
    if context.async_processing
        || task_has_action(task, ImplementationAction::ImplementAsyncProcessing)
    {
        items.insert("async".to_string());
    }
    if task_has_action(task, ImplementationAction::ImplementCachePolicy) {
        items.insert("cache".to_string());
    }
    if context.integration
        || task_has_action(
            task,
            ImplementationAction::ImplementExternalServiceIntegration,
        )
    {
        items.insert("integration".to_string());
    }
    if context.resilience || task_has_action(task, ImplementationAction::ImplementResiliencePolicy)
    {
        items.insert("resilience".to_string());
    }
    if task_has_action(
        task,
        ImplementationAction::ConfigureServiceRoutingOrDiscovery,
    ) && stack_frameworks.contains("spring_cloud")
    {
        items.insert("cloud".to_string());
    }
    if context.observability || task_has_action(task, ImplementationAction::ImplementObservability)
    {
        items.insert("observability".to_string());
    }
}

fn task_has_action(task: &TaskDefinition, expected: ImplementationAction) -> bool {
    task.implementation_actions
        .iter()
        .any(|action| *action == expected)
}

fn frontend_reference_items_for_signal(
    signal: &CodeStackSignal,
    focus_tags: &[String],
    task: &TaskDefinition,
) -> BTreeMap<String, BTreeSet<String>> {
    let has_focus = |tag: &str| focus_tags.iter().any(|item| item == tag);
    let mut groups = BTreeMap::<String, BTreeSet<String>>::new();
    if !has_focus("frontend") {
        return groups;
    }
    if signal.frameworks.iter().any(|item| item == "nextjs") {
        let mut items = BTreeSet::new();
        if task_owns_frontend_implementation(task) {
            items.insert("core".to_string());
        }
        if signal.frameworks.iter().any(|item| item == "app_router")
            && task_has_action(task, ImplementationAction::CreateOrUpdateFrontendNavigation)
        {
            items.insert("app-router".to_string());
        }
        if task_owns_test_implementation(task) {
            items.insert("testing".to_string());
        }
        if task_uses_api_client_binding(task)
            || task_owns_persistence(task)
            || task_has_action(task, ImplementationAction::ImplementReactiveClientFlow)
        {
            items.insert("data".to_string());
        }
        if signal.frameworks.iter().any(|item| item == "app_router")
            && task_has_action(task, ImplementationAction::ImplementServerMutation)
        {
            items.insert("actions".to_string());
        }
        if signal.frameworks.iter().any(|item| item == "app_router")
            && task_has_action(task, ImplementationAction::ImplementServerRenderedComponent)
        {
            items.insert("server-components".to_string());
        }
        if matches!(task.task_kind, TaskKind::ConfigurationSupport)
            || task_has_action(task, ImplementationAction::AddOrUpdateConfig)
            || task_has_action(task, ImplementationAction::ImplementRuntimeDeliveryContract)
            || task_has_action(task, ImplementationAction::OptimizeFrontendPerformance)
        {
            items.insert("runtime".to_string());
        }
        groups.insert("nextjs".to_string(), items);
    }
    if signal.frameworks.iter().any(|item| item == "react") {
        let mut items = BTreeSet::new();
        let nextjs_owns_framework_boundary = signal.frameworks.iter().any(|item| item == "nextjs");
        if task_owns_frontend_implementation(task) {
            items.insert("core".to_string());
        }
        if task_has_action(task, ImplementationAction::ImplementReactiveClientFlow) {
            items.insert("hooks".to_string());
        }
        if task_uses_api_client_binding(task)
            || task_has_action(task, ImplementationAction::ImplementSharedClientState)
        {
            items.insert("state".to_string());
        }
        if task_owns_test_implementation(task) && !nextjs_owns_framework_boundary {
            items.insert("testing".to_string());
        }
        if task_has_action(task, ImplementationAction::OptimizeFrontendPerformance)
            && !nextjs_owns_framework_boundary
        {
            items.insert("performance".to_string());
        }
        if signal
            .frameworks
            .iter()
            .any(|item| item == "react_server_components")
            && task_has_action(task, ImplementationAction::ImplementServerRenderedComponent)
            && !nextjs_owns_framework_boundary
        {
            items.insert("server-components".to_string());
        }
        if signal.frameworks.iter().any(|item| item == "react19")
            && task_has_action(
                task,
                ImplementationAction::ImplementFrontendFrameworkVersionFeature,
            )
            && !nextjs_owns_framework_boundary
        {
            items.insert("react19".to_string());
        }
        if task_has_action(task, ImplementationAction::MigrateFrameworkImplementation) {
            items.insert("migration".to_string());
        }
        groups.insert("react".to_string(), items);
    }
    if signal.frameworks.iter().any(|item| item == "vue") {
        let mut items = BTreeSet::new();
        let owns_vue_component = task_owns_frontend_surface(task)
            || task_has_action(task, ImplementationAction::CreateOrUpdateFrontendNavigation)
            || task_has_action(task, ImplementationAction::ImplementReactiveClientFlow)
            || task_has_action(task, ImplementationAction::ImplementSharedClientState)
            || task_has_action(task, ImplementationAction::OptimizeFrontendPerformance)
            || task_has_action(task, ImplementationAction::ImplementServerRenderedComponent)
            || task_has_action(
                task,
                ImplementationAction::ImplementFrontendFrameworkVersionFeature,
            );
        if owns_vue_component {
            items.insert("core".to_string());
        }
        if task_owns_frontend_surface(task) {
            items.insert("components".to_string());
        }
        if task_uses_api_client_binding(task)
            || task_has_action(task, ImplementationAction::ImplementSharedClientState)
            || task_has_action(task, ImplementationAction::ImplementClientStorage)
        {
            items.insert("state".to_string());
        }
        if task_owns_test_implementation(task) {
            items.insert("testing".to_string());
        }
        if signal.language.as_deref() == Some("typescript") && owns_vue_component {
            items.insert("typescript".to_string());
        }
        if signal.frameworks.iter().any(|item| item == "nuxt")
            && (task_has_action(task, ImplementationAction::CreateOrUpdateFrontendNavigation)
                || task_has_action(task, ImplementationAction::ImplementServerRenderedComponent)
                || task_has_action(task, ImplementationAction::ImplementServerMutation)
                || task_has_action(task, ImplementationAction::AddOrUpdateConfig)
                || task_has_action(task, ImplementationAction::MigrateFrameworkImplementation))
        {
            items.insert("nuxt".to_string());
        }
        if task_has_action(task, ImplementationAction::AddOrUpdateConfig)
            || task_has_action(task, ImplementationAction::MigrateFrameworkImplementation)
        {
            items.insert("build".to_string());
        }
        if signal
            .frameworks
            .iter()
            .any(|item| matches!(item.as_str(), "quasar" | "capacitor" | "pwa"))
            && task_has_action(task, ImplementationAction::ImplementMobilePlatformBehavior)
        {
            items.insert("mobile".to_string());
        }
        groups.insert("vue".to_string(), items);
    }
    if signal.frameworks.iter().any(|item| item == "angular") {
        let mut items = BTreeSet::new();
        if task_owns_frontend_implementation(task) {
            items.insert("core".to_string());
            items.insert("components".to_string());
        }
        if task_owns_test_implementation(task) {
            items.insert("testing".to_string());
        }
        if task_has_action(task, ImplementationAction::CreateOrUpdateFrontendNavigation) {
            items.insert("routing".to_string());
        }
        if task_uses_api_client_binding(task)
            || task_has_action(task, ImplementationAction::ImplementReactiveClientFlow)
        {
            items.insert("rxjs".to_string());
        }
        if signal.frameworks.iter().any(|item| item == "ngrx")
            && task_has_action(task, ImplementationAction::ImplementSharedClientState)
        {
            items.insert("ngrx".to_string());
        }
        groups.insert("angular".to_string(), items);
    }
    if signal.frameworks.iter().any(|item| item == "reactnative") {
        let mut items = BTreeSet::new();
        if task_owns_frontend_implementation(task) {
            items.insert("core".to_string());
        }
        if matches!(task.task_kind, TaskKind::ConfigurationSupport)
            || task_has_action(task, ImplementationAction::AddOrUpdateConfig)
            || task_has_action(task, ImplementationAction::MigrateFrameworkImplementation)
            || task_has_action(
                task,
                ImplementationAction::ImplementFrontendExperienceContract,
            )
        {
            items.insert("structure".to_string());
        }
        if task_owns_test_implementation(task) {
            items.insert("testing".to_string());
        }
        if task_has_action(task, ImplementationAction::CreateOrUpdateFrontendNavigation) {
            items.insert("navigation".to_string());
        }
        if task_has_action(task, ImplementationAction::OptimizeFrontendPerformance) {
            items.insert("lists".to_string());
        }
        if task_has_action(task, ImplementationAction::ImplementMobilePlatformBehavior) {
            items.insert("platform".to_string());
        }
        if task_has_action(task, ImplementationAction::ImplementClientStorage) {
            items.insert("storage".to_string());
        }
        groups.insert("reactnative".to_string(), items);
    }
    if signal.frameworks.iter().any(|item| item == "flutter") {
        let mut items = BTreeSet::new();
        if task_owns_frontend_implementation(task) {
            items.insert("core".to_string());
        }
        if task_owns_frontend_surface(task) {
            items.insert("widgets".to_string());
        }
        if matches!(task.task_kind, TaskKind::ConfigurationSupport)
            || task_has_action(task, ImplementationAction::AddOrUpdateConfig)
            || task_has_action(task, ImplementationAction::MigrateFrameworkImplementation)
            || task_has_action(
                task,
                ImplementationAction::ImplementFrontendExperienceContract,
            )
        {
            items.insert("structure".to_string());
        }
        if task_owns_test_implementation(task) {
            items.insert("testing".to_string());
        }
        if task_has_action(task, ImplementationAction::CreateOrUpdateFrontendNavigation) {
            items.insert("navigation".to_string());
        }
        if signal.frameworks.iter().any(|item| item == "riverpod")
            && task_has_action(task, ImplementationAction::ImplementSharedClientState)
        {
            items.insert("riverpod".to_string());
        }
        if signal.frameworks.iter().any(|item| item == "bloc")
            && task_has_action(task, ImplementationAction::ImplementSharedClientState)
        {
            items.insert("bloc".to_string());
        }
        if task_has_action(task, ImplementationAction::OptimizeFrontendPerformance) {
            items.insert("performance".to_string());
        }
        groups.insert("flutter".to_string(), items);
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
    if let Some((ref_prefix, path_group, label)) = match group_key {
        "react" => Some(("fe.react", "react", "React")),
        "nextjs" => Some(("fe.next", "nextjs", "Next.js")),
        "vue" => Some(("fe.vue", "vue", "Vue")),
        "angular" => Some(("fe.angular", "angular", "Angular")),
        "reactnative" => Some(("fe.rn", "react-native", "React Native")),
        "flutter" => Some(("fe.flutter", "flutter", "Flutter")),
        _ => None,
    } {
        return ReferenceLoadPlanItem {
            ref_id: format!("{ref_prefix}.{group}"),
            path: format!("tech/frontend/{path_group}/{group}.md"),
            reason: format!(
                "Selected {label} {group} frontend framework quality reference for this task."
            ),
        };
    }
    if group_key == "sql" {
        if let Some((provider, subject)) = group.split_once('.') {
            if matches!(provider, "mysql" | "postgresql" | "sqlserver" | "oracle")
                && matches!(subject, "schema" | "queries" | "transactions")
            {
                let label = match provider {
                    "mysql" => "MySQL",
                    "postgresql" => "PostgreSQL",
                    "sqlserver" => "SQL Server",
                    _ => "Oracle",
                };
                return ReferenceLoadPlanItem {
                    ref_id: format!("tech.code.sql.{provider}.{subject}"),
                    path: format!("tech/code/sql/{provider}/{subject}.md"),
                    reason: format!(
                        "Selected {label} {subject} dialect reference for this persistence task."
                    ),
                };
            }
        }
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
                    | ImplementationAction::CreateOrUpdateFrontendNavigation
                    | ImplementationAction::ImplementReactiveClientFlow
                    | ImplementationAction::ImplementSharedClientState
                    | ImplementationAction::OptimizeFrontendPerformance
                    | ImplementationAction::ImplementServerRenderedComponent
                    | ImplementationAction::ImplementServerMutation
                    | ImplementationAction::ImplementFrontendFrameworkVersionFeature
                    | ImplementationAction::ImplementMobilePlatformBehavior
                    | ImplementationAction::ImplementClientStorage
                    | ImplementationAction::ImplementFrontendExperienceContract
                    | ImplementationAction::CreateEntityAdminPage
            )
        })
}

fn task_owns_frontend_implementation(task: &TaskDefinition) -> bool {
    matches!(
        task.task_kind,
        TaskKind::FrontendExperience | TaskKind::UiFlowIncrement
    ) || task.implementation_actions.iter().any(|action| {
        matches!(
            action,
            ImplementationAction::CreateOrUpdateUiFlow
                | ImplementationAction::CreateOrUpdateFrontendNavigation
                | ImplementationAction::ImplementReactiveClientFlow
                | ImplementationAction::ImplementSharedClientState
                | ImplementationAction::OptimizeFrontendPerformance
                | ImplementationAction::ImplementServerRenderedComponent
                | ImplementationAction::ImplementServerMutation
                | ImplementationAction::ImplementFrontendFrameworkVersionFeature
                | ImplementationAction::WireReferenceInApiOrUi
                | ImplementationAction::CreateEntityAdminPage
                | ImplementationAction::ImplementFrontendExperienceContract
        )
    })
}

fn task_owns_frontend_surface(task: &TaskDefinition) -> bool {
    matches!(
        task.task_kind,
        TaskKind::FrontendExperience | TaskKind::UiFlowIncrement
    ) || task.implementation_actions.iter().any(|action| {
        matches!(
            action,
            ImplementationAction::CreateOrUpdateUiFlow
                | ImplementationAction::CreateEntityAdminPage
                | ImplementationAction::ImplementFrontendExperienceContract
        )
    })
}

fn task_owns_test_implementation(task: &TaskDefinition) -> bool {
    matches!(task.task_kind, TaskKind::VerificationIncrement)
        || task.implementation_actions.iter().any(|action| {
            matches!(
                action,
                ImplementationAction::AddOrUpdateTests
                    | ImplementationAction::AddOrUpdatePersistenceTests
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
                | ImplementationAction::CreateOrUpdatePersistenceQuery
                | ImplementationAction::ImplementPersistenceTransaction
                | ImplementationAction::OptimizePersistenceQuery
                | ImplementationAction::ImplementAnalyticalQuery
                | ImplementationAction::AddOrUpdatePersistenceTests
                | ImplementationAction::ImplementEntityLifecycle
                | ImplementationAction::ImplementAuthenticationOrAuthorization
                | ImplementationAction::ImplementAsyncProcessing
                | ImplementationAction::ImplementCachePolicy
                | ImplementationAction::ImplementExternalServiceIntegration
                | ImplementationAction::ImplementResiliencePolicy
                | ImplementationAction::ConfigureServiceRoutingOrDiscovery
                | ImplementationAction::ImplementObservability
                | ImplementationAction::MigrateFrameworkImplementation
                | ImplementationAction::RefactorSupportingCode
        )
    })
}

fn task_owns_api_contract(task: &TaskDefinition) -> bool {
    if task_is_frontend_task(task) {
        return false;
    }
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

fn task_uses_api_client_binding(task: &TaskDefinition) -> bool {
    task_is_frontend_task(task)
        && (!task.write_boundary.artifact_refs.interfaces.is_empty()
            || task
                .implementation_actions
                .iter()
                .any(|action| matches!(action, ImplementationAction::WireReferenceInApiOrUi)))
}

fn task_owns_typescript_type_modeling(task: &TaskDefinition) -> bool {
    task.implementation_actions.iter().any(|action| {
        matches!(
            action,
            ImplementationAction::CreateOrUpdateEntity
                | ImplementationAction::CreateOrUpdateInterface
                | ImplementationAction::CreateOrUpdateStateMachine
                | ImplementationAction::CreateOrUpdateBusinessRule
                | ImplementationAction::ImplementGenericTypeAbstraction
                | ImplementationAction::AddReferenceField
                | ImplementationAction::ValidateReferenceFormat
                | ImplementationAction::WireReferenceInApiOrUi
        )
    })
}

fn task_owns_typescript_configuration(task: &TaskDefinition) -> bool {
    matches!(task.task_kind, TaskKind::ConfigurationSupport)
        || task.implementation_actions.iter().any(|action| {
            matches!(
                action,
                ImplementationAction::AddOrUpdateConfig
                    | ImplementationAction::ImplementLanguageVersionFeature
                    | ImplementationAction::RefactorModuleStructure
                    | ImplementationAction::MigrateFrameworkImplementation
            )
        })
}

fn task_owns_typescript_pattern(task: &TaskDefinition) -> bool {
    task.implementation_actions.iter().any(|action| {
        matches!(
            action,
            ImplementationAction::CreateOrUpdateInterface
                | ImplementationAction::CreateOrUpdateStateMachine
                | ImplementationAction::ImplementSharedClientState
                | ImplementationAction::ImplementGenericTypeAbstraction
                | ImplementationAction::ImplementDependencyAbstraction
                | ImplementationAction::WireReferenceInApiOrUi
                | ImplementationAction::CreateEntityCrud
                | ImplementationAction::CreateEntityRepository
                | ImplementationAction::CreateOrUpdatePersistenceQuery
                | ImplementationAction::ImplementExternalServiceIntegration
                | ImplementationAction::ImplementResiliencePolicy
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
                    | ImplementationAction::CreateOrUpdatePersistenceQuery
                    | ImplementationAction::ImplementPersistenceTransaction
                    | ImplementationAction::OptimizePersistenceQuery
                    | ImplementationAction::ImplementAnalyticalQuery
                    | ImplementationAction::AddOrUpdatePersistenceTests
            )
        })
}

fn task_owns_nest_service_boundary(task: &TaskDefinition) -> bool {
    task.implementation_actions.iter().any(|action| {
        matches!(
            action,
            ImplementationAction::CreateOrUpdateBusinessRule
                | ImplementationAction::CreateOrUpdateStateMachine
                | ImplementationAction::CreateEntityCrud
                | ImplementationAction::CreateEntityRepository
                | ImplementationAction::ImplementPersistenceTransaction
                | ImplementationAction::ImplementEntityLifecycle
                | ImplementationAction::ImplementAsyncProcessing
                | ImplementationAction::ImplementCachePolicy
                | ImplementationAction::ImplementExternalServiceIntegration
                | ImplementationAction::ImplementResiliencePolicy
                | ImplementationAction::ImplementObservability
                | ImplementationAction::RefactorSupportingCode
        )
    })
}

fn task_owns_sql_schema(task: &TaskDefinition) -> bool {
    matches!(task.task_kind, TaskKind::DataModelIncrement)
        || task.implementation_actions.iter().any(|action| {
            matches!(
                action,
                ImplementationAction::CreateOrUpdateEntity
                    | ImplementationAction::CreateOrUpdatePersistence
                    | ImplementationAction::CreateEntityMigration
                    | ImplementationAction::CreateEntityCrud
            )
        })
}

fn task_owns_sql_query(task: &TaskDefinition) -> bool {
    task.implementation_actions.iter().any(|action| {
        matches!(
            action,
            ImplementationAction::CreateEntityRepository
                | ImplementationAction::CreateEntityCrud
                | ImplementationAction::CreateOrUpdatePersistenceQuery
                | ImplementationAction::OptimizePersistenceQuery
                | ImplementationAction::ImplementAnalyticalQuery
        )
    })
}

fn task_owns_sql_transaction(task: &TaskDefinition) -> bool {
    task.implementation_actions.iter().any(|action| {
        matches!(
            action,
            ImplementationAction::CreateOrUpdatePersistence
                | ImplementationAction::CreateEntityRepository
                | ImplementationAction::CreateEntityCrud
                | ImplementationAction::ImplementPersistenceTransaction
        )
    })
}

fn task_owns_sql_performance(task: &TaskDefinition) -> bool {
    task.implementation_actions
        .iter()
        .any(|action| matches!(action, ImplementationAction::OptimizePersistenceQuery))
}

fn task_owns_sql_analytics(task: &TaskDefinition) -> bool {
    task.implementation_actions
        .iter()
        .any(|action| matches!(action, ImplementationAction::ImplementAnalyticalQuery))
}

fn task_owns_sql_tests(task: &TaskDefinition) -> bool {
    task.implementation_actions
        .iter()
        .any(|action| matches!(action, ImplementationAction::AddOrUpdatePersistenceTests))
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

fn selection_mentions_frontend_framework(haystack: &str) -> bool {
    contains_any(
        haystack,
        &[
            "react",
            "next",
            "nextjs",
            "next js",
            "app router",
            "vue",
            "nuxt",
            "svelte",
            "angular",
            "ngrx",
            "react native",
            "reactnative",
            "expo",
            "expo router",
            "flutter",
            "riverpod",
            "go router",
            "gorouter",
        ],
    )
}

fn push_frontend_frameworks_from_haystack(haystack: &str, frameworks: &mut Vec<String>) {
    let react_native = contains_any(
        haystack,
        &["react native", "reactnative", "expo", "expo router"],
    );
    if react_native {
        push_unique(frameworks, "reactnative");
        push_if_contains(haystack, frameworks, "expo", &["expo", "expo router"]);
    }
    if selection_mentions_flutter_framework(haystack) {
        push_unique(frameworks, "flutter");
        push_if_contains(haystack, frameworks, "riverpod", &["riverpod"]);
        push_if_contains(haystack, frameworks, "bloc", &[" bloc ", "cubit"]);
        push_if_contains(haystack, frameworks, "gorouter", &["go router", "gorouter"]);
    }
    push_if_contains(
        haystack,
        frameworks,
        "nextjs",
        &["next", "nextjs", "next js", "app router"],
    );
    push_if_contains(haystack, frameworks, "app_router", &["app router"]);
    if !react_native {
        push_if_contains(
            haystack,
            frameworks,
            "react",
            &["react", "next", "nextjs", "next js"],
        );
    }
    push_if_contains(haystack, frameworks, "react19", &["react 19", "react19"]);
    push_if_contains(
        haystack,
        frameworks,
        "react_server_components",
        &[
            "react server component",
            "server components",
            " rsc ",
            "app router",
        ],
    );
    push_if_contains(haystack, frameworks, "vue", &["vue", "nuxt"]);
    push_if_contains(haystack, frameworks, "nuxt", &["nuxt"]);
    push_if_contains(haystack, frameworks, "quasar", &["quasar"]);
    push_if_contains(haystack, frameworks, "capacitor", &["capacitor"]);
    push_if_contains(
        haystack,
        frameworks,
        "pwa",
        &["progressive web app", " pwa ", "pwa+", "+pwa"],
    );
    push_if_contains(haystack, frameworks, "angular", &["angular", "ngrx"]);
    push_if_contains(haystack, frameworks, "ngrx", &["ngrx"]);
    push_if_contains(haystack, frameworks, "svelte", &["svelte"]);
}

fn selection_mentions_flutter_framework(haystack: &str) -> bool {
    contains_any(haystack, &["flutter", "riverpod", "go router", "gorouter"])
}

fn push_spring_frameworks_from_haystack(haystack: &str, frameworks: &mut Vec<String>) {
    push_if_contains(haystack, frameworks, "spring_framework", &["spring"]);
    push_if_contains(
        haystack,
        frameworks,
        "spring_boot",
        &["spring boot", "springboot"],
    );
    push_if_contains(
        haystack,
        frameworks,
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
        haystack,
        frameworks,
        "spring_data_jpa",
        &["spring data jpa", "spring-data-jpa"],
    );
    push_if_contains(
        haystack,
        frameworks,
        "jpa_orm",
        &["jpa", "hibernate", "eclipselink"],
    );
    push_if_contains(
        haystack,
        frameworks,
        "spring_webflux",
        &["spring webflux", "webflux"],
    );
    push_if_contains(
        haystack,
        frameworks,
        "project_reactor",
        &["project reactor", "reactor"],
    );
    push_if_contains(haystack, frameworks, "r2dbc", &["r2dbc"]);
}

fn push_unique(output: &mut Vec<String>, value: &str) {
    if !output.iter().any(|item| item == value) {
        output.push(value.to_string());
    }
}

pub fn package_naming_policy_for_reference_groups(
    reference_groups: &BTreeMap<String, Vec<String>>,
) -> Option<CodePackageNamingPolicy> {
    let applies_to = ["java", "springboot", "kotlin"]
        .into_iter()
        .filter(|group| reference_groups.contains_key(*group))
        .map(str::to_string)
        .collect::<Vec<_>>();
    if applies_to.is_empty() {
        return None;
    }
    Some(jvm_package_naming_policy(applies_to))
}

pub fn jvm_package_naming_policy(applies_to: Vec<String>) -> CodePackageNamingPolicy {
    CodePackageNamingPolicy {
        applies_to,
        priority_order: vec![
            "existing production package root in src/main".to_string(),
            "build metadata group such as Gradle group or Maven groupId".to_string(),
            "confirmed organization or product namespace from project context".to_string(),
            "fallback app.<project_slug> derived from repository or confirmed project name"
                .to_string(),
            "absolute fallback app.generated only when no stable project slug exists"
                .to_string(),
        ],
        forbidden_package_prefixes: forbidden_jvm_package_prefixes()
            .into_iter()
            .map(str::to_string)
            .collect(),
        fallback_package_template: "app.<project_slug>".to_string(),
        absolute_fallback_package: "app.generated".to_string(),
        notes: vec![
            "project_slug must use lowercase letters and digits; split invalid separators into package segments and drop empty segments.".to_string(),
            "Fallback packages are local bootstrap namespaces, not public organization identities, and should be replaced when a real organization namespace is known.".to_string(),
        ],
    }
}

pub fn forbidden_jvm_package_prefixes() -> Vec<&'static str> {
    vec![
        "com.example",
        "org.example",
        "net.example",
        "io.example",
        "example",
        "com.company",
        "org.company",
        "company",
        "com.demo",
        "org.demo",
        "demo",
        "com.sample",
        "org.sample",
        "sample",
    ]
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

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
            source: TechnicalBaselineSource::AgentRecommendedForNewProject,
            project_kind: ProjectKind::NewProject,
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
        assert!(!selection.reference_groups["java"].contains(&"testing".to_string()));
        assert!(selection.reference_groups["springboot"].contains(&"data".to_string()));
        assert!(!selection.reference_groups["java"].contains(&"reactive".to_string()));
        assert!(!selection.reference_groups["springboot"].contains(&"testing".to_string()));
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
        let policy = package_naming_policy_for_reference_groups(&selection.reference_groups)
            .expect("JVM package naming policy");
        assert_eq!(policy.fallback_package_template, "app.<project_slug>");
        assert_eq!(policy.absolute_fallback_package, "app.generated");
        assert!(policy
            .forbidden_package_prefixes
            .contains(&"com.example".to_string()));
    }

    #[test]
    fn maps_react_typescript_only_for_frontend_task() {
        let baseline = baseline(json!({
            "tracks": {
                "web": {"selection": "React + TypeScript"},
                "backend": {"selection": "Java + Spring Boot"}
            }
        }));
        let seed = build_code_quality_seed(&baseline);
        assert_eq!(
            seed.pointer("/unmappedSignals")
                .and_then(Value::as_array)
                .map(Vec::len),
            Some(0)
        );
        assert!(seed.get("techReferenceProfile").is_none());
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![
                ImplementationAction::CreateOrUpdateUiFlow,
                ImplementationAction::WireReferenceInApiOrUi,
                ImplementationAction::ImplementReactiveClientFlow,
            ],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        task.write_boundary.artifact_refs.interfaces = vec!["api.purchase.create".to_string()];
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups.contains_key("typescript"));
        assert!(selection.reference_groups.contains_key("react"));
        assert!(selection.reference_groups["react"].contains(&"core".to_string()));
        assert!(selection.reference_groups["react"].contains(&"hooks".to_string()));
        assert!(selection.reference_groups["react"].contains(&"state".to_string()));
        assert!(!selection.reference_groups["react"].contains(&"testing".to_string()));
        assert!(!selection.reference_groups["react"].contains(&"migration".to_string()));
        assert!(!selection.reference_groups["react"].contains(&"server-components".to_string()));
        assert!(!selection.reference_groups.contains_key("java"));
        assert!(!selection.reference_groups.contains_key("springboot"));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.react.core" && item.path == "tech/frontend/react/core.md"
        }));
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.react.hooks" && item.path == "tech/frontend/react/hooks.md"
        }));
        assert!(load_plan
            .iter()
            .any(|item| item.path == "tech/code/typescript/core.md"));
        assert!(!load_plan
            .iter()
            .any(|item| item.path == "tech/code/java/spring.md"));
        assert!(!load_plan
            .iter()
            .any(|item| item.path.starts_with("tech/backend/springboot/")));
    }

    #[test]
    fn testing_references_require_structured_test_ownership() {
        let baseline = baseline(json!({
            "tracks": {
                "web": {"selection": "React + TypeScript"}
            }
        }));
        let seed = build_code_quality_seed(&baseline);
        assert!(seed.get("techReferenceProfile").is_none());

        let mut implementation_task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        implementation_task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let implementation_selection =
            code_reference_selection_for_task(&baseline, &implementation_task).unwrap();
        assert!(!implementation_selection.reference_groups["typescript"]
            .contains(&"testing".to_string()));
        assert!(
            !implementation_selection.reference_groups["react"].contains(&"testing".to_string())
        );

        let mut test_task = task(
            TaskKind::VerificationIncrement,
            vec![ImplementationAction::AddOrUpdateTests],
        );
        test_task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let test_selection = code_reference_selection_for_task(&baseline, &test_task).unwrap();
        assert!(test_selection.reference_groups["typescript"].contains(&"testing".to_string()));
        assert!(test_selection.reference_groups["react"].contains(&"testing".to_string()));
        let load_plan = code_reference_load_plan(&test_selection.reference_groups);
        assert!(load_plan
            .iter()
            .any(|item| item.path == "tech/code/typescript/testing.md"));
        assert!(load_plan
            .iter()
            .any(|item| item.path == "tech/frontend/react/testing.md"));
    }

    #[test]
    fn typescript_references_are_scoped_to_owned_capabilities() {
        let baseline = baseline(json!({
            "tracks": {
                "web": {"selection": "React + TypeScript"}
            }
        }));

        let mut ui_task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        ui_task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let ui = code_reference_selection_for_task(&baseline, &ui_task).unwrap();
        let typescript = &ui.reference_groups["typescript"];
        assert!(typescript.contains(&"core".to_string()));
        assert!(!typescript.contains(&"types".to_string()));
        assert!(!typescript.contains(&"config".to_string()));
        assert!(!typescript.contains(&"patterns".to_string()));
        assert!(!typescript.contains(&"guards".to_string()));

        let mut api_client_task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::WireReferenceInApiOrUi],
        );
        api_client_task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let api_client = code_reference_selection_for_task(&baseline, &api_client_task).unwrap();
        let api_typescript = &api_client.reference_groups["typescript"];
        assert!(api_typescript.contains(&"core".to_string()));
        assert!(api_typescript.contains(&"types".to_string()));
        assert!(api_typescript.contains(&"guards".to_string()));
        assert!(api_typescript.contains(&"patterns".to_string()));
        assert!(!api_typescript.contains(&"config".to_string()));

        let config_task = task(
            TaskKind::ConfigurationSupport,
            vec![ImplementationAction::AddOrUpdateConfig],
        );
        let config = code_reference_selection_for_task(&baseline, &config_task).unwrap();
        let config_typescript = &config.reference_groups["typescript"];
        assert!(config_typescript.contains(&"core".to_string()));
        assert!(config_typescript.contains(&"config".to_string()));
        assert!(!config_typescript.contains(&"types".to_string()));
        assert!(!config_typescript.contains(&"patterns".to_string()));
        assert!(!config_typescript.contains(&"testing".to_string()));
    }

    #[test]
    fn typescript_specialized_references_ignore_prose_without_ownership() {
        let baseline = baseline(json!({
            "tracks": {
                "web": {"selection": "React + TypeScript"}
            }
        }));
        let mut prose_task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        prose_task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        prose_task.objective =
            "Use advanced generics, utility types, a builder, and a strict tsconfig.".to_string();

        let selection = code_reference_selection_for_task(&baseline, &prose_task).unwrap();
        let typescript = &selection.reference_groups["typescript"];
        assert!(typescript.contains(&"core".to_string()));
        assert!(!typescript.contains(&"types".to_string()));
        assert!(!typescript.contains(&"config".to_string()));
        assert!(!typescript.contains(&"patterns".to_string()));
    }

    #[test]
    fn typescript_references_are_complete_and_decision_oriented() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let references = [
            (
                "plugins/shared/loom/references/tech/code/typescript/core.md",
                40,
                &["## Boundary Decisions", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/typescript/types.md",
                40,
                &["## Decision Rules", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/typescript/guards.md",
                40,
                &["## Boundary Decisions", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/typescript/config.md",
                40,
                &["## Decision Rules", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/typescript/patterns.md",
                40,
                &["## Decision Rules", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/typescript/testing.md",
                40,
                &["## Decision Rules", "## Evidence Focus"][..],
            ),
        ];

        for (relative, minimum_lines, required_sections) in references {
            let path = root.join(relative);
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            assert!(
                content.lines().count() >= minimum_lines,
                "{} is too thin",
                path.display()
            );
            for section in required_sections {
                assert!(
                    content.contains(section),
                    "{} missing {section}",
                    path.display()
                );
            }
        }
    }

    #[test]
    fn maps_framework_only_react_baseline_to_frontend_refs() {
        let baseline = baseline(json!({
            "tracks": {
                "web": {"selection": "React"}
            }
        }));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups.contains_key("react"));
        assert!(!selection.reference_groups.contains_key("typescript"));
        assert!(selection.unmapped_signals.is_empty());
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.react.core" && item.path == "tech/frontend/react/core.md"
        }));
    }

    #[test]
    fn react_specialized_refs_are_task_scoped() {
        let baseline = baseline(json!({
            "tracks": {
                "web": {"selection": "React 19 + React Server Components + TypeScript"}
            }
        }));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![
                ImplementationAction::CreateOrUpdateUiFlow,
                ImplementationAction::MigrateFrameworkImplementation,
                ImplementationAction::ImplementFrontendFrameworkVersionFeature,
                ImplementationAction::ImplementServerRenderedComponent,
                ImplementationAction::OptimizeFrontendPerformance,
            ],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        task.objective =
            "Implement the accepted React modernization and rendering boundary.".to_string();
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        let react_refs = &selection.reference_groups["react"];
        assert!(react_refs.contains(&"migration".to_string()));
        assert!(react_refs.contains(&"react19".to_string()));
        assert!(react_refs.contains(&"server-components".to_string()));
        assert!(react_refs.contains(&"performance".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.react.migration" && item.path == "tech/frontend/react/migration.md"
        }));
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.react.react19" && item.path == "tech/frontend/react/react19.md"
        }));
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.react.server-components"
                && item.path == "tech/frontend/react/server-components.md"
        }));
    }

    #[test]
    fn react_specialized_references_ignore_prose_and_require_stack_capabilities() {
        let specialized_baseline = baseline(json!({
            "tracks": {"web": {"selection": "React 19 + React Server Components + TypeScript"}}
        }));
        let react18_baseline = baseline(json!({
            "tracks": {"web": {"selection": "React 18 + TypeScript"}}
        }));
        let mut prose_only = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        prose_only.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        prose_only.objective = "Migrate classes to hooks, add React 19 useActionState, Server Components, memo performance, and tests.".to_string();
        let mut unsupported = task(
            TaskKind::UiFlowIncrement,
            vec![
                ImplementationAction::CreateOrUpdateUiFlow,
                ImplementationAction::ImplementFrontendFrameworkVersionFeature,
                ImplementationAction::ImplementServerRenderedComponent,
            ],
        );
        unsupported.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let mut testing = task(
            TaskKind::VerificationIncrement,
            vec![ImplementationAction::AddOrUpdateTests],
        );
        testing.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let prose = code_reference_selection_for_task(&specialized_baseline, &prose_only).unwrap();
        let unsupported_selection =
            code_reference_selection_for_task(&react18_baseline, &unsupported).unwrap();
        let tests = code_reference_selection_for_task(&react18_baseline, &testing).unwrap();

        for specialized in [
            "hooks",
            "state",
            "performance",
            "server-components",
            "react19",
            "migration",
            "testing",
        ] {
            assert!(!prose.reference_groups["react"].contains(&specialized.to_string()));
        }
        assert!(!unsupported_selection.reference_groups["react"]
            .contains(&"server-components".to_string()));
        assert!(!unsupported_selection.reference_groups["react"].contains(&"react19".to_string()));
        assert!(tests.reference_groups["react"].contains(&"testing".to_string()));
        assert!(!tests.reference_groups["react"].contains(&"core".to_string()));
        assert!(!tests.reference_groups["react"].contains(&"hooks".to_string()));
        assert!(!tests.reference_groups["react"].contains(&"state".to_string()));
    }

    #[test]
    fn maps_nextjs_typescript_to_nextjs_react_and_typescript_refs() {
        let baseline = baseline(json!({
            "tracks": {
                "web": {"selection": "Next.js App Router + TypeScript"}
            }
        }));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![
                ImplementationAction::CreateOrUpdateUiFlow,
                ImplementationAction::CreateOrUpdateFrontendNavigation,
            ],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        task.objective = "Implement the accepted dashboard route surface.".to_string();
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups.contains_key("nextjs"));
        assert!(selection.reference_groups["nextjs"].contains(&"core".to_string()));
        assert!(selection.reference_groups["nextjs"].contains(&"app-router".to_string()));
        assert!(!selection.reference_groups["nextjs"].contains(&"testing".to_string()));
        assert!(selection.reference_groups.contains_key("react"));
        assert!(selection.reference_groups.contains_key("typescript"));
        assert!(!selection.reference_groups.contains_key("vue"));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.next.app-router" && item.path == "tech/frontend/nextjs/app-router.md"
        }));
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.react.core" && item.path == "tech/frontend/react/core.md"
        }));
        assert!(load_plan
            .iter()
            .any(|item| item.path == "tech/code/typescript/core.md"));
    }

    #[test]
    fn nextjs_specialized_refs_are_task_scoped() {
        let baseline = baseline(json!({
            "tracks": {
                "web": {"selection": "Next.js App Router + TypeScript"}
            }
        }));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![
                ImplementationAction::CreateOrUpdateUiFlow,
                ImplementationAction::ImplementServerMutation,
                ImplementationAction::ImplementServerRenderedComponent,
                ImplementationAction::ImplementReactiveClientFlow,
                ImplementationAction::OptimizeFrontendPerformance,
            ],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        task.objective = "Implement the accepted server-backed Next.js workflow.".to_string();
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        let next_refs = &selection.reference_groups["nextjs"];
        assert!(next_refs.contains(&"actions".to_string()));
        assert!(next_refs.contains(&"data".to_string()));
        assert!(next_refs.contains(&"server-components".to_string()));
        assert!(next_refs.contains(&"runtime".to_string()));
        assert!(!selection.reference_groups["react"].contains(&"server-components".to_string()));
        assert!(!selection.reference_groups["react"].contains(&"performance".to_string()));
        assert!(!selection.reference_groups["react"].contains(&"testing".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.next.actions" && item.path == "tech/frontend/nextjs/actions.md"
        }));
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.next.data" && item.path == "tech/frontend/nextjs/data.md"
        }));
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.next.runtime" && item.path == "tech/frontend/nextjs/runtime.md"
        }));
    }

    #[test]
    fn nextjs_specialized_references_ignore_prose_and_require_owned_capabilities() {
        let app_router_baseline = baseline(json!({
            "tracks": {"web": {"selection": "Next.js App Router + TypeScript"}}
        }));
        let generic_baseline = baseline(json!({
            "tracks": {"web": {"selection": "Next.js + TypeScript"}}
        }));
        let mut prose_only = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        prose_only.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        prose_only.objective = "Add App Router layouts, Server Components, Server Actions, cached fetch, runtime config, and tests.".to_string();
        let mut unsupported_server = task(
            TaskKind::UiFlowIncrement,
            vec![
                ImplementationAction::CreateOrUpdateUiFlow,
                ImplementationAction::CreateOrUpdateFrontendNavigation,
                ImplementationAction::ImplementServerRenderedComponent,
                ImplementationAction::ImplementServerMutation,
            ],
        );
        unsupported_server.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let mut testing = task(
            TaskKind::VerificationIncrement,
            vec![ImplementationAction::AddOrUpdateTests],
        );
        testing.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));

        let prose = code_reference_selection_for_task(&app_router_baseline, &prose_only).unwrap();
        let unsupported =
            code_reference_selection_for_task(&generic_baseline, &unsupported_server).unwrap();
        let tests = code_reference_selection_for_task(&generic_baseline, &testing).unwrap();

        for specialized in [
            "app-router",
            "actions",
            "server-components",
            "data",
            "runtime",
            "testing",
        ] {
            assert!(!prose.reference_groups["nextjs"].contains(&specialized.to_string()));
        }
        assert!(!unsupported.reference_groups["nextjs"].contains(&"app-router".to_string()));
        assert!(!unsupported.reference_groups["nextjs"].contains(&"actions".to_string()));
        assert!(!unsupported.reference_groups["nextjs"].contains(&"server-components".to_string()));
        assert!(tests.reference_groups["nextjs"].contains(&"testing".to_string()));
        assert!(!tests.reference_groups["nextjs"].contains(&"core".to_string()));
    }

    #[test]
    fn maps_vue_typescript_to_vue_and_typescript_refs() {
        let baseline = baseline(json!({
            "tracks": {
                "web": {"selection": "Vue 3 + TypeScript"}
            }
        }));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![
                ImplementationAction::CreateOrUpdateUiFlow,
                ImplementationAction::ImplementSharedClientState,
            ],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        task.objective = "Implement the accepted Vue purchase request workflow.".to_string();
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups.contains_key("vue"));
        assert!(selection.reference_groups["vue"].contains(&"core".to_string()));
        assert!(selection.reference_groups["vue"].contains(&"components".to_string()));
        assert!(selection.reference_groups["vue"].contains(&"state".to_string()));
        assert!(selection.reference_groups["vue"].contains(&"typescript".to_string()));
        assert!(!selection.reference_groups["vue"].contains(&"testing".to_string()));
        assert!(selection.reference_groups.contains_key("typescript"));
        assert!(!selection.reference_groups.contains_key("react"));
        assert!(!selection.reference_groups.contains_key("nextjs"));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.vue.components" && item.path == "tech/frontend/vue/components.md"
        }));
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.vue.typescript" && item.path == "tech/frontend/vue/typescript.md"
        }));
    }

    #[test]
    fn maps_framework_only_vue_without_assuming_typescript() {
        let baseline = baseline(json!({"tracks": {"web": {"selection": "Vue"}}}));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups.contains_key("vue"));
        assert!(!selection.reference_groups["vue"].contains(&"typescript".to_string()));
        assert!(!selection.reference_groups.contains_key("typescript"));
        assert!(selection.unmapped_signals.is_empty());
    }

    #[test]
    fn nuxt_specialized_refs_are_task_scoped_to_vue() {
        let baseline = baseline(json!({
            "tracks": {
                "web": {"selection": "Nuxt 3 + Quasar + Capacitor + PWA + TypeScript"}
            }
        }));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![
                ImplementationAction::CreateOrUpdateUiFlow,
                ImplementationAction::CreateOrUpdateFrontendNavigation,
                ImplementationAction::ImplementServerRenderedComponent,
                ImplementationAction::AddOrUpdateConfig,
                ImplementationAction::ImplementMobilePlatformBehavior,
            ],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        task.objective = "Implement the accepted Nuxt hybrid application boundary.".to_string();
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        let vue_refs = &selection.reference_groups["vue"];
        assert!(vue_refs.contains(&"nuxt".to_string()));
        assert!(vue_refs.contains(&"build".to_string()));
        assert!(vue_refs.contains(&"mobile".to_string()));
        assert!(vue_refs.contains(&"typescript".to_string()));
        assert!(!selection.reference_groups.contains_key("nextjs"));
        assert!(!selection.reference_groups.contains_key("react"));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan
            .iter()
            .any(|item| item.ref_id == "fe.vue.nuxt" && item.path == "tech/frontend/vue/nuxt.md"));
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.vue.build" && item.path == "tech/frontend/vue/build.md"
        }));
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.vue.mobile" && item.path == "tech/frontend/vue/mobile.md"
        }));
    }

    #[test]
    fn vue_specialized_references_ignore_prose_and_require_owned_capabilities() {
        let specialized_baseline = baseline(json!({
            "tracks": {"web": {"selection": "Nuxt 3 + Quasar + Capacitor + PWA + TypeScript"}}
        }));
        let vue_baseline = baseline(json!({
            "tracks": {"web": {"selection": "Vue 3 + TypeScript"}}
        }));
        let mut prose_only = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        prose_only.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        prose_only.objective =
            "Add Pinia, Nuxt SSR routes, Vite optimization, Capacitor PWA behavior, and tests."
                .to_string();
        let mut unsupported_mobile = task(
            TaskKind::UiFlowIncrement,
            vec![
                ImplementationAction::CreateOrUpdateUiFlow,
                ImplementationAction::ImplementMobilePlatformBehavior,
            ],
        );
        unsupported_mobile.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let mut testing = task(
            TaskKind::VerificationIncrement,
            vec![ImplementationAction::AddOrUpdateTests],
        );
        testing.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let server_mutation = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::ImplementServerMutation],
        );
        let performance = task(
            TaskKind::RefactorSupport,
            vec![ImplementationAction::OptimizeFrontendPerformance],
        );

        let prose = code_reference_selection_for_task(&specialized_baseline, &prose_only).unwrap();
        let unsupported =
            code_reference_selection_for_task(&vue_baseline, &unsupported_mobile).unwrap();
        let tests = code_reference_selection_for_task(&vue_baseline, &testing).unwrap();
        let server =
            code_reference_selection_for_task(&specialized_baseline, &server_mutation).unwrap();
        let performance_selection =
            code_reference_selection_for_task(&vue_baseline, &performance).unwrap();

        for specialized in ["state", "nuxt", "build", "mobile", "testing"] {
            assert!(!prose.reference_groups["vue"].contains(&specialized.to_string()));
        }
        assert!(!unsupported.reference_groups["vue"].contains(&"mobile".to_string()));
        assert!(tests.reference_groups["vue"].contains(&"testing".to_string()));
        assert!(!tests.reference_groups["vue"].contains(&"core".to_string()));
        assert!(!tests.reference_groups["vue"].contains(&"components".to_string()));
        assert!(!tests.reference_groups["vue"].contains(&"typescript".to_string()));
        assert!(server.reference_groups["vue"].contains(&"nuxt".to_string()));
        assert!(!server.reference_groups["vue"].contains(&"core".to_string()));
        assert!(!server.reference_groups["vue"].contains(&"components".to_string()));
        assert!(!server.reference_groups["vue"].contains(&"typescript".to_string()));
        assert!(performance_selection.reference_groups["vue"].contains(&"core".to_string()));
        assert!(!performance_selection.reference_groups["vue"].contains(&"build".to_string()));
    }

    #[test]
    fn maps_angular_typescript_to_task_scoped_angular_refs() {
        let baseline = baseline(json!({
            "tracks": {
                "web": {"selection": "Angular 17 + NgRx + TypeScript"}
            }
        }));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![
                ImplementationAction::CreateOrUpdateUiFlow,
                ImplementationAction::CreateOrUpdateFrontendNavigation,
                ImplementationAction::ImplementReactiveClientFlow,
                ImplementationAction::ImplementSharedClientState,
            ],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        task.objective = "Implement the accepted purchase approval frontend workflow.".to_string();
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups.contains_key("angular"));
        assert!(selection.reference_groups.contains_key("typescript"));
        let angular_refs = &selection.reference_groups["angular"];
        assert!(angular_refs.contains(&"core".to_string()));
        assert!(angular_refs.contains(&"components".to_string()));
        assert!(angular_refs.contains(&"routing".to_string()));
        assert!(angular_refs.contains(&"rxjs".to_string()));
        assert!(angular_refs.contains(&"ngrx".to_string()));
        assert!(!angular_refs.contains(&"testing".to_string()));
        assert!(!selection.reference_groups.contains_key("react"));
        assert!(!selection.reference_groups.contains_key("nextjs"));
        assert!(!selection.reference_groups.contains_key("vue"));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.angular.routing" && item.path == "tech/frontend/angular/routing.md"
        }));
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.angular.ngrx" && item.path == "tech/frontend/angular/ngrx.md"
        }));
    }

    #[test]
    fn framework_only_angular_does_not_assume_typescript() {
        let baseline = baseline(json!({"tracks": {"web": {"selection": "Angular"}}}));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups.contains_key("angular"));
        assert!(!selection.reference_groups.contains_key("typescript"));
        assert!(selection.unmapped_signals.is_empty());
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.angular.components"
                && item.path == "tech/frontend/angular/components.md"
        }));
    }

    #[test]
    fn angular_specialized_references_ignore_prose_and_require_owned_capabilities() {
        let angular_baseline = baseline(json!({
            "tracks": {"web": {"selection": "Angular 17 + TypeScript"}}
        }));
        let ngrx_baseline = baseline(json!({
            "tracks": {"web": {"selection": "Angular 17 + NgRx + TypeScript"}}
        }));
        let mut prose_only = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        prose_only.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        prose_only.objective =
            "Add Angular routes, RxJS streams, resolvers, selectors, effects, and an NgRx store."
                .to_string();
        let mut state_without_ngrx = task(
            TaskKind::UiFlowIncrement,
            vec![
                ImplementationAction::CreateOrUpdateUiFlow,
                ImplementationAction::ImplementSharedClientState,
            ],
        );
        state_without_ngrx.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let mut testing = task(
            TaskKind::VerificationIncrement,
            vec![ImplementationAction::AddOrUpdateTests],
        );
        testing.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));

        let prose = code_reference_selection_for_task(&ngrx_baseline, &prose_only).unwrap();
        let state =
            code_reference_selection_for_task(&angular_baseline, &state_without_ngrx).unwrap();
        let tests = code_reference_selection_for_task(&angular_baseline, &testing).unwrap();

        for specialized in ["routing", "rxjs", "ngrx", "testing"] {
            assert!(!prose.reference_groups["angular"].contains(&specialized.to_string()));
        }
        assert!(!state.reference_groups["angular"].contains(&"ngrx".to_string()));
        assert!(tests.reference_groups["angular"].contains(&"testing".to_string()));
        assert!(!tests.reference_groups["angular"].contains(&"core".to_string()));
        assert!(!tests.reference_groups["angular"].contains(&"components".to_string()));
        assert!(!tests.reference_groups["angular"].contains(&"routing".to_string()));
        assert!(!tests.reference_groups["angular"].contains(&"rxjs".to_string()));
    }

    #[test]
    fn maps_react_native_typescript_without_web_react_refs() {
        let baseline = baseline(json!({
            "tracks": {
                "app": {"selection": "React Native + Expo + TypeScript"}
            }
        }));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![
                ImplementationAction::CreateOrUpdateUiFlow,
                ImplementationAction::CreateOrUpdateFrontendNavigation,
                ImplementationAction::OptimizeFrontendPerformance,
                ImplementationAction::ImplementMobilePlatformBehavior,
                ImplementationAction::ImplementClientStorage,
                ImplementationAction::ImplementFrontendExperienceContract,
            ],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        task.objective = "Implement the accepted mobile purchase request workflow.".to_string();
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups.contains_key("reactnative"));
        assert!(selection.reference_groups.contains_key("typescript"));
        let rn_refs = &selection.reference_groups["reactnative"];
        assert!(rn_refs.contains(&"core".to_string()));
        assert!(rn_refs.contains(&"structure".to_string()));
        assert!(rn_refs.contains(&"navigation".to_string()));
        assert!(rn_refs.contains(&"platform".to_string()));
        assert!(rn_refs.contains(&"lists".to_string()));
        assert!(rn_refs.contains(&"storage".to_string()));
        assert!(!rn_refs.contains(&"testing".to_string()));
        assert!(!selection.reference_groups.contains_key("react"));
        assert!(!selection.reference_groups.contains_key("nextjs"));
        assert!(!selection.reference_groups.contains_key("vue"));
        assert!(!selection.reference_groups.contains_key("angular"));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.rn.navigation"
                && item.path == "tech/frontend/react-native/navigation.md"
        }));
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.rn.platform" && item.path == "tech/frontend/react-native/platform.md"
        }));
        assert!(load_plan
            .iter()
            .any(|item| item.path == "tech/code/typescript/core.md"));
    }

    #[test]
    fn framework_only_react_native_does_not_assume_typescript_or_web_react() {
        let baseline =
            baseline(json!({"tracks": {"app": {"selection": "Expo Router + React Native"}}}));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![
                ImplementationAction::CreateOrUpdateUiFlow,
                ImplementationAction::CreateOrUpdateFrontendNavigation,
            ],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups.contains_key("reactnative"));
        assert!(!selection.reference_groups.contains_key("typescript"));
        assert!(!selection.reference_groups.contains_key("react"));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.rn.navigation"
                && item.path == "tech/frontend/react-native/navigation.md"
        }));
    }

    #[test]
    fn react_native_specialized_references_ignore_prose_and_require_owned_capabilities() {
        let baseline =
            baseline(json!({"tracks": {"app": {"selection": "Expo Router + React Native"}}}));
        let mut prose_only = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        prose_only.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        prose_only.objective = "Add Expo routes, safe areas, keyboard handling, FlatList performance, MMKV persistence, and tests.".to_string();
        let mut testing = task(
            TaskKind::VerificationIncrement,
            vec![ImplementationAction::AddOrUpdateTests],
        );
        testing.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let storage = task(
            TaskKind::ConfigurationSupport,
            vec![ImplementationAction::ImplementClientStorage],
        );
        let platform = task(
            TaskKind::RefactorSupport,
            vec![ImplementationAction::ImplementMobilePlatformBehavior],
        );

        let prose = code_reference_selection_for_task(&baseline, &prose_only).unwrap();
        let tests = code_reference_selection_for_task(&baseline, &testing).unwrap();
        let storage_selection = code_reference_selection_for_task(&baseline, &storage).unwrap();
        let platform_selection = code_reference_selection_for_task(&baseline, &platform).unwrap();

        assert!(prose.reference_groups["reactnative"].contains(&"core".to_string()));
        for specialized in [
            "structure",
            "navigation",
            "platform",
            "lists",
            "storage",
            "testing",
        ] {
            assert!(!prose.reference_groups["reactnative"].contains(&specialized.to_string()));
        }
        assert!(tests.reference_groups["reactnative"].contains(&"testing".to_string()));
        assert!(!tests.reference_groups["reactnative"].contains(&"core".to_string()));
        assert!(!tests.reference_groups["reactnative"].contains(&"structure".to_string()));
        assert!(!tests.reference_groups["reactnative"].contains(&"platform".to_string()));
        assert!(storage_selection.reference_groups["reactnative"].contains(&"storage".to_string()));
        assert!(
            storage_selection.reference_groups["reactnative"].contains(&"structure".to_string())
        );
        assert!(!storage_selection.reference_groups["reactnative"].contains(&"core".to_string()));
        assert!(
            platform_selection.reference_groups["reactnative"].contains(&"platform".to_string())
        );
        assert!(!platform_selection.reference_groups["reactnative"].contains(&"core".to_string()));
    }

    #[test]
    fn maps_flutter_stack_to_task_scoped_flutter_refs() {
        let baseline = baseline(json!({
            "tracks": {
                "app": {"selection": "Flutter 3 + Riverpod + GoRouter"}
            }
        }));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![
                ImplementationAction::CreateOrUpdateUiFlow,
                ImplementationAction::CreateOrUpdateFrontendNavigation,
                ImplementationAction::ImplementSharedClientState,
                ImplementationAction::OptimizeFrontendPerformance,
                ImplementationAction::ImplementFrontendExperienceContract,
            ],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        task.objective = "Implement the accepted purchase approval Flutter workflow.".to_string();
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups.contains_key("flutter"));
        let flutter_refs = &selection.reference_groups["flutter"];
        assert!(flutter_refs.contains(&"core".to_string()));
        assert!(flutter_refs.contains(&"structure".to_string()));
        assert!(flutter_refs.contains(&"widgets".to_string()));
        assert!(flutter_refs.contains(&"navigation".to_string()));
        assert!(flutter_refs.contains(&"riverpod".to_string()));
        assert!(flutter_refs.contains(&"performance".to_string()));
        assert!(!flutter_refs.contains(&"testing".to_string()));
        assert!(!flutter_refs.contains(&"bloc".to_string()));
        assert!(!selection.reference_groups.contains_key("typescript"));
        assert!(!selection.reference_groups.contains_key("react"));
        assert!(!selection.reference_groups.contains_key("reactnative"));
        assert!(!selection.reference_groups.contains_key("kotlin"));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.flutter.navigation"
                && item.path == "tech/frontend/flutter/navigation.md"
        }));
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.flutter.riverpod" && item.path == "tech/frontend/flutter/riverpod.md"
        }));
    }

    #[test]
    fn flutter_bloc_reference_is_explicitly_scoped() {
        let baseline = baseline(json!({
            "tracks": {
                "app": {"selection": "Flutter 3 + Bloc"}
            }
        }));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![
                ImplementationAction::CreateOrUpdateUiFlow,
                ImplementationAction::ImplementSharedClientState,
            ],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        task.objective = "Implement the accepted multi-step Flutter workflow.".to_string();
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        let flutter_refs = &selection.reference_groups["flutter"];
        assert!(flutter_refs.contains(&"bloc".to_string()));
        assert!(flutter_refs.contains(&"widgets".to_string()));
        assert!(!flutter_refs.contains(&"testing".to_string()));
        assert!(!flutter_refs.contains(&"riverpod".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan
            .iter()
            .any(|item| item.ref_id == "fe.flutter.bloc"
                && item.path == "tech/frontend/flutter/bloc.md"));
    }

    #[test]
    fn framework_only_flutter_does_not_assume_dart_language_refs() {
        let baseline = baseline(json!({"tracks": {"app": {"selection": "Flutter"}}}));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        task.objective =
            "Build a Flutter form that shows a business-blocking validation message.".to_string();
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups.contains_key("flutter"));
        assert!(!selection.reference_groups.contains_key("typescript"));
        assert!(!selection.reference_groups.contains_key("javascript"));
        assert!(!selection.reference_groups.contains_key("kotlin"));
        assert!(!selection.reference_groups["flutter"].contains(&"bloc".to_string()));
        assert!(!selection.reference_groups["flutter"].contains(&"structure".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.flutter.widgets" && item.path == "tech/frontend/flutter/widgets.md"
        }));
    }

    #[test]
    fn flutter_specialized_references_ignore_prose_and_require_owned_capabilities() {
        let selected_baseline = baseline(json!({
            "tracks": {"app": {"selection": "Flutter 3 + Riverpod + GoRouter"}}
        }));
        let plain_baseline = baseline(json!({
            "tracks": {"app": {"selection": "Flutter 3"}}
        }));
        let mut prose_only = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        prose_only.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        prose_only.objective = "Add GoRouter navigation, Riverpod providers, Bloc state, profile list rebuilds, and widget tests.".to_string();
        let mut shared_without_library = task(
            TaskKind::UiFlowIncrement,
            vec![
                ImplementationAction::CreateOrUpdateUiFlow,
                ImplementationAction::ImplementSharedClientState,
            ],
        );
        shared_without_library.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let mut testing = task(
            TaskKind::VerificationIncrement,
            vec![ImplementationAction::AddOrUpdateTests],
        );
        testing.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));

        let prose = code_reference_selection_for_task(&selected_baseline, &prose_only).unwrap();
        let shared =
            code_reference_selection_for_task(&plain_baseline, &shared_without_library).unwrap();
        let tests = code_reference_selection_for_task(&plain_baseline, &testing).unwrap();

        for specialized in [
            "navigation",
            "riverpod",
            "bloc",
            "performance",
            "structure",
            "testing",
        ] {
            assert!(!prose.reference_groups["flutter"].contains(&specialized.to_string()));
        }
        assert!(!shared.reference_groups["flutter"].contains(&"riverpod".to_string()));
        assert!(!shared.reference_groups["flutter"].contains(&"bloc".to_string()));
        assert!(tests.reference_groups["flutter"].contains(&"testing".to_string()));
        assert!(!tests.reference_groups["flutter"].contains(&"core".to_string()));
        assert!(!tests.reference_groups["flutter"].contains(&"widgets".to_string()));
    }

    #[test]
    fn dart_alone_stays_unmapped_without_blocking() {
        let baseline = baseline(json!({"tracks": {"app": {"selection": "Dart"}}}));
        let seed = build_code_quality_seed(&baseline);
        assert_eq!(seed["required"], false);
        assert_eq!(seed["unmappedSignals"][0]["confidence"], "low");
        assert!(seed.get("techReferenceProfile").is_none());
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
            assert!(
                package_naming_policy_for_reference_groups(&selection.reference_groups).is_none(),
                "{raw} should not receive JVM package naming policy"
            );
        }
    }

    #[test]
    fn csharp_references_are_capability_scoped_without_aspnet_duplication() {
        let aspnet_baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": ".NET 8 + ASP.NET Core Minimal APIs"},
                "dataAccess": {"selection": "Entity Framework Core"}
            }
        }));
        let worker_baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": ".NET 8 C# worker"},
                "dataAccess": {"selection": "Entity Framework Core"}
            }
        }));
        let blazor_baseline = baseline(json!({
            "tracks": {"web": {"selection": ".NET 8 + ASP.NET Core + Blazor WebAssembly + C#"}}
        }));
        let api_task = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::CreateOrUpdateInterface],
        );
        let persistence_task = task(
            TaskKind::DataModelIncrement,
            vec![ImplementationAction::CreateOrUpdatePersistence],
        );
        let mut blazor_task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        blazor_task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let mut blazor_testing_task = task(
            TaskKind::VerificationIncrement,
            vec![ImplementationAction::AddOrUpdateTests],
        );
        blazor_testing_task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));

        let api = code_reference_selection_for_task(&aspnet_baseline, &api_task).unwrap();
        let aspnet_data =
            code_reference_selection_for_task(&aspnet_baseline, &persistence_task).unwrap();
        let worker_data =
            code_reference_selection_for_task(&worker_baseline, &persistence_task).unwrap();
        let blazor = code_reference_selection_for_task(&blazor_baseline, &blazor_task).unwrap();
        let blazor_testing =
            code_reference_selection_for_task(&blazor_baseline, &blazor_testing_task).unwrap();

        assert!(api.reference_groups["csharp"].contains(&"core".to_string()));
        assert!(!api.reference_groups["csharp"].contains(&"aspnet".to_string()));
        assert!(api.reference_groups["aspnetcore"].contains(&"minimal".to_string()));
        assert!(aspnet_data.reference_groups["aspnetcore"].contains(&"data".to_string()));
        assert!(!aspnet_data.reference_groups["csharp"].contains(&"persistence".to_string()));
        assert!(worker_data.reference_groups["csharp"].contains(&"persistence".to_string()));
        assert!(!worker_data.reference_groups.contains_key("aspnetcore"));
        assert!(blazor.reference_groups["csharp"].contains(&"core".to_string()));
        assert!(blazor.reference_groups["csharp"].contains(&"blazor".to_string()));
        assert!(blazor_testing.reference_groups["csharp"].contains(&"testing".to_string()));
        assert!(!blazor_testing
            .reference_groups
            .get("aspnetcore")
            .is_some_and(|items| items.contains(&"testing".to_string())));
    }

    #[test]
    fn csharp_specialized_references_ignore_prose_and_require_owned_capabilities() {
        let baseline = baseline(json!({
            "tracks": {"backend": {"selection": ".NET 8 + C# 12"}}
        }));
        let owned = task(
            TaskKind::RefactorSupport,
            vec![
                ImplementationAction::ImplementLanguageVersionFeature,
                ImplementationAction::OptimizeRuntimePerformance,
            ],
        );
        let mut prose_only = task(
            TaskKind::FeatureIncrement,
            vec![ImplementationAction::CreateOrUpdateBusinessRule],
        );
        prose_only.objective =
            "Use C# 12 primary constructors, Span, ArrayPool, BenchmarkDotNet, and tests."
                .to_string();
        let testing = task(
            TaskKind::VerificationIncrement,
            vec![ImplementationAction::AddOrUpdateTests],
        );

        let selected = code_reference_selection_for_task(&baseline, &owned).unwrap();
        let prose = code_reference_selection_for_task(&baseline, &prose_only).unwrap();
        let tests = code_reference_selection_for_task(&baseline, &testing).unwrap();

        assert!(selected.reference_groups["csharp"].contains(&"core".to_string()));
        assert!(selected.reference_groups["csharp"].contains(&"modern".to_string()));
        assert!(selected.reference_groups["csharp"].contains(&"performance".to_string()));
        assert_eq!(prose.reference_groups["csharp"], vec!["core".to_string()]);
        assert_eq!(
            tests.reference_groups["csharp"],
            vec!["testing".to_string()]
        );
    }

    #[test]
    fn go_references_are_task_scoped_without_api_or_prose_defaults() {
        let baseline = baseline(json!({
            "tracks": {"backend": {"selection": "Go 1.23 + Gin"}}
        }));
        let api = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::CreateOrUpdateInterface],
        );
        let owned = task(
            TaskKind::RefactorSupport,
            vec![
                ImplementationAction::ImplementAsyncProcessing,
                ImplementationAction::ImplementGenericTypeAbstraction,
                ImplementationAction::ImplementDependencyAbstraction,
                ImplementationAction::RefactorModuleStructure,
            ],
        );
        let mut prose_only = task(
            TaskKind::FeatureIncrement,
            vec![ImplementationAction::CreateOrUpdateBusinessRule],
        );
        prose_only.objective = "Add goroutines, channels, worker pools, interfaces, generics, cmd/internal structure, fuzzing, and tests.".to_string();
        let testing = task(
            TaskKind::VerificationIncrement,
            vec![ImplementationAction::AddOrUpdateTests],
        );
        let config = task(
            TaskKind::ConfigurationSupport,
            vec![ImplementationAction::AddOrUpdateConfig],
        );

        let api_selection = code_reference_selection_for_task(&baseline, &api).unwrap();
        let selected = code_reference_selection_for_task(&baseline, &owned).unwrap();
        let prose = code_reference_selection_for_task(&baseline, &prose_only).unwrap();
        let tests = code_reference_selection_for_task(&baseline, &testing).unwrap();
        let structure = code_reference_selection_for_task(&baseline, &config).unwrap();

        assert_eq!(
            api_selection.reference_groups["go"],
            vec!["core".to_string()]
        );
        for expected in ["core", "concurrency", "generics", "interfaces", "structure"] {
            assert!(selected.reference_groups["go"].contains(&expected.to_string()));
        }
        assert_eq!(prose.reference_groups["go"], vec!["core".to_string()]);
        assert_eq!(tests.reference_groups["go"], vec!["testing".to_string()]);
        assert_eq!(
            structure.reference_groups["go"],
            vec!["structure".to_string()]
        );
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
        assert!(selection.reference_groups["cpp"].contains(&"core".to_string()));
        assert!(!selection.reference_groups["cpp"].contains(&"build".to_string()));
        assert!(!selection.reference_groups["cpp"].contains(&"modern".to_string()));
    }

    #[test]
    fn cpp_specialized_references_require_explicit_task_ownership() {
        let baseline = baseline(json!({
            "tracks": {"backend": {"selection": "C++23 + CMake + Clang"}}
        }));
        let mut owned = task(
            TaskKind::FeatureIncrement,
            vec![
                ImplementationAction::CreateOrUpdateBusinessRule,
                ImplementationAction::ImplementLanguageVersionFeature,
                ImplementationAction::ImplementGenericTypeAbstraction,
                ImplementationAction::OptimizeRuntimePerformance,
                ImplementationAction::ImplementAsyncProcessing,
            ],
        );
        owned.objective = "Implement the accepted C++ processing boundary.".to_string();
        let mut prose_only = task(
            TaskKind::FeatureIncrement,
            vec![ImplementationAction::CreateOrUpdateBusinessRule],
        );
        prose_only.objective = "Use C++23 concepts, templates, coroutines, SIMD performance, CMake, sanitizers, and tests.".to_string();
        let testing = task(
            TaskKind::VerificationIncrement,
            vec![ImplementationAction::AddOrUpdateTests],
        );
        let config = task(
            TaskKind::ConfigurationSupport,
            vec![ImplementationAction::AddOrUpdateConfig],
        );

        let selected = code_reference_selection_for_task(&baseline, &owned).unwrap();
        let prose = code_reference_selection_for_task(&baseline, &prose_only).unwrap();
        let tests = code_reference_selection_for_task(&baseline, &testing).unwrap();
        let build = code_reference_selection_for_task(&baseline, &config).unwrap();

        for expected in [
            "core",
            "modern",
            "build",
            "templates",
            "performance",
            "concurrency",
        ] {
            assert!(selected.reference_groups["cpp"].contains(&expected.to_string()));
        }
        assert_eq!(prose.reference_groups["cpp"], vec!["core".to_string()]);
        assert_eq!(tests.reference_groups["cpp"], vec!["testing".to_string()]);
        assert_eq!(build.reference_groups["cpp"], vec!["build".to_string()]);
    }

    #[test]
    fn maps_task_specific_language_reference_groups() {
        let cpp_baseline = baseline(json!({"tracks": {"backend": {"selection": "C++20 + CMake"}}}));
        let mut cpp_task = task(
            TaskKind::FeatureIncrement,
            vec![
                ImplementationAction::CreateOrUpdateBusinessRule,
                ImplementationAction::ImplementGenericTypeAbstraction,
            ],
        );
        cpp_task.objective = "Implement the accepted reusable rules registry.".to_string();
        let cpp_selection = code_reference_selection_for_task(&cpp_baseline, &cpp_task).unwrap();
        assert!(cpp_selection.reference_groups["cpp"].contains(&"templates".to_string()));

        let go_baseline = baseline(json!({"tracks": {"backend": {"selection": "Go + Gin"}}}));
        let mut go_task = task(
            TaskKind::FeatureIncrement,
            vec![
                ImplementationAction::CreateOrUpdateBusinessRule,
                ImplementationAction::ImplementGenericTypeAbstraction,
            ],
        );
        go_task.objective = "Implement the accepted collection helper contract.".to_string();
        let go_selection = code_reference_selection_for_task(&go_baseline, &go_task).unwrap();
        assert!(go_selection.reference_groups["go"].contains(&"generics".to_string()));

        let swift_baseline = baseline(json!({"tracks": {"app": {"selection": "Swift + SwiftUI"}}}));
        let mut swift_task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        swift_task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        swift_task.objective = "Fix SwiftUI detail flow state.".to_string();
        swift_task
            .implementation_actions
            .push(ImplementationAction::OptimizeRuntimePerformance);
        let swift_selection =
            code_reference_selection_for_task(&swift_baseline, &swift_task).unwrap();
        assert!(swift_selection.reference_groups["swift"].contains(&"memory".to_string()));

        let sql_baseline =
            baseline(json!({"tracks": {"persistence": {"selection": "PostgreSQL"}}}));
        let mut sql_task = task(
            TaskKind::FeatureIncrement,
            vec![ImplementationAction::ImplementAnalyticalQuery],
        );
        sql_task.objective =
            "Add reporting query with window function ranking and running total.".to_string();
        let sql_selection = code_reference_selection_for_task(&sql_baseline, &sql_task).unwrap();
        assert!(sql_selection.reference_groups["sql"].contains(&"queries".to_string()));
        assert!(sql_selection.reference_groups["sql"].contains(&"windows".to_string()));
        assert!(sql_selection.reference_groups["sql"].contains(&"dialects".to_string()));
        assert!(sql_selection.reference_groups["sql"].contains(&"postgresql.queries".to_string()));
    }

    #[test]
    fn sql_reference_selection_does_not_infer_subject_from_prose() {
        let baseline = baseline(json!({"tracks": {"persistence": {"selection": "PostgreSQL"}}}));
        let mut prose_only = task(
            TaskKind::FeatureIncrement,
            vec![ImplementationAction::CreateOrUpdateBusinessRule],
        );
        prose_only.objective =
            "Add a reporting query with a window function and running total.".to_string();

        assert!(code_reference_selection_for_task(&baseline, &prose_only).is_none());
    }

    #[test]
    fn java_security_reference_is_task_scoped() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Java + Spring Boot"},
                "dataAccess": {"selection": "Spring Data JPA"}
            }
        }));
        let security_task = task(
            TaskKind::InterfaceIncrement,
            vec![
                ImplementationAction::CreateOrUpdateInterface,
                ImplementationAction::ImplementAuthenticationOrAuthorization,
            ],
        );
        let selection = code_reference_selection_for_task(&baseline, &security_task).unwrap();
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

        let mut prose_only = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::CreateOrUpdateInterface],
        );
        prose_only.objective =
            "Add JWT login and role-based permission checks to the endpoint.".to_string();
        let prose_selection = code_reference_selection_for_task(&baseline, &prose_only).unwrap();
        assert!(!prose_selection.reference_groups["java"].contains(&"security".to_string()));
        assert!(!prose_selection.reference_groups["springboot"].contains(&"security".to_string()));
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
        assert!(!selection.reference_groups["springboot"].contains(&"testing".to_string()));
        assert!(!selection.reference_groups["springboot"].contains(&"data".to_string()));
        assert!(!selection.reference_groups["java"].contains(&"reactive".to_string()));
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
        assert!(!load_plan
            .iter()
            .any(|item| item.path == "tech/backend/springboot/runtime.md"));
    }

    #[test]
    fn spring_boot_testing_reference_requires_test_implementation_ownership() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Java + Spring Boot"}
            }
        }));
        let test_task = task(
            TaskKind::VerificationIncrement,
            vec![ImplementationAction::AddOrUpdateTests],
        );

        let selection = code_reference_selection_for_task(&baseline, &test_task).unwrap();

        assert!(selection.reference_groups["java"].contains(&"testing".to_string()));
        assert!(selection.reference_groups["springboot"].contains(&"testing".to_string()));
        assert!(!selection.reference_groups["springboot"].contains(&"web".to_string()));
        assert!(!selection.reference_groups["springboot"].contains(&"data".to_string()));
    }

    #[test]
    fn spring_boot_async_and_cache_task_loads_only_owned_framework_references() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Java + Spring Boot"}
            }
        }));
        let task = task(
            TaskKind::RefactorSupport,
            vec![
                ImplementationAction::ImplementAsyncProcessing,
                ImplementationAction::ImplementCachePolicy,
            ],
        );
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups["springboot"].contains(&"async".to_string()));
        assert!(selection.reference_groups["springboot"].contains(&"cache".to_string()));
        assert!(!selection.reference_groups["springboot"].contains(&"runtime".to_string()));
        assert!(!selection.reference_groups["java"].contains(&"reactive".to_string()));
        assert!(!selection.reference_groups["springboot"].contains(&"testing".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan
            .iter()
            .any(|item| item.path == "tech/backend/springboot/async.md"));
        assert!(load_plan
            .iter()
            .any(|item| item.path == "tech/backend/springboot/cache.md"));
        assert!(!load_plan
            .iter()
            .any(|item| item.path == "tech/code/java/reactive.md"));
    }

    #[test]
    fn structured_task_context_selects_owned_spring_capabilities() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Java + Spring Boot"}
            }
        }));
        let task = task(
            TaskKind::RefactorSupport,
            vec![ImplementationAction::RefactorSupportingCode],
        );
        let context = CodeReferenceTaskContext {
            application_architecture: false,
            security: true,
            async_processing: true,
            integration: true,
            resilience: true,
            observability: true,
        };

        let selection =
            code_reference_selection_for_task_with_context(&baseline, &task, &context).unwrap();
        let spring = &selection.reference_groups["springboot"];

        for expected in [
            "security",
            "async",
            "integration",
            "resilience",
            "observability",
        ] {
            assert!(spring.contains(&expected.to_string()));
        }
        assert!(!spring.contains(&"cloud".to_string()));
    }

    #[test]
    fn spring_boot_reactive_task_loads_reactive_without_runtime() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Java + Spring Boot + WebFlux"}
            }
        }));
        let task = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::CreateOrUpdateInterface],
        );
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups["java"].contains(&"reactive".to_string()));
        assert!(selection.reference_groups["springboot"].contains(&"web".to_string()));
        assert!(!selection.reference_groups["java"].contains(&"persistence".to_string()));
        assert!(!selection.reference_groups["springboot"].contains(&"data".to_string()));
        assert!(!selection.reference_groups["springboot"].contains(&"runtime".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan
            .iter()
            .any(|item| item.path == "tech/code/java/reactive.md"));
    }

    #[test]
    fn project_reactor_does_not_imply_spring_boot_or_webflux() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Java + Project Reactor"}
            }
        }));
        let task = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::CreateOrUpdateInterface],
        );

        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();

        assert!(selection.reference_groups["java"].contains(&"reactive".to_string()));
        assert!(!selection.reference_groups.contains_key("springboot"));
    }

    #[test]
    fn spring_boot_persistence_performance_does_not_load_runtime() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Java + Spring Boot"},
                "dataAccess": {"selection": "Spring Data JPA"},
                "persistence": {"selection": "PostgreSQL"}
            }
        }));
        let mut task = task(
            TaskKind::DataModelIncrement,
            vec![ImplementationAction::OptimizePersistenceQuery],
        );
        task.objective = "Optimize a Spring Data JPA query plan and index path.".to_string();
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups["springboot"].contains(&"data".to_string()));
        assert!(!selection.reference_groups["springboot"].contains(&"runtime".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(!load_plan
            .iter()
            .any(|item| item.path == "tech/backend/springboot/runtime.md"));
    }

    #[test]
    fn spring_data_reference_requires_spring_data_stack_selection() {
        let hibernate_baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Java + Spring Boot"},
                "dataAccess": {"selection": "Hibernate ORM"}
            }
        }));
        let jooq_baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Java + Spring Boot"},
                "dataAccess": {"selection": "jOOQ"}
            }
        }));
        let spring_data_jdbc_baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Java + Spring Boot"},
                "dataAccess": {"selection": "Spring Data JDBC"}
            }
        }));
        let task = task(
            TaskKind::DataModelIncrement,
            vec![ImplementationAction::CreateEntityRepository],
        );

        let hibernate = code_reference_selection_for_task(&hibernate_baseline, &task).unwrap();
        let jooq = code_reference_selection_for_task(&jooq_baseline, &task).unwrap();
        let spring_data_jdbc =
            code_reference_selection_for_task(&spring_data_jdbc_baseline, &task).unwrap();

        assert!(hibernate.reference_groups["java"].contains(&"persistence".to_string()));
        assert!(!hibernate
            .reference_groups
            .get("springboot")
            .is_some_and(|items| items.contains(&"data".to_string())));
        assert!(!jooq.reference_groups["java"].contains(&"persistence".to_string()));
        assert!(!jooq
            .reference_groups
            .get("springboot")
            .is_some_and(|items| items.contains(&"data".to_string())));
        assert!(!spring_data_jdbc.reference_groups["java"].contains(&"persistence".to_string()));
        assert!(!spring_data_jdbc
            .reference_groups
            .get("springboot")
            .is_some_and(|items| items.contains(&"data".to_string())));
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
        assert!(!selection.reference_groups["springboot"].contains(&"testing".to_string()));
        assert!(!selection.reference_groups["springboot"].contains(&"web".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "bk.spring.runtime" && item.path == "tech/backend/springboot/runtime.md"
        }));
    }

    #[test]
    fn spring_boot_integration_resilience_and_cloud_are_independently_scoped() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Java + Spring Boot"},
                "cloud": {"selection": "Spring Cloud"}
            }
        }));
        let task = task(
            TaskKind::IntegrationIncrement,
            vec![
                ImplementationAction::ImplementExternalServiceIntegration,
                ImplementationAction::ImplementResiliencePolicy,
                ImplementationAction::ConfigureServiceRoutingOrDiscovery,
            ],
        );
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups["springboot"].contains(&"integration".to_string()));
        assert!(selection.reference_groups["springboot"].contains(&"resilience".to_string()));
        assert!(selection.reference_groups["springboot"].contains(&"cloud".to_string()));
        assert!(!selection.reference_groups["springboot"].contains(&"web".to_string()));
        assert!(!selection.reference_groups["springboot"].contains(&"testing".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "bk.spring.integration"
                && item.path == "tech/backend/springboot/integration.md"
        }));
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "bk.spring.resilience"
                && item.path == "tech/backend/springboot/resilience.md"
        }));
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "bk.spring.cloud" && item.path == "tech/backend/springboot/cloud.md"
        }));
    }

    #[test]
    fn integration_task_kind_does_not_imply_external_spring_integration() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Java + Spring Boot"}
            }
        }));
        let task = task(
            TaskKind::IntegrationIncrement,
            vec![ImplementationAction::RefactorSupportingCode],
        );

        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();

        assert!(!selection
            .reference_groups
            .get("springboot")
            .is_some_and(|items| items.contains(&"integration".to_string())));
    }

    #[test]
    fn spring_cloud_baseline_does_not_attach_cloud_to_data_task() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Java + Spring Boot + Spring Cloud"},
                "dataAccess": {"selection": "Spring Data JPA"}
            }
        }));
        let task = task(
            TaskKind::DataModelIncrement,
            vec![ImplementationAction::CreateEntityRepository],
        );

        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();

        assert!(selection.reference_groups["springboot"].contains(&"data".to_string()));
        assert!(!selection.reference_groups["springboot"].contains(&"cloud".to_string()));
        assert!(!selection.reference_groups["springboot"].contains(&"integration".to_string()));
    }

    #[test]
    fn non_spring_boot_stacks_never_receive_spring_boot_references() {
        for selection in [
            "Java + Spring Framework",
            "Java + Quarkus",
            "Kotlin + Ktor",
            "C# + ASP.NET Core",
            "Python + Django",
        ] {
            let baseline = baseline(json!({
                "tracks": {"backend": {"selection": selection}}
            }));
            let task = task(
                TaskKind::InterfaceIncrement,
                vec![
                    ImplementationAction::CreateOrUpdateInterface,
                    ImplementationAction::ImplementAuthenticationOrAuthorization,
                    ImplementationAction::ImplementAsyncProcessing,
                    ImplementationAction::ImplementCachePolicy,
                    ImplementationAction::ImplementExternalServiceIntegration,
                    ImplementationAction::ImplementResiliencePolicy,
                    ImplementationAction::ConfigureServiceRoutingOrDiscovery,
                    ImplementationAction::ImplementObservability,
                ],
            );
            let selected = code_reference_selection_for_task(&baseline, &task);
            assert!(selected
                .as_ref()
                .is_none_or(|selected| !selected.reference_groups.contains_key("springboot")));
        }
    }

    #[test]
    fn kotlin_spring_boot_uses_kotlin_and_spring_boot_references() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Kotlin + Spring Boot + Spring WebFlux"}
            }
        }));
        let task = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::CreateOrUpdateInterface],
        );

        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();

        assert!(selection.reference_groups.contains_key("kotlin"));
        assert!(selection.reference_groups.contains_key("springboot"));
        assert!(selection.reference_groups["springboot"].contains(&"web".to_string()));
        assert!(!selection.reference_groups["kotlin"].contains(&"ktor".to_string()));
        assert!(!selection.reference_groups.contains_key("java"));
    }

    #[test]
    fn kotlin_dsl_reference_requires_owned_generic_abstraction() {
        let baseline = baseline(json!({
            "tracks": {"backend": {"selection": "Kotlin + Ktor"}}
        }));
        let ordinary_task = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::CreateOrUpdateInterface],
        );
        let dsl_task = task(
            TaskKind::RefactorSupport,
            vec![ImplementationAction::ImplementGenericTypeAbstraction],
        );

        let ordinary = code_reference_selection_for_task(&baseline, &ordinary_task).unwrap();
        let dsl = code_reference_selection_for_task(&baseline, &dsl_task).unwrap();

        assert!(!ordinary.reference_groups["kotlin"].contains(&"dsl".to_string()));
        assert!(dsl.reference_groups["kotlin"].contains(&"dsl".to_string()));
        assert!(code_reference_load_plan(&dsl.reference_groups)
            .iter()
            .any(|item| {
                item.ref_id == "tech.code.kotlin.dsl" && item.path == "tech/code/kotlin/dsl.md"
            }));
    }

    #[test]
    fn kotlin_framework_references_require_selected_frameworks() {
        let ktor_baseline = baseline(json!({
            "tracks": {"backend": {"selection": "Kotlin + Ktor"}}
        }));
        let ktor_task = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::CreateOrUpdateInterface],
        );
        let ktor = code_reference_selection_for_task(&ktor_baseline, &ktor_task).unwrap();
        assert!(ktor.reference_groups["kotlin"].contains(&"ktor".to_string()));
        assert!(!ktor.reference_groups["kotlin"].contains(&"compose".to_string()));
        assert!(!ktor.reference_groups["kotlin"].contains(&"multiplatform".to_string()));
        assert!(!ktor.reference_groups["kotlin"].contains(&"testing".to_string()));

        let compose_baseline = baseline(json!({
            "tracks": {"app": {"selection": "Kotlin + Jetpack Compose"}}
        }));
        let mut compose_task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        compose_task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let compose = code_reference_selection_for_task(&compose_baseline, &compose_task).unwrap();
        assert!(compose.reference_groups["kotlin"].contains(&"compose".to_string()));
        assert!(!compose.reference_groups["kotlin"].contains(&"ktor".to_string()));

        let kmp_baseline = baseline(json!({
            "tracks": {"app": {"selection": "Kotlin Multiplatform + Ktor Client"}}
        }));
        let kmp_task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        let kmp = code_reference_selection_for_task(&kmp_baseline, &kmp_task).unwrap();
        assert!(kmp.reference_groups["kotlin"].contains(&"multiplatform".to_string()));
        assert!(!kmp.reference_groups["kotlin"].contains(&"ktor".to_string()));
        assert!(!kmp.reference_groups["kotlin"].contains(&"compose".to_string()));
    }

    #[test]
    fn kotlin_references_are_complete_and_decision_oriented() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let references = [
            (
                "plugins/shared/loom/references/tech/code/kotlin/core.md",
                35,
                &["## Boundary Decisions", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/kotlin/coroutines.md",
                35,
                &["## Decision Rules", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/kotlin/ktor.md",
                35,
                &["## Boundary Decisions", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/kotlin/compose.md",
                35,
                &["## Decision Rules", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/kotlin/multiplatform.md",
                35,
                &["## Decision Rules", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/kotlin/dsl.md",
                30,
                &["## Implementation Focus", "## Failure Modes"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/kotlin/testing.md",
                35,
                &["## Decision Rules", "## Evidence Focus"][..],
            ),
        ];

        for (relative, minimum_lines, required_sections) in references {
            let path = root.join(relative);
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            assert!(
                content.lines().count() >= minimum_lines,
                "{} is too thin",
                path.display()
            );
            for section in required_sections {
                assert!(
                    content.contains(section),
                    "{} missing {section}",
                    path.display()
                );
            }
        }
    }

    #[test]
    fn java_api_without_spring_does_not_load_spring_container_reference() {
        for stack in ["Java 21", "Java + Quarkus"] {
            let baseline = baseline(json!({
                "tracks": {"backend": {"selection": stack}}
            }));
            let task = task(
                TaskKind::InterfaceIncrement,
                vec![ImplementationAction::CreateOrUpdateInterface],
            );

            let selection = code_reference_selection_for_task(&baseline, &task).unwrap();

            assert!(!selection.reference_groups["java"].contains(&"spring".to_string()));
            assert!(!selection.reference_groups.contains_key("springboot"));
        }
    }

    #[test]
    fn php_references_are_framework_and_capability_scoped() {
        let laravel_baseline = baseline(json!({
            "tracks": {"backend": {"selection": "PHP 8.3 + Laravel"}}
        }));
        let ordinary_task = task(
            TaskKind::FeatureIncrement,
            vec![ImplementationAction::CreateOrUpdateBusinessRule],
        );
        let ordinary =
            code_reference_selection_for_task(&laravel_baseline, &ordinary_task).unwrap();
        assert!(ordinary.reference_groups["php"].contains(&"core".to_string()));
        assert!(ordinary.reference_groups["php"].contains(&"laravel".to_string()));
        assert!(!ordinary.reference_groups["php"].contains(&"modern".to_string()));
        assert!(!ordinary.reference_groups["php"].contains(&"testing".to_string()));
        assert!(!ordinary.reference_groups["php"].contains(&"async".to_string()));
        assert!(!ordinary.reference_groups["php"].contains(&"symfony".to_string()));

        let modern_task = task(
            TaskKind::RefactorSupport,
            vec![ImplementationAction::ImplementLanguageVersionFeature],
        );
        let modern = code_reference_selection_for_task(&laravel_baseline, &modern_task).unwrap();
        assert!(modern.reference_groups["php"].contains(&"modern".to_string()));
        assert!(code_reference_load_plan(&modern.reference_groups)
            .iter()
            .any(|item| item.path == "tech/code/php/modern.md"));

        let async_baseline = baseline(json!({
            "tracks": {"backend": {"selection": "PHP 8.3 + Swoole"}}
        }));
        let async_task = task(
            TaskKind::RefactorSupport,
            vec![ImplementationAction::ImplementAsyncProcessing],
        );
        let async_selection =
            code_reference_selection_for_task(&async_baseline, &async_task).unwrap();
        assert!(async_selection.reference_groups["php"].contains(&"async".to_string()));

        let symfony_baseline = baseline(json!({
            "tracks": {"backend": {"selection": "PHP 8.3 + Symfony"}}
        }));
        let symfony = code_reference_selection_for_task(&symfony_baseline, &ordinary_task).unwrap();
        assert!(symfony.reference_groups["php"].contains(&"core".to_string()));
        assert!(symfony.reference_groups["php"].contains(&"symfony".to_string()));
        assert!(!symfony.reference_groups["php"].contains(&"laravel".to_string()));
    }

    #[test]
    fn php_references_are_complete_and_decision_oriented() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let references = [
            (
                "plugins/shared/loom/references/tech/code/php/core.md",
                35,
                &["## Boundary Decisions", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/php/modern.md",
                30,
                &["## Implementation Focus", "## Failure Modes"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/php/async.md",
                35,
                &["## Runtime Selection", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/php/laravel.md",
                35,
                &["## Delivery Decisions", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/php/symfony.md",
                35,
                &["## Delivery Decisions", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/php/testing.md",
                35,
                &["## Decision Rules", "## Evidence Focus"][..],
            ),
        ];

        for (relative, minimum_lines, required_sections) in references {
            let path = root.join(relative);
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            assert!(
                content.lines().count() >= minimum_lines,
                "{} is too thin",
                path.display()
            );
            for section in required_sections {
                assert!(
                    content.contains(section),
                    "{} missing {section}",
                    path.display()
                );
            }
        }
    }

    #[test]
    fn rust_references_are_capability_scoped_even_with_tokio_baseline() {
        let baseline = baseline(json!({
            "tracks": {"backend": {"selection": "Rust 2021 + Tokio"}}
        }));
        let ordinary_task = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::CreateOrUpdateInterface],
        );
        let ordinary = code_reference_selection_for_task(&baseline, &ordinary_task).unwrap();
        assert_eq!(ordinary.reference_groups["rust"], vec!["core", "errors"]);
        assert!(!ordinary.reference_groups["rust"].contains(&"async".to_string()));
        assert!(!ordinary.reference_groups["rust"].contains(&"ownership".to_string()));
        assert!(!ordinary.reference_groups["rust"].contains(&"traits".to_string()));
        assert!(!ordinary.reference_groups["rust"].contains(&"testing".to_string()));

        let generic_task = task(
            TaskKind::RefactorSupport,
            vec![ImplementationAction::ImplementDependencyAbstraction],
        );
        let generic = code_reference_selection_for_task(&baseline, &generic_task).unwrap();
        assert!(generic.reference_groups["rust"].contains(&"ownership".to_string()));
        assert!(generic.reference_groups["rust"].contains(&"traits".to_string()));
        assert!(!generic.reference_groups["rust"].contains(&"async".to_string()));

        let async_task = task(
            TaskKind::RefactorSupport,
            vec![ImplementationAction::ImplementAsyncProcessing],
        );
        let async_selection = code_reference_selection_for_task(&baseline, &async_task).unwrap();
        assert!(async_selection.reference_groups["rust"].contains(&"async".to_string()));
        assert!(async_selection.reference_groups["rust"].contains(&"ownership".to_string()));
    }

    #[test]
    fn rust_references_are_complete_and_decision_oriented() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let references = [
            (
                "plugins/shared/loom/references/tech/code/rust/core.md",
                35,
                &["## Boundary Decisions", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/rust/ownership.md",
                35,
                &["## Decision Rules", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/rust/traits.md",
                35,
                &["## Decision Rules", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/rust/errors.md",
                35,
                &["## Boundary Decisions", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/rust/async.md",
                35,
                &["## Decision Rules", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/rust/testing.md",
                35,
                &["## Decision Rules", "## Evidence Focus"][..],
            ),
        ];

        for (relative, minimum_lines, required_sections) in references {
            let path = root.join(relative);
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            assert!(
                content.lines().count() >= minimum_lines,
                "{} is too thin",
                path.display()
            );
            for section in required_sections {
                assert!(
                    content.contains(section),
                    "{} missing {section}",
                    path.display()
                );
            }
        }
    }

    #[test]
    fn swift_references_are_capability_and_framework_scoped() {
        let ui_baseline = baseline(json!({
            "tracks": {"app": {"selection": "Swift + SwiftUI"}}
        }));
        let mut ui_task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        ui_task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let ui = code_reference_selection_for_task(&ui_baseline, &ui_task).unwrap();
        assert!(ui.reference_groups["swift"].contains(&"core".to_string()));
        assert!(ui.reference_groups["swift"].contains(&"swiftui".to_string()));
        assert!(!ui.reference_groups["swift"].contains(&"protocols".to_string()));
        assert!(!ui.reference_groups["swift"].contains(&"concurrency".to_string()));
        assert!(!ui.reference_groups["swift"].contains(&"memory".to_string()));
        assert!(!ui.reference_groups["swift"].contains(&"testing".to_string()));

        let backend_baseline = baseline(json!({
            "tracks": {"backend": {"selection": "Swift + Vapor"}}
        }));
        let protocol_task = task(
            TaskKind::RefactorSupport,
            vec![ImplementationAction::ImplementDependencyAbstraction],
        );
        let protocols =
            code_reference_selection_for_task(&backend_baseline, &protocol_task).unwrap();
        assert!(protocols.reference_groups["swift"].contains(&"protocols".to_string()));
        assert!(!protocols.reference_groups["swift"].contains(&"swiftui".to_string()));

        let async_task = task(
            TaskKind::RefactorSupport,
            vec![ImplementationAction::ImplementAsyncProcessing],
        );
        let async_selection =
            code_reference_selection_for_task(&backend_baseline, &async_task).unwrap();
        assert!(async_selection.reference_groups["swift"].contains(&"concurrency".to_string()));
        assert!(!async_selection.reference_groups["swift"].contains(&"protocols".to_string()));

        let mut performance_task = task(
            TaskKind::UiFlowIncrement,
            vec![
                ImplementationAction::CreateOrUpdateUiFlow,
                ImplementationAction::OptimizeRuntimePerformance,
            ],
        );
        performance_task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let performance =
            code_reference_selection_for_task(&ui_baseline, &performance_task).unwrap();
        assert!(performance.reference_groups["swift"].contains(&"memory".to_string()));
    }

    #[test]
    fn swift_references_are_complete_and_decision_oriented() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let references = [
            (
                "plugins/shared/loom/references/tech/code/swift/core.md",
                35,
                &["## Boundary Decisions", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/swift/protocols.md",
                35,
                &["## Decision Rules", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/swift/concurrency.md",
                35,
                &["## Decision Rules", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/swift/memory.md",
                35,
                &["## Decision Rules", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/swift/swiftui.md",
                35,
                &["## Decision Rules", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/swift/testing.md",
                35,
                &["## Decision Rules", "## Evidence Focus"][..],
            ),
        ];

        for (relative, minimum_lines, required_sections) in references {
            let path = root.join(relative);
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            assert!(
                content.lines().count() >= minimum_lines,
                "{} is too thin",
                path.display()
            );
            for section in required_sections {
                assert!(
                    content.contains(section),
                    "{} missing {section}",
                    path.display()
                );
            }
        }
    }

    #[test]
    fn spring_boot_observability_requires_structured_ownership() {
        let baseline = baseline(json!({
            "tracks": {"backend": {"selection": "Java + Spring Boot"}}
        }));
        let observability_task = task(
            TaskKind::ConfigurationSupport,
            vec![ImplementationAction::ImplementObservability],
        );
        let ordinary_config_task = task(
            TaskKind::ConfigurationSupport,
            vec![ImplementationAction::AddOrUpdateConfig],
        );

        let observability =
            code_reference_selection_for_task(&baseline, &observability_task).unwrap();
        let ordinary = code_reference_selection_for_task(&baseline, &ordinary_config_task).unwrap();

        assert!(observability.reference_groups["springboot"].contains(&"observability".to_string()));
        assert!(!ordinary.reference_groups["springboot"].contains(&"observability".to_string()));
        assert!(ordinary.reference_groups["springboot"].contains(&"runtime".to_string()));
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
        assert!(!selection.reference_groups["django"].contains(&"testing".to_string()));
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
        assert!(!selection.reference_groups["django"].contains(&"testing".to_string()));
        assert!(!selection.reference_groups["django"].contains(&"security".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "bk.django.models" && item.path == "tech/backend/django/models.md"
        }));
    }

    #[test]
    fn django_rest_framework_references_require_drf_stack_selection() {
        let django_baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Python + Django"}
            }
        }));
        let drf_baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Python + Django"},
                "apiFramework": {"selection": "Django REST Framework"}
            }
        }));
        let api_task = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::CreateOrUpdateInterface],
        );

        let django = code_reference_selection_for_task(&django_baseline, &api_task).unwrap();
        let drf = code_reference_selection_for_task(&drf_baseline, &api_task).unwrap();

        assert!(!django
            .reference_groups
            .get("django")
            .is_some_and(|items| items.contains(&"views".to_string())));
        assert!(!django
            .reference_groups
            .get("django")
            .is_some_and(|items| items.contains(&"serializers".to_string())));
        assert!(drf.reference_groups["django"].contains(&"views".to_string()));
        assert!(drf.reference_groups["django"].contains(&"serializers".to_string()));
    }

    #[test]
    fn django_security_and_testing_references_require_owned_actions() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Python + Django + Django REST Framework"}
            }
        }));
        let security_task = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::ImplementAuthenticationOrAuthorization],
        );
        let testing_task = task(
            TaskKind::VerificationIncrement,
            vec![ImplementationAction::AddOrUpdateTests],
        );

        let security = code_reference_selection_for_task(&baseline, &security_task).unwrap();
        let testing = code_reference_selection_for_task(&baseline, &testing_task).unwrap();

        assert!(security.reference_groups["django"].contains(&"security".to_string()));
        assert!(!security.reference_groups["django"].contains(&"testing".to_string()));
        assert!(testing.reference_groups["django"].contains(&"testing".to_string()));
        assert!(!testing.reference_groups["django"].contains(&"security".to_string()));
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
        assert!(!selection.reference_groups["python"].contains(&"async".to_string()));
        assert!(!selection.reference_groups["python"].contains(&"packaging".to_string()));
        assert!(selection.reference_groups["fastapi"].contains(&"routing".to_string()));
        assert!(selection.reference_groups["fastapi"].contains(&"schemas".to_string()));
        assert!(!selection.reference_groups["fastapi"].contains(&"testing".to_string()));
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
    fn python_specialized_references_require_owned_capabilities() {
        let baseline = baseline(json!({
            "tracks": {"backend": {"selection": "Python 3.11"}}
        }));
        let ordinary_task = task(
            TaskKind::FeatureIncrement,
            vec![ImplementationAction::CreateOrUpdateBusinessRule],
        );
        let ordinary = code_reference_selection_for_task(&baseline, &ordinary_task).unwrap();
        assert!(ordinary.reference_groups["python"].contains(&"core".to_string()));
        assert!(ordinary.reference_groups["python"].contains(&"typing".to_string()));
        assert!(!ordinary.reference_groups["python"].contains(&"async".to_string()));
        assert!(!ordinary.reference_groups["python"].contains(&"packaging".to_string()));
        assert!(!ordinary.reference_groups["python"].contains(&"testing".to_string()));

        let async_task = task(
            TaskKind::RefactorSupport,
            vec![ImplementationAction::ImplementAsyncProcessing],
        );
        let async_selection = code_reference_selection_for_task(&baseline, &async_task).unwrap();
        assert!(async_selection.reference_groups["python"].contains(&"async".to_string()));
        assert!(!async_selection.reference_groups["python"].contains(&"packaging".to_string()));

        let packaging_task = task(
            TaskKind::ConfigurationSupport,
            vec![ImplementationAction::AddOrUpdateConfig],
        );
        let packaging = code_reference_selection_for_task(&baseline, &packaging_task).unwrap();
        assert!(packaging.reference_groups["python"].contains(&"packaging".to_string()));
        assert!(!packaging.reference_groups["python"].contains(&"async".to_string()));
    }

    #[test]
    fn python_references_are_complete_and_decision_oriented() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let references = [
            (
                "plugins/shared/loom/references/tech/code/python/core.md",
                35,
                &["## Boundary Decisions", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/python/typing.md",
                35,
                &["## Decision Rules", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/python/async.md",
                35,
                &["## Decision Rules", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/python/packaging.md",
                35,
                &["## Decision Rules", "## Verification Focus"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/python/testing.md",
                35,
                &["## Decision Rules", "## Evidence Focus"][..],
            ),
        ];

        for (relative, minimum_lines, required_sections) in references {
            let path = root.join(relative);
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            assert!(
                content.lines().count() >= minimum_lines,
                "{} is too thin",
                path.display()
            );
            for section in required_sections {
                assert!(
                    content.contains(section),
                    "{} missing {section}",
                    path.display()
                );
            }
        }
    }

    #[test]
    fn fastapi_migration_reference_is_explicitly_scoped() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Python + FastAPI + SQLAlchemy"}
            }
        }));
        let migration_task = task(
            TaskKind::IntegrationIncrement,
            vec![ImplementationAction::MigrateFrameworkImplementation],
        );
        let selection = code_reference_selection_for_task(&baseline, &migration_task).unwrap();
        assert!(selection.reference_groups["fastapi"].contains(&"migration".to_string()));
        assert!(!selection.reference_groups["fastapi"].contains(&"data".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "bk.fastapi.migration"
                && item.path == "tech/backend/fastapi/migration.md"
        }));

        let mut prose_only = task(
            TaskKind::IntegrationIncrement,
            vec![ImplementationAction::RefactorSupportingCode],
        );
        prose_only.objective = "Migrate a Django endpoint to FastAPI.".to_string();
        let prose_selection = code_reference_selection_for_task(&baseline, &prose_only).unwrap();
        assert!(!prose_selection
            .reference_groups
            .get("fastapi")
            .is_some_and(|items| items.contains(&"migration".to_string())));
    }

    #[test]
    fn fastapi_data_reference_requires_sqlalchemy_stack_and_persistence_ownership() {
        let sqlalchemy_baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Python + FastAPI"},
                "dataAccess": {"selection": "SQLAlchemy 2"}
            }
        }));
        let tortoise_baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "Python + FastAPI"},
                "dataAccess": {"selection": "Tortoise ORM"}
            }
        }));
        let persistence_task = task(
            TaskKind::DataModelIncrement,
            vec![ImplementationAction::CreateOrUpdatePersistence],
        );

        let sqlalchemy =
            code_reference_selection_for_task(&sqlalchemy_baseline, &persistence_task).unwrap();
        let tortoise =
            code_reference_selection_for_task(&tortoise_baseline, &persistence_task).unwrap();

        assert!(sqlalchemy.reference_groups["fastapi"].contains(&"data".to_string()));
        assert!(!tortoise
            .reference_groups
            .get("fastapi")
            .is_some_and(|items| items.contains(&"data".to_string())));
    }

    #[test]
    fn aspnet_core_api_task_loads_minimal_without_data_or_architecture() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": ".NET 8 + ASP.NET Core Minimal APIs + Entity Framework Core"}
            }
        }));
        let task = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::CreateOrUpdateInterface],
        );
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(selection.reference_groups["csharp"].contains(&"core".to_string()));
        assert!(!selection.reference_groups["csharp"].contains(&"aspnet".to_string()));
        assert!(selection.reference_groups["aspnetcore"].contains(&"minimal".to_string()));
        assert!(!selection.reference_groups["aspnetcore"].contains(&"testing".to_string()));
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
        let prose_only = code_reference_selection_for_task(&baseline, &task).unwrap();
        assert!(!prose_only
            .reference_groups
            .get("aspnetcore")
            .is_some_and(|items| items.contains(&"architecture".to_string())));

        let context = CodeReferenceTaskContext {
            application_architecture: true,
            ..CodeReferenceTaskContext::default()
        };
        let selection =
            code_reference_selection_for_task_with_context(&baseline, &task, &context).unwrap();
        assert!(selection.reference_groups["aspnetcore"].contains(&"architecture".to_string()));
        assert!(!selection.reference_groups["aspnetcore"].contains(&"testing".to_string()));
        assert!(!selection.reference_groups["aspnetcore"].contains(&"minimal".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "bk.aspnet.architecture"
                && item.path == "tech/backend/aspnetcore/architecture.md"
        }));
    }

    #[test]
    fn aspnet_core_minimal_reference_requires_minimal_api_stack_selection() {
        let minimal_baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "C# 12 + ASP.NET Core Minimal APIs"}
            }
        }));
        let controller_baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "C# 12 + ASP.NET Core MVC Controllers"}
            }
        }));
        let task = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::CreateOrUpdateInterface],
        );

        let minimal = code_reference_selection_for_task(&minimal_baseline, &task).unwrap();
        let controllers = code_reference_selection_for_task(&controller_baseline, &task).unwrap();

        assert!(minimal.reference_groups["aspnetcore"].contains(&"minimal".to_string()));
        assert!(!controllers
            .reference_groups
            .get("aspnetcore")
            .is_some_and(|items| items.contains(&"minimal".to_string())));
    }

    #[test]
    fn aspnet_core_data_reference_requires_ef_core_and_persistence_ownership() {
        let ef_baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "C# 12 + ASP.NET Core"},
                "dataAccess": {"selection": "Entity Framework Core 8"}
            }
        }));
        let dapper_baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "C# 12 + ASP.NET Core"},
                "dataAccess": {"selection": "Dapper"}
            }
        }));
        let task = task(
            TaskKind::DataModelIncrement,
            vec![ImplementationAction::CreateOrUpdatePersistence],
        );

        let ef = code_reference_selection_for_task(&ef_baseline, &task).unwrap();
        let dapper = code_reference_selection_for_task(&dapper_baseline, &task).unwrap();

        assert!(ef.reference_groups["aspnetcore"].contains(&"data".to_string()));
        assert!(!dapper
            .reference_groups
            .get("aspnetcore")
            .is_some_and(|items| items.contains(&"data".to_string())));
    }

    #[test]
    fn aspnet_core_security_testing_and_runtime_references_are_task_owned() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "C# 12 + ASP.NET Core"}
            }
        }));
        let security_task = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::ImplementAuthenticationOrAuthorization],
        );
        let testing_task = task(
            TaskKind::VerificationIncrement,
            vec![ImplementationAction::AddOrUpdateTests],
        );
        let config_task = task(
            TaskKind::ConfigurationSupport,
            vec![ImplementationAction::AddOrUpdateConfig],
        );
        let capability_task = task(
            TaskKind::IntegrationIncrement,
            vec![
                ImplementationAction::ImplementAsyncProcessing,
                ImplementationAction::ImplementCachePolicy,
            ],
        );

        let security = code_reference_selection_for_task(&baseline, &security_task).unwrap();
        let testing = code_reference_selection_for_task(&baseline, &testing_task).unwrap();
        let runtime = code_reference_selection_for_task(&baseline, &config_task).unwrap();
        let capability = code_reference_selection_for_task(&baseline, &capability_task).unwrap();

        assert!(security.reference_groups["aspnetcore"].contains(&"security".to_string()));
        assert!(!security.reference_groups["aspnetcore"].contains(&"testing".to_string()));
        assert!(testing.reference_groups["aspnetcore"].contains(&"testing".to_string()));
        assert!(!testing.reference_groups["aspnetcore"].contains(&"security".to_string()));
        assert!(!testing
            .reference_groups
            .get("csharp")
            .is_some_and(|items| items.contains(&"testing".to_string())));
        assert!(runtime.reference_groups["aspnetcore"].contains(&"runtime".to_string()));
        assert!(!runtime.reference_groups["aspnetcore"].contains(&"testing".to_string()));
        assert!(capability.reference_groups["aspnetcore"].contains(&"runtime".to_string()));
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
        assert!(!selection.reference_groups["nestjs"].contains(&"testing".to_string()));
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
        let migration_task = task(
            TaskKind::IntegrationIncrement,
            vec![ImplementationAction::MigrateFrameworkImplementation],
        );
        let selection = code_reference_selection_for_task(&baseline, &migration_task).unwrap();
        assert!(selection.reference_groups["nestjs"].contains(&"migration".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "bk.nest.migration" && item.path == "tech/backend/nestjs/migration.md"
        }));

        let mut prose_only = task(
            TaskKind::IntegrationIncrement,
            vec![ImplementationAction::RefactorSupportingCode],
        );
        prose_only.objective =
            "Migrate existing Express routers and middleware into NestJS modules and controllers."
                .to_string();
        let prose_selection = code_reference_selection_for_task(&baseline, &prose_only).unwrap();
        assert!(!prose_selection.reference_groups["nestjs"].contains(&"migration".to_string()));
        assert!(prose_selection.reference_groups["nestjs"].contains(&"services".to_string()));
    }

    #[test]
    fn nestjs_testing_and_security_references_require_owned_actions() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "TypeScript + NestJS"}
            }
        }));
        let testing_task = task(
            TaskKind::VerificationIncrement,
            vec![ImplementationAction::AddOrUpdateTests],
        );
        let security_task = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::ImplementAuthenticationOrAuthorization],
        );

        let testing = code_reference_selection_for_task(&baseline, &testing_task).unwrap();
        let security = code_reference_selection_for_task(&baseline, &security_task).unwrap();

        assert!(testing.reference_groups["nestjs"].contains(&"testing".to_string()));
        assert!(testing.reference_groups["typescript"].contains(&"testing".to_string()));
        assert!(!testing.reference_groups["nestjs"].contains(&"security".to_string()));
        assert!(security.reference_groups["nestjs"].contains(&"security".to_string()));
        assert!(!security.reference_groups["nestjs"].contains(&"testing".to_string()));
    }

    #[test]
    fn nestjs_entity_only_task_does_not_assume_service_or_transport_work() {
        let baseline = baseline(json!({
            "tracks": {
                "backend": {"selection": "TypeScript + NestJS + TypeORM"}
            }
        }));
        let task = task(
            TaskKind::DataModelIncrement,
            vec![ImplementationAction::CreateOrUpdateEntity],
        );

        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();

        assert!(!selection
            .reference_groups
            .get("nestjs")
            .is_some_and(|items| {
                items.iter().any(|item| {
                    matches!(
                        item.as_str(),
                        "controllers" | "dtos" | "services" | "testing"
                    )
                })
            }));
    }

    #[test]
    fn javascript_async_reference_requires_async_task_ownership() {
        let baseline = baseline(json!({
            "tracks": {"backend": {"selection": "JavaScript + Node.js"}}
        }));
        let api_task = task(
            TaskKind::InterfaceIncrement,
            vec![ImplementationAction::CreateOrUpdateInterface],
        );
        let async_task = task(
            TaskKind::RefactorSupport,
            vec![ImplementationAction::ImplementAsyncProcessing],
        );

        let api = code_reference_selection_for_task(&baseline, &api_task).unwrap();
        let async_selection = code_reference_selection_for_task(&baseline, &async_task).unwrap();

        assert!(api.reference_groups["javascript"].contains(&"core".to_string()));
        assert!(api.reference_groups["javascript"].contains(&"modules".to_string()));
        assert!(api.reference_groups["javascript"].contains(&"node".to_string()));
        assert!(!api.reference_groups["javascript"].contains(&"async".to_string()));
        assert!(async_selection.reference_groups["javascript"].contains(&"async".to_string()));
    }

    #[test]
    fn javascript_frontend_task_loads_browser_without_node_reference() {
        let baseline = baseline(json!({
            "tracks": {"web": {"selection": "JavaScript"}}
        }));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        let javascript = &selection.reference_groups["javascript"];

        assert!(javascript.contains(&"browser".to_string()));
        assert!(!javascript.contains(&"node".to_string()));
    }

    #[test]
    fn javascript_references_are_complete_and_task_scoped() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let references = [
            (
                "plugins/shared/loom/references/tech/code/javascript/core.md",
                30,
                &[
                    "## When To Use",
                    "## Implementation Focus",
                    "## Verification Focus",
                ][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/javascript/async.md",
                45,
                &[
                    "### Combinator And Failure Selection",
                    "### Timeout And Cancellation Ownership",
                    "### Queue And Stream Boundaries",
                ][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/javascript/browser.md",
                40,
                &[
                    "### Worker And Observer Ownership",
                    "### Storage And Cache Evolution",
                    "### Permission And Main-Thread Boundaries",
                ][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/javascript/modules.md",
                35,
                &["### Resolution And Publication"][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/javascript/node.md",
                35,
                &[
                    "### Process And Child Boundaries",
                    "### HTTP And Stream Boundaries",
                ][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/javascript/testing.md",
                30,
                &[
                    "## When To Use",
                    "## Implementation Focus",
                    "## Verification Focus",
                ][..],
            ),
        ];

        for (relative, minimum_lines, required_sections) in references {
            let path = root.join(relative);
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            assert!(
                content.lines().count() >= minimum_lines,
                "{} is too thin",
                path.display()
            );
            for section in required_sections {
                assert!(
                    content.contains(section),
                    "{} missing {section}",
                    path.display()
                );
            }
        }
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
    fn framework_only_react_signal_maps_frontend_references() {
        let baseline = baseline(json!({"tracks": {"web": {"selection": "React"}}}));
        let seed = build_code_quality_seed(&baseline);
        assert_eq!(seed["required"], true);
        assert!(seed
            .pointer("/unmappedSignals")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty));
        assert!(seed.get("techReferenceProfile").is_none());
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.react.core" && item.path == "tech/frontend/react/core.md"
        }));
        assert!(!load_plan
            .iter()
            .any(|item| item.path.starts_with("tech/code/typescript/")));
    }

    #[test]
    fn unknown_stack_is_unmapped_without_blocking() {
        let baseline = baseline(json!({"tracks": {"backend": {"selection": "MysteryFlow"}}}));
        let seed = build_code_quality_seed(&baseline);
        assert_eq!(seed["required"], false);
        assert_eq!(seed["unmappedSignals"][0]["confidence"], "low");
    }

    #[test]
    fn mysql_schema_task_loads_only_schema_overlay() {
        let baseline = baseline(json!({
            "tracks": {"persistence": {"selection": "MySQL 8"}}
        }));
        let task = task(
            TaskKind::DataModelIncrement,
            vec![
                ImplementationAction::CreateOrUpdateEntity,
                ImplementationAction::CreateEntityMigration,
            ],
        );
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        let sql = &selection.reference_groups["sql"];
        assert!(sql.contains(&"schema".to_string()));
        assert!(sql.contains(&"dialects".to_string()));
        assert!(sql.contains(&"mysql.schema".to_string()));
        assert!(!sql.contains(&"queries".to_string()));
        assert!(!sql.contains(&"mysql.queries".to_string()));
        assert!(!sql.contains(&"mysql.transactions".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "tech.code.sql.mysql.schema"
                && item.path == "tech/code/sql/mysql/schema.md"
        }));
    }

    #[test]
    fn postgresql_query_task_loads_query_without_transaction_or_schema_overlays() {
        let baseline = baseline(json!({
            "tracks": {"persistence": {"selection": "PostgreSQL 16"}}
        }));
        let task = task(
            TaskKind::FeatureIncrement,
            vec![ImplementationAction::CreateOrUpdatePersistenceQuery],
        );
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        let sql = &selection.reference_groups["sql"];
        assert!(sql.contains(&"queries".to_string()));
        assert!(sql.contains(&"dialects".to_string()));
        assert!(sql.contains(&"postgresql.queries".to_string()));
        assert!(!sql.contains(&"postgresql.transactions".to_string()));
        assert!(!sql.contains(&"schema".to_string()));
        assert!(!sql.contains(&"postgresql.schema".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(!load_plan
            .iter()
            .any(|item| item.path == "tech/code/sql/postgresql/transactions.md"));
    }

    #[test]
    fn postgresql_transaction_task_loads_transaction_overlay() {
        let baseline = baseline(json!({
            "tracks": {"persistence": {"selection": "PostgreSQL 16"}}
        }));
        let task = task(
            TaskKind::IntegrationIncrement,
            vec![ImplementationAction::ImplementPersistenceTransaction],
        );
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        let sql = &selection.reference_groups["sql"];

        assert!(sql.contains(&"dialects".to_string()));
        assert!(sql.contains(&"postgresql.transactions".to_string()));
        assert!(!sql.contains(&"postgresql.schema".to_string()));
        assert!(!sql.contains(&"postgresql.queries".to_string()));
        assert!(code_reference_load_plan(&selection.reference_groups)
            .iter()
            .any(|item| {
                item.ref_id == "tech.code.sql.postgresql.transactions"
                    && item.path == "tech/code/sql/postgresql/transactions.md"
            }));
    }

    #[test]
    fn sqlserver_schema_task_loads_only_sqlserver_overlay() {
        let baseline = baseline(json!({
            "tracks": {"persistence": {"selection": "SQL Server 2022"}}
        }));
        let task = task(
            TaskKind::DataModelIncrement,
            vec![
                ImplementationAction::CreateOrUpdateEntity,
                ImplementationAction::CreateEntityMigration,
            ],
        );
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        let sql = &selection.reference_groups["sql"];
        assert!(sql.contains(&"schema".to_string()));
        assert!(sql.contains(&"dialects".to_string()));
        assert!(sql.contains(&"sqlserver.schema".to_string()));
        assert!(!sql.iter().any(|item| item.starts_with("mysql.")));
        assert!(!sql.iter().any(|item| item.starts_with("postgresql.")));
        assert!(!sql.iter().any(|item| item.starts_with("oracle.")));
        assert!(!sql.contains(&"queries".to_string()));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "tech.code.sql.sqlserver.schema"
                && item.path == "tech/code/sql/sqlserver/schema.md"
        }));
    }

    #[test]
    fn oracle_query_task_loads_query_without_transaction_or_schema_overlays() {
        let baseline = baseline(json!({
            "tracks": {"persistence": {"selection": "Oracle Database 19c"}}
        }));
        let task = task(
            TaskKind::FeatureIncrement,
            vec![ImplementationAction::CreateOrUpdatePersistenceQuery],
        );
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        let sql = &selection.reference_groups["sql"];
        assert!(sql.contains(&"queries".to_string()));
        assert!(sql.contains(&"oracle.queries".to_string()));
        assert!(!sql.contains(&"oracle.transactions".to_string()));
        assert!(!sql.contains(&"schema".to_string()));
        assert!(!sql.iter().any(|item| item.starts_with("mysql.")));
        assert!(!sql.iter().any(|item| item.starts_with("postgresql.")));
        assert!(!sql.iter().any(|item| item.starts_with("sqlserver.")));
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(!load_plan
            .iter()
            .any(|item| item.path == "tech/code/sql/oracle/transactions.md"));
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "tech.code.sql.oracle.queries"
                && item.path == "tech/code/sql/oracle/queries.md"
        }));
    }

    #[test]
    fn generic_tests_do_not_load_sql_references() {
        let baseline = baseline(json!({
            "tracks": {"persistence": {"selection": "MySQL"}}
        }));
        let task = task(
            TaskKind::VerificationIncrement,
            vec![ImplementationAction::AddOrUpdateTests],
        );
        assert!(code_reference_selection_for_task(&baseline, &task).is_none());
    }

    #[test]
    fn mariadb_does_not_silently_load_mysql_overlay() {
        let baseline = baseline(json!({
            "tracks": {"persistence": {"selection": "MariaDB"}}
        }));
        let task = task(
            TaskKind::DataModelIncrement,
            vec![ImplementationAction::CreateOrUpdateEntity],
        );
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        let sql = &selection.reference_groups["sql"];
        assert!(sql.contains(&"schema".to_string()));
        assert!(!sql.iter().any(|item| item.starts_with("mysql.")));
        assert!(!sql.iter().any(|item| item.starts_with("postgresql.")));
    }

    #[test]
    fn baseline_seed_does_not_expose_reference_routes() {
        let baseline = baseline(json!({
            "tracks": {"persistence": {"selection": "PostgreSQL"}}
        }));
        let seed = build_code_quality_seed(&baseline);
        assert_eq!(seed["required"], true);
        assert!(seed.get("techReferenceProfile").is_none());
    }

    #[test]
    fn sql_dialect_references_are_complete_and_non_operational() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let files = [
            "plugins/shared/loom/references/tech/code/sql/mysql/schema.md",
            "plugins/shared/loom/references/tech/code/sql/mysql/queries.md",
            "plugins/shared/loom/references/tech/code/sql/mysql/transactions.md",
            "plugins/shared/loom/references/tech/code/sql/postgresql/schema.md",
            "plugins/shared/loom/references/tech/code/sql/postgresql/queries.md",
            "plugins/shared/loom/references/tech/code/sql/postgresql/transactions.md",
            "plugins/shared/loom/references/tech/code/sql/sqlserver/schema.md",
            "plugins/shared/loom/references/tech/code/sql/sqlserver/queries.md",
            "plugins/shared/loom/references/tech/code/sql/sqlserver/transactions.md",
            "plugins/shared/loom/references/tech/code/sql/oracle/schema.md",
            "plugins/shared/loom/references/tech/code/sql/oracle/queries.md",
            "plugins/shared/loom/references/tech/code/sql/oracle/transactions.md",
        ];
        let excluded_operations = [
            "replication",
            "backup",
            "pitr",
            "wal",
            "vacuum",
            "pgbouncer",
            "pgpool",
            "pg_stat",
            "my.cnf",
            "max_connections",
            "innodb_buffer_pool",
            "create user",
        ];
        for relative in files {
            let path = root.join(relative);
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            assert!(
                content.lines().count() >= 55,
                "{} is too thin",
                path.display()
            );
            for section in [
                "## When To Use",
                "## Implementation Focus",
                "## Verification Focus",
                "## Evidence Focus",
            ] {
                assert!(
                    content.contains(section),
                    "{} missing {section}",
                    path.display()
                );
            }
            let lower = content.to_ascii_lowercase();
            for term in excluded_operations {
                assert!(
                    !lower.contains(term),
                    "{} contains excluded operation {term}",
                    path.display()
                );
            }
        }
    }

    #[test]
    fn sql_common_references_cover_design_query_plan_and_analytics_boundaries() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let references = [
            (
                "plugins/shared/loom/references/tech/code/sql/schema.md",
                50,
                &[
                    "## Temporal, Audit, And Soft-Delete Data",
                    "## Migration Compatibility",
                ][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/sql/queries.md",
                45,
                &[
                    "### Subqueries And Set Operations",
                    "### Mutation Result Boundary",
                ][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/sql/optimization.md",
                55,
                &[
                    "### Plan Reading",
                    "### Before And After Proof",
                    "## Risks To Avoid",
                ][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/sql/windows.md",
                45,
                &[
                    "### Function And Frame Selection",
                    "### Analytic Cost Boundary",
                ][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/sql/dialects.md",
                60,
                &[
                    "## Provider Decision Matrix",
                    "## Provider Overlays",
                    "SQL Server",
                    "Oracle",
                ][..],
            ),
        ];

        for (relative, minimum_lines, required_sections) in references {
            let path = root.join(relative);
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            assert!(
                content.lines().count() >= minimum_lines,
                "{} is too thin",
                path.display()
            );
            for section in required_sections {
                assert!(
                    content.contains(section),
                    "{} missing {section}",
                    path.display()
                );
            }
        }
    }

    #[test]
    fn java_references_are_complete_and_keep_framework_boundaries() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
        let references = [
            (
                "plugins/shared/loom/references/tech/code/java/core.md",
                &[
                    "## When To Use",
                    "## Implementation Focus",
                    "## Verification Focus",
                    "## Evidence Focus",
                ][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/java/spring.md",
                &[
                    "## When To Use",
                    "## Implementation Focus",
                    "## Verification Focus",
                    "## Evidence Focus",
                ][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/java/persistence.md",
                &[
                    "## Query Shape And Pagination",
                    "## Batch And Bulk Boundaries",
                    "## Cache And Measurement",
                ][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/java/reactive.md",
                &[
                    "## R2DBC Mapping Boundary",
                    "## Reactive Client Boundary",
                    "## Verification Focus",
                ][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/java/security.md",
                &[
                    "## When To Use",
                    "## Implementation Focus",
                    "## Verification Focus",
                    "## Evidence Focus",
                ][..],
            ),
            (
                "plugins/shared/loom/references/tech/code/java/testing.md",
                &[
                    "## When To Use",
                    "## Implementation Focus",
                    "## Verification Focus",
                    "## Evidence Focus",
                ][..],
            ),
        ];

        for (relative, required_sections) in references {
            let path = root.join(relative);
            let content = fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
            assert!(
                content.lines().count() >= 38,
                "{} is too thin",
                path.display()
            );
            for section in required_sections {
                assert!(
                    content.contains(section),
                    "{} missing {section}",
                    path.display()
                );
            }
        }

        let persistence = fs::read_to_string(
            root.join("plugins/shared/loom/references/tech/code/java/persistence.md"),
        )
        .expect("read Java persistence reference");
        assert!(persistence.contains("Spring Data repositories"));
        assert!(persistence.contains("selected provider"));

        let reactive = fs::read_to_string(
            root.join("plugins/shared/loom/references/tech/code/java/reactive.md"),
        )
        .expect("read Java reactive reference");
        assert!(reactive.contains("dedicated external-service integration reference"));
        assert!(reactive.contains("StepVerifier"));
    }
}
