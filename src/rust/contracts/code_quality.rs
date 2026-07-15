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

pub fn build_code_quality_seed(baseline: &TechnicalBaselineContract) -> Value {
    let signals = code_stack_signals_from_baseline(&baseline.stack);
    if signals.is_empty() {
        return Value::Null;
    }
    let reference_groups = baseline_reference_groups(&signals);
    let reference_load_plan = code_reference_load_plan(&reference_groups);
    let package_naming_policy = package_naming_policy_for_reference_groups(&reference_groups);
    json!({
        "required": !reference_groups.is_empty(),
        "qualityLevel": "production_code_implementation",
        "codeStackSignals": signals,
        "unmappedSignals": signals
            .iter()
            .filter(|signal| signal_is_unmapped(signal))
            .collect::<Vec<_>>(),
        "techReferenceProfile": {
            "loadMode": "mcp_reference_load_plan",
            "groups": {
                "code": reference_groups
            },
            "referenceLoadPlan": reference_load_plan
        },
        "packageNamingPolicy": package_naming_policy,
        "generationRules": [
            "Use TechnicalBaseline.stack only as the source fact for stack signals; do not reselect or reconfirm the technology stack.",
            "Use codeStackSignals as derived signals, then select code references by current task scope.",
            "Read only files listed in techReferenceProfile.referenceLoadPlan; selected code groups are semantic evidence labels, not path maps.",
            "Do not attach SQL references to every backend task merely because a database exists; attach SQL only for schema, migration, query, reporting, dialect, or optimization work.",
            "MySQL and PostgreSQL overlays are selected only from accepted dialect signals plus explicit persistence task ownership; do not load database administration or unrelated provider material.",
            "For JVM production source, derive package names from existing repository package roots, build group metadata, or confirmed organization/project identity; never create com.example/org.example/com.company/demo/sample package roots.",
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
                "csharp": ["core", "aspnet", "persistence", "blazor", "performance", "testing"],
                "cpp": ["core", "modern", "templates", "performance", "concurrency", "build", "testing"],
                "kotlin": ["core", "coroutines", "ktor", "compose", "multiplatform", "testing"],
                "php": ["core", "laravel", "symfony", "async", "testing"],
                "rust": ["core", "ownership", "traits", "errors", "async", "testing"],
                "swift": ["core", "swiftui", "concurrency", "protocols", "memory", "testing"],
                "sql": [
                    "schema", "queries", "dialects", "optimization", "windows",
                    "mysql.schema", "mysql.queries", "mysql.transactions",
                    "postgresql.schema", "postgresql.queries", "postgresql.transactions"
                ]
            }
        },
        "focusTag": ["api", "api_client", "frontend", "persistence", "security", "async", "performance", "configuration", "runtime", "integration", "migration", "architecture", "testing", "sql", "sql_schema", "sql_query", "sql_transaction", "sql_test", "generics", "analytics", "memory", "hooks", "state", "server_components", "react19", "app_router", "server_actions", "data_fetching", "build_tooling", "mobile", "nuxt", "routing", "rxjs", "ngrx", "riverpod", "bloc", "list_performance", "storage"],
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
        if !signal_applies_to_task(&signal, &focus_tags) {
            if signal.language.is_none() {
                unmapped_signals.push(signal);
            }
            continue;
        }
        let items = if signal.language.is_some() {
            reference_items_for_signal(&signal, &focus_tags)
        } else {
            BTreeSet::new()
        };
        let backend_items = backend_reference_items_for_signal(&signal, &focus_tags);
        let frontend_items = frontend_reference_items_for_signal(&signal, &focus_tags);
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

fn baseline_reference_groups(signals: &[CodeStackSignal]) -> BTreeMap<String, Vec<String>> {
    let mut groups = BTreeMap::<String, BTreeSet<String>>::new();
    let focus_tags = vec![
        "api".to_string(),
        "frontend".to_string(),
        "persistence".to_string(),
        "configuration".to_string(),
    ];
    for signal in signals {
        if let Some(language) = &signal.language {
            groups
                .entry(language.clone())
                .or_default()
                .extend(reference_items_for_signal(signal, &focus_tags));
        }
        for (framework, items) in backend_reference_items_for_signal(signal, &focus_tags) {
            groups.entry(framework).or_default().extend(items);
        }
        for (framework, items) in frontend_reference_items_for_signal(signal, &focus_tags) {
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
    } else if selection_mentions_flutter_framework(&haystack) {
        push_frontend_frameworks_from_haystack(&haystack, &mut frameworks);
        push_unique(&mut roles, "frontend");
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
        None => roles.contains("frontend") && has_focus("frontend"),
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
            if has_focus("testing") {
                items.insert("testing".to_string());
            }
        }
        Some("typescript") => {
            items.extend(["core", "types", "config", "patterns"].map(str::to_string));
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
            if has_focus("async") || has_focus("api") || has_focus("api_client") {
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
            items.extend(["core", "typing", "packaging"].map(str::to_string));
            if has_focus("testing") {
                items.insert("testing".to_string());
            }
            if has_focus("async") || signal.frameworks.iter().any(|item| item == "fastapi") {
                items.insert("async".to_string());
            }
        }
        Some("go") => {
            items.extend(["core", "interfaces", "structure"].map(str::to_string));
            if has_focus("testing") {
                items.insert("testing".to_string());
            }
            if has_focus("async") || has_focus("api") {
                items.insert("concurrency".to_string());
            }
            if has_focus("generics") {
                items.insert("generics".to_string());
            }
        }
        Some("csharp") => {
            items.insert("core".to_string());
            if has_focus("testing") {
                items.insert("testing".to_string());
            }
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
            items.extend(["core", "modern", "build"].map(str::to_string));
            if has_focus("testing") {
                items.insert("testing".to_string());
            }
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
            items.insert("core".to_string());
            if has_focus("testing") {
                items.insert("testing".to_string());
            }
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
            items.insert("core".to_string());
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
            items.extend(["core", "ownership", "traits", "errors"].map(str::to_string));
            if has_focus("testing") {
                items.insert("testing".to_string());
            }
            if has_focus("async") || signal.frameworks.iter().any(|item| item == "tokio") {
                items.insert("async".to_string());
            }
        }
        Some("swift") => {
            items.extend(["core", "protocols"].map(str::to_string));
            if has_focus("testing") {
                items.insert("testing".to_string());
            }
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
            let task_scoped_sql = has_focus("sql_schema")
                || has_focus("sql_query")
                || has_focus("sql_transaction")
                || has_focus("sql_test");
            if has_focus("sql_schema") || (has_focus("persistence") && !task_scoped_sql) {
                items.insert("schema".to_string());
                items.insert("dialects".to_string());
            }
            if has_focus("sql_query") {
                items.insert("queries".to_string());
            }
            if has_focus("performance") {
                items.insert("optimization".to_string());
            }
            if has_focus("analytics") {
                items.insert("windows".to_string());
            }
            if has_focus("sql_test") {
                items.insert("schema".to_string());
                items.insert("queries".to_string());
                items.insert("dialects".to_string());
            }
            if has_focus("sql_query") || has_focus("analytics") {
                items.insert("queries".to_string());
            }
            if has_focus("sql_transaction") {
                items.insert("dialects".to_string());
            }
            if has_focus("sql_schema")
                || has_focus("sql_query")
                || has_focus("sql_transaction")
                || has_focus("sql_test")
            {
                if signal.dialects.iter().any(|dialect| dialect == "mysql") {
                    if has_focus("sql_schema") || has_focus("sql_test") {
                        items.insert("mysql.schema".to_string());
                    }
                    if has_focus("sql_query")
                        || has_focus("performance")
                        || has_focus("analytics")
                        || has_focus("sql_test")
                    {
                        items.insert("mysql.queries".to_string());
                    }
                    if has_focus("sql_transaction") || has_focus("sql_test") {
                        items.insert("mysql.transactions".to_string());
                    }
                }
                if signal
                    .dialects
                    .iter()
                    .any(|dialect| dialect == "postgresql")
                {
                    if has_focus("sql_schema") || has_focus("sql_test") {
                        items.insert("postgresql.schema".to_string());
                    }
                    if has_focus("sql_query")
                        || has_focus("performance")
                        || has_focus("analytics")
                        || has_focus("sql_test")
                    {
                        items.insert("postgresql.queries".to_string());
                    }
                    if has_focus("sql_transaction") || has_focus("sql_test") {
                        items.insert("postgresql.transactions".to_string());
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
    focus_tags: &[String],
) -> BTreeMap<String, BTreeSet<String>> {
    let has_focus = |tag: &str| focus_tags.iter().any(|item| item == tag);
    let mut groups = BTreeMap::<String, BTreeSet<String>>::new();
    if signal.frameworks.iter().any(|item| item == "spring_boot") {
        let items = groups.entry("springboot".to_string()).or_default();
        if has_focus("testing") {
            items.insert("testing".to_string());
        }
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
        if has_focus("testing") {
            items.insert("testing".to_string());
        }
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
        if has_focus("testing") {
            items.insert("testing".to_string());
        }
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
        if has_focus("testing") {
            items.insert("testing".to_string());
        }
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
        if has_focus("testing") {
            items.insert("testing".to_string());
        }
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

fn frontend_reference_items_for_signal(
    signal: &CodeStackSignal,
    focus_tags: &[String],
) -> BTreeMap<String, BTreeSet<String>> {
    let has_focus = |tag: &str| focus_tags.iter().any(|item| item == tag);
    let mut groups = BTreeMap::<String, BTreeSet<String>>::new();
    if !has_focus("frontend") {
        return groups;
    }
    if signal.frameworks.iter().any(|item| item == "nextjs") {
        let items = groups.entry("nextjs".to_string()).or_default();
        items.insert("core".to_string());
        items.insert("app-router".to_string());
        if has_focus("testing") {
            items.insert("testing".to_string());
        }
        if has_focus("data_fetching") || has_focus("performance") || has_focus("persistence") {
            items.insert("data".to_string());
        }
        if has_focus("server_actions") {
            items.insert("actions".to_string());
        }
        if has_focus("server_components") {
            items.insert("server-components".to_string());
        }
        if has_focus("runtime") || has_focus("configuration") || has_focus("performance") {
            items.insert("runtime".to_string());
        }
    }
    if signal.frameworks.iter().any(|item| item == "react") {
        let items = groups.entry("react".to_string()).or_default();
        items.insert("core".to_string());
        items.insert("hooks".to_string());
        items.insert("state".to_string());
        if has_focus("testing") {
            items.insert("testing".to_string());
        }
        if has_focus("performance") {
            items.insert("performance".to_string());
        }
        if has_focus("server_components") {
            items.insert("server-components".to_string());
        }
        if has_focus("react19") {
            items.insert("react19".to_string());
        }
        if has_focus("migration") {
            items.insert("migration".to_string());
        }
    }
    if signal.frameworks.iter().any(|item| item == "vue") {
        let items = groups.entry("vue".to_string()).or_default();
        items.insert("core".to_string());
        items.insert("components".to_string());
        items.insert("state".to_string());
        if has_focus("testing") {
            items.insert("testing".to_string());
        }
        if signal.language.as_deref() == Some("typescript") {
            items.insert("typescript".to_string());
        }
        if has_focus("nuxt") || signal.frameworks.iter().any(|item| item == "nuxt") {
            items.insert("nuxt".to_string());
        }
        if has_focus("build_tooling")
            || has_focus("runtime")
            || has_focus("configuration")
            || has_focus("performance")
        {
            items.insert("build".to_string());
        }
        if has_focus("mobile") {
            items.insert("mobile".to_string());
        }
    }
    if signal.frameworks.iter().any(|item| item == "angular") {
        let items = groups.entry("angular".to_string()).or_default();
        items.insert("core".to_string());
        items.insert("components".to_string());
        if has_focus("testing") {
            items.insert("testing".to_string());
        }
        if has_focus("routing") {
            items.insert("routing".to_string());
        }
        if has_focus("rxjs") || has_focus("async") || has_focus("data_fetching") {
            items.insert("rxjs".to_string());
        }
        if has_focus("ngrx") {
            items.insert("ngrx".to_string());
        }
    }
    if signal.frameworks.iter().any(|item| item == "reactnative") {
        let items = groups.entry("reactnative".to_string()).or_default();
        items.insert("core".to_string());
        items.insert("structure".to_string());
        items.insert("platform".to_string());
        if has_focus("testing") {
            items.insert("testing".to_string());
        }
        if has_focus("routing") || signal.frameworks.iter().any(|item| item == "expo") {
            items.insert("navigation".to_string());
        }
        if has_focus("list_performance") || has_focus("performance") {
            items.insert("lists".to_string());
        }
        if has_focus("storage") {
            items.insert("storage".to_string());
        }
    }
    if signal.frameworks.iter().any(|item| item == "flutter") {
        let items = groups.entry("flutter".to_string()).or_default();
        items.insert("core".to_string());
        items.insert("structure".to_string());
        items.insert("widgets".to_string());
        if has_focus("testing") {
            items.insert("testing".to_string());
        }
        if has_focus("routing") || signal.frameworks.iter().any(|item| item == "gorouter") {
            items.insert("navigation".to_string());
        }
        if has_focus("riverpod") || signal.frameworks.iter().any(|item| item == "riverpod") {
            items.insert("riverpod".to_string());
        }
        if has_focus("bloc") || signal.frameworks.iter().any(|item| item == "bloc") {
            items.insert("bloc".to_string());
        }
        if has_focus("performance") || has_focus("list_performance") {
            items.insert("performance".to_string());
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
            if matches!(provider, "mysql" | "postgresql")
                && matches!(subject, "schema" | "queries" | "transactions")
            {
                let label = match provider {
                    "mysql" => "MySQL",
                    _ => "PostgreSQL",
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
                    | ImplementationAction::ImplementFrontendExperienceContract
                    | ImplementationAction::CreateEntityAdminPage
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
                | ImplementationAction::CreateOrUpdatePersistenceQuery
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
    if !react_native {
        push_if_contains(
            haystack,
            frameworks,
            "react",
            &["react", "next", "nextjs", "next js"],
        );
    }
    push_if_contains(haystack, frameworks, "vue", &["vue", "nuxt"]);
    push_if_contains(haystack, frameworks, "nuxt", &["nuxt"]);
    push_if_contains(haystack, frameworks, "angular", &["angular", "ngrx"]);
    push_if_contains(haystack, frameworks, "svelte", &["svelte"]);
}

fn selection_mentions_flutter_framework(haystack: &str) -> bool {
    contains_any(haystack, &["flutter", "riverpod", "go router", "gorouter"])
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
        let seed = build_code_quality_seed(&baseline);
        assert_eq!(
            seed["packageNamingPolicy"]["fallbackPackageTemplate"],
            json!("app.<project_slug>")
        );
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
        assert!(seed
            .pointer("/techReferenceProfile/referenceLoadPlan")
            .and_then(Value::as_array)
            .unwrap()
            .iter()
            .any(|item| item.get("path").and_then(Value::as_str)
                == Some("tech/frontend/react/core.md")));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![
                ImplementationAction::CreateOrUpdateUiFlow,
                ImplementationAction::WireReferenceInApiOrUi,
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
        assert!(!seed["techReferenceProfile"]["referenceLoadPlan"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["path"]
                .as_str()
                .is_some_and(|path| path.ends_with("/testing.md"))));

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
            item.ref_id == "fe.react.state" && item.path == "tech/frontend/react/state.md"
        }));
    }

    #[test]
    fn react_specialized_refs_are_task_scoped() {
        let baseline = baseline(json!({
            "tracks": {
                "web": {"selection": "React + TypeScript"}
            }
        }));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        task.objective = "Migrate a class component to hooks and add React 19 useActionState optimistic form state with server components while optimizing table performance.".to_string();
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
    fn maps_nextjs_typescript_to_nextjs_react_and_typescript_refs() {
        let baseline = baseline(json!({
            "tracks": {
                "web": {"selection": "Next.js + TypeScript"}
            }
        }));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        task.objective =
            "Create an App Router dashboard page with loading.tsx and error.tsx boundaries."
                .to_string();
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
                "web": {"selection": "Next.js + TypeScript"}
            }
        }));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        task.objective = "Add Server Actions with revalidatePath, no-store data fetching, Suspense server components, runtime config, and next build validation.".to_string();
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
        let next_refs = &selection.reference_groups["nextjs"];
        assert!(next_refs.contains(&"actions".to_string()));
        assert!(next_refs.contains(&"data".to_string()));
        assert!(next_refs.contains(&"server-components".to_string()));
        assert!(next_refs.contains(&"runtime".to_string()));
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
    fn maps_vue_typescript_to_vue_and_typescript_refs() {
        let baseline = baseline(json!({
            "tracks": {
                "web": {"selection": "Vue 3 + TypeScript"}
            }
        }));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        task.objective = "Build a Vue 3 Composition API purchase request list with typed props, Pinia store state, and component tests.".to_string();
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
                "web": {"selection": "Nuxt 3 + TypeScript"}
            }
        }));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        task.objective = "Implement a Nuxt page with useFetch, runtimeConfig, ClientOnly hydration guard, Vite build optimization, and mobile PWA offline state.".to_string();
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
    fn maps_angular_typescript_to_task_scoped_angular_refs() {
        let baseline = baseline(json!({
            "tracks": {
                "web": {"selection": "Angular 17 + TypeScript"}
            }
        }));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        task.objective = "Build standalone purchase approval components with signals, route guard and resolver, RxJS switchMap search, NgRx entity adapter state, and component tests.".to_string();
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
    fn maps_react_native_typescript_without_web_react_refs() {
        let baseline = baseline(json!({
            "tracks": {
                "app": {"selection": "React Native + Expo + TypeScript"}
            }
        }));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        task.objective = "Create a mobile purchase request screen with Expo Router navigation, iOS and Android SafeArea keyboard handling, FlatList pull to refresh, and MMKV storage.".to_string();
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
            vec![ImplementationAction::CreateOrUpdateUiFlow],
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
    fn maps_flutter_stack_to_task_scoped_flutter_refs() {
        let baseline = baseline(json!({
            "tracks": {
                "app": {"selection": "Flutter 3 + Riverpod + GoRouter"}
            }
        }));
        let mut task = task(
            TaskKind::UiFlowIncrement,
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        task.objective = "Create a purchase approval Flutter screen with GoRouter route params, ConsumerWidget Riverpod AsyncValue state, ListView.builder rows, and jank profiling.".to_string();
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
            vec![ImplementationAction::CreateOrUpdateUiFlow],
        );
        task.frontend_experience_requirement = Some(json!({"uiTaskScope": {}}));
        task.objective = "Implement a Flutter wizard using Cubit, BlocBuilder, BlocListener, immutable states, and widget tests.".to_string();
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
        let load_plan = code_reference_load_plan(&selection.reference_groups);
        assert!(load_plan.iter().any(|item| {
            item.ref_id == "fe.flutter.widgets" && item.path == "tech/frontend/flutter/widgets.md"
        }));
    }

    #[test]
    fn dart_alone_stays_unmapped_without_blocking() {
        let baseline = baseline(json!({"tracks": {"app": {"selection": "Dart"}}}));
        let seed = build_code_quality_seed(&baseline);
        assert_eq!(seed["required"], false);
        assert_eq!(seed["unmappedSignals"][0]["confidence"], "low");
        assert!(seed
            .pointer("/techReferenceProfile/referenceLoadPlan")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty));
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
        assert!(!selection.reference_groups["springboot"].contains(&"testing".to_string()));
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
        assert!(!selection.reference_groups["springboot"].contains(&"testing".to_string()));
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
        assert!(!selection.reference_groups["springboot"].contains(&"testing".to_string()));
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
        let selection = code_reference_selection_for_task(&baseline, &task).unwrap();
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
    fn framework_only_react_signal_maps_frontend_references() {
        let baseline = baseline(json!({"tracks": {"web": {"selection": "React"}}}));
        let seed = build_code_quality_seed(&baseline);
        assert_eq!(seed["required"], true);
        assert!(seed
            .pointer("/unmappedSignals")
            .and_then(Value::as_array)
            .is_some_and(Vec::is_empty));
        let load_plan = seed
            .pointer("/techReferenceProfile/referenceLoadPlan")
            .and_then(Value::as_array)
            .unwrap();
        assert!(load_plan.iter().any(|item| {
            item.get("refId").and_then(Value::as_str) == Some("fe.react.core")
                && item.get("path").and_then(Value::as_str) == Some("tech/frontend/react/core.md")
        }));
        assert!(!load_plan.iter().any(|item| item
            .get("path")
            .and_then(Value::as_str)
            .is_some_and(|path| path.starts_with("tech/code/typescript/"))));
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
    fn postgresql_query_task_loads_query_and_transaction_overlays_without_schema() {
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
        assert!(sql.contains(&"postgresql.transactions".to_string()));
        assert!(!sql.contains(&"schema".to_string()));
        assert!(!sql.contains(&"postgresql.schema".to_string()));
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
    fn baseline_seed_does_not_load_vendor_overlays() {
        let baseline = baseline(json!({
            "tracks": {"persistence": {"selection": "PostgreSQL"}}
        }));
        let seed = build_code_quality_seed(&baseline);
        let load_plan = seed
            .pointer("/techReferenceProfile/referenceLoadPlan")
            .and_then(Value::as_array)
            .unwrap();
        assert!(!load_plan.iter().any(|item| {
            item.get("path")
                .and_then(Value::as_str)
                .is_some_and(|path| path.contains("/mysql/") || path.contains("/postgresql/"))
        }));
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
}
