use setup::{
    archive_package_layout, install, package_file_names, prepare_browser_runtime, purge,
    release_artifact_file_names, write_package_layout, AgentKind, BrowserRuntimePrepareOptions,
    ReleaseManifest, SetupEnvironment, SetupError, TargetPlatform, VERSION,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use toml_edit::DocumentMut;

const CODE_REFERENCE_FILES: &[&str] = &[
    "cpp/build",
    "cpp/concurrency",
    "cpp/core",
    "cpp/modern",
    "cpp/performance",
    "cpp/templates",
    "cpp/testing",
    "csharp/aspnet",
    "csharp/blazor",
    "csharp/core",
    "csharp/performance",
    "csharp/persistence",
    "csharp/testing",
    "go/concurrency",
    "go/core",
    "go/generics",
    "go/interfaces",
    "go/structure",
    "go/testing",
    "java/core",
    "java/persistence",
    "java/reactive",
    "java/security",
    "java/spring",
    "java/testing",
    "javascript/async",
    "javascript/browser",
    "javascript/core",
    "javascript/modules",
    "javascript/node",
    "javascript/testing",
    "kotlin/compose",
    "kotlin/core",
    "kotlin/coroutines",
    "kotlin/ktor",
    "kotlin/multiplatform",
    "kotlin/testing",
    "php/async",
    "php/core",
    "php/laravel",
    "php/symfony",
    "php/testing",
    "python/async",
    "python/core",
    "python/packaging",
    "python/testing",
    "python/typing",
    "rust/async",
    "rust/core",
    "rust/errors",
    "rust/ownership",
    "rust/testing",
    "rust/traits",
    "sql/dialects",
    "sql/optimization",
    "sql/queries",
    "sql/schema",
    "sql/windows",
    "sql/mysql/schema",
    "sql/mysql/queries",
    "sql/mysql/transactions",
    "sql/postgresql/schema",
    "sql/postgresql/queries",
    "sql/postgresql/transactions",
    "swift/concurrency",
    "swift/core",
    "swift/memory",
    "swift/protocols",
    "swift/swiftui",
    "swift/testing",
    "typescript/config",
    "typescript/core",
    "typescript/guards",
    "typescript/patterns",
    "typescript/testing",
    "typescript/types",
];

const BACKEND_REFERENCE_FILES: &[&str] = &[
    "aspnetcore/architecture",
    "aspnetcore/data",
    "aspnetcore/minimal",
    "aspnetcore/runtime",
    "aspnetcore/security",
    "aspnetcore/testing",
    "django/models",
    "django/security",
    "django/serializers",
    "django/testing",
    "django/views",
    "fastapi/data",
    "fastapi/migration",
    "fastapi/routing",
    "fastapi/schemas",
    "fastapi/security",
    "fastapi/testing",
    "nestjs/controllers",
    "nestjs/dtos",
    "nestjs/migration",
    "nestjs/security",
    "nestjs/services",
    "nestjs/testing",
    "springboot/async",
    "springboot/cache",
    "springboot/cloud",
    "springboot/data",
    "springboot/integration",
    "springboot/observability",
    "springboot/resilience",
    "springboot/runtime",
    "springboot/security",
    "springboot/testing",
    "springboot/web",
];

const FRONTEND_REFERENCE_FILES: &[&str] = &[
    "angular/components",
    "angular/core",
    "angular/ngrx",
    "angular/routing",
    "angular/rxjs",
    "angular/testing",
    "flutter/bloc",
    "flutter/core",
    "flutter/navigation",
    "flutter/performance",
    "flutter/riverpod",
    "flutter/structure",
    "flutter/testing",
    "flutter/widgets",
    "nextjs/actions",
    "nextjs/app-router",
    "nextjs/core",
    "nextjs/data",
    "nextjs/runtime",
    "nextjs/server-components",
    "nextjs/testing",
    "react/core",
    "react/hooks",
    "react/migration",
    "react/performance",
    "react/react19",
    "react/server-components",
    "react/state",
    "react/testing",
    "react-native/core",
    "react-native/lists",
    "react-native/navigation",
    "react-native/platform",
    "react-native/storage",
    "react-native/structure",
    "react-native/testing",
    "vue/build",
    "vue/components",
    "vue/core",
    "vue/mobile",
    "vue/nuxt",
    "vue/state",
    "vue/testing",
    "vue/typescript",
];

const REVIEW_REFERENCE_FILES: &[&str] = &[
    "core",
    "defect-patterns",
    "finding-quality",
    "spec-compliance",
    "test-evidence",
];

const PLAYWRIGHT_REFERENCE_FILES: &[&str] = &[
    "accessibility",
    "configuration",
    "core",
    "fixtures",
    "locators",
    "network",
    "reliability",
    "visual",
];

#[cfg(unix)]
#[test]
fn browser_runtime_prepare_reuses_valid_cache_and_rebuilds_checksum_drift() {
    use std::os::unix::fs::PermissionsExt;

    let fixture = Fixture::new("browser-runtime-cache");
    let fake_npm = fixture.root.join("fake-npm.sh");
    fs::write(
        &fake_npm,
        r#"#!/bin/sh
set -eu
spec=""
for arg in "$@"; do spec="$arg"; done
version="${spec##*@}"
mkdir -p node_modules/@playwright/test node_modules/.bin
printf '{"name":"@playwright/test","version":"%s"}\n' "$version" > node_modules/@playwright/test/package.json
cat > node_modules/.bin/playwright <<'EOF'
#!/bin/sh
set -eu
mkdir -p "$PLAYWRIGHT_BROWSERS_PATH/chromium-test"
printf ready > "$PLAYWRIGHT_BROWSERS_PATH/chromium-test/marker"
EOF
chmod +x node_modules/.bin/playwright
printf '{"lockfileVersion":3,"packages":{}}\n' > package-lock.json
"#,
    )
    .unwrap();
    let mut permissions = fs::metadata(&fake_npm).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_npm, permissions).unwrap();
    let fake_node = fixture.root.join("fake-node.sh");
    fs::write(&fake_node, "#!/bin/sh\nexit 0\n").unwrap();
    let mut permissions = fs::metadata(&fake_node).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_node, permissions).unwrap();
    let fake_container = fixture.root.join("fake-docker.sh");
    fs::write(&fake_container, "#!/bin/sh\nexit 0\n").unwrap();
    let mut permissions = fs::metadata(&fake_container).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&fake_container, permissions).unwrap();
    let env = fixture.env();
    let options = BrowserRuntimePrepareOptions {
        requested_versions: vec!["1.55.0".to_string()],
        npm_program: Some(fake_npm),
        node_program: Some(fake_node),
        container_program: Some(fake_container),
        ..BrowserRuntimePrepareOptions::default()
    };

    let first = prepare_browser_runtime(&env, &options).unwrap();
    assert_eq!(first.status, "ready");
    assert_eq!(first.platform, setup_platform_key());
    assert_eq!(first.runtimes.len(), 1);
    assert!(!first.runtimes[0].reused);
    assert_eq!(first.runtimes[0].platform, first.platform);
    assert_eq!(first.runtimes[0].browsers, vec!["chromium"]);
    assert!(Path::new(&first.runtimes[0].runner_path).is_file());
    assert!(first.runtimes[0]
        .doctor_checks
        .iter()
        .all(|check| check.status == "passed"));

    let second = prepare_browser_runtime(&env, &options).unwrap();
    assert!(second.runtimes[0].reused);

    let fake_node = options.node_program.as_ref().unwrap();
    fs::write(
        fake_node,
        "#!/bin/sh\necho 'Host system is missing dependencies' >&2\nexit 1\n",
    )
    .unwrap();
    let container_fallback = prepare_browser_runtime(&env, &options).unwrap();
    assert_eq!(container_fallback.status, "ready");
    assert!(container_fallback.runtimes[0].reused);
    assert_eq!(container_fallback.runtimes[0].backend, "managed_container");
    assert!(container_fallback.runtimes[0]
        .doctor_checks
        .iter()
        .any(|check| { check.failure_code.as_deref() == Some("missing_system_dependencies") }));
    assert!(container_fallback.runtimes[0].managed_container.is_some());
    fs::write(fake_node, "#!/bin/sh\nexit 0\n").unwrap();

    let runtime_root = Path::new(&second.runtimes[0].manifest_path)
        .parent()
        .unwrap();
    fs::write(runtime_root.join("package-lock.json"), "corrupt").unwrap();
    let repaired = prepare_browser_runtime(&env, &options).unwrap();
    assert!(!repaired.runtimes[0].reused);
    assert!(repaired.runtimes[0]
        .doctor_checks
        .iter()
        .all(|check| check.status == "passed"));
}

fn setup_platform_key() -> String {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        "windows" => "windows",
        value => value,
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "x64",
        value => value,
    };
    format!("{os}-{arch}")
}

#[test]
fn install_cleans_confirmed_legacy_and_writes_mcp_registration() {
    let fixture = Fixture::new("install_cleans_legacy");
    fixture.write_package();
    let legacy_plugin = fixture.user_home.join("plugins/loom");
    fs::create_dir_all(&legacy_plugin).unwrap();
    fs::write(
        legacy_plugin.join("SKILL.md"),
        "old adapter uses $HOME/.loom/bin/loom-cli",
    )
    .unwrap();
    fs::create_dir_all(fixture.loom_home.join("adapters/codex")).unwrap();
    fs::write(
        fixture.loom_home.join("adapters/codex/refresh.json"),
        "{\"adapter\":\"codex\"}",
    )
    .unwrap();
    fs::create_dir_all(fixture.loom_home.join("bin")).unwrap();
    fs::write(
        fixture.loom_home.join("bin/loom-cli"),
        "LOOM_AGENT_PROFILE=codex",
    )
    .unwrap();
    fs::create_dir_all(fixture.user_home.join(".codex")).unwrap();
    fs::write(
        fixture.user_home.join(".codex/config.toml"),
        "model = \"gpt-5\"\n\n[mcp_servers.existing]\ncommand = \"existing-server\"\n",
    )
    .unwrap();
    fs::create_dir_all(fixture.user_home.join(".codex/mcp")).unwrap();
    fs::write(
        fixture.user_home.join(".codex/mcp/loom.json"),
        serde_json::json!({
            "name": "loom",
            "transport": "stdio",
            "command": "/old/runtime/bin/loom-mcp-server"
        })
        .to_string(),
    )
    .unwrap();
    let stale_codex_cache = fixture
        .user_home
        .join(".codex/plugins/cache/local-plugins/loom/0.1.0/skills/loom/references/delivery");
    fs::create_dir_all(&stale_codex_cache).unwrap();
    fs::write(
        stale_codex_cache.join("planning.md"),
        "Loom MCP-only stale delivery planning reference",
    )
    .unwrap();

    let env = fixture.env();
    let report = install(&env, &[AgentKind::Codex]).unwrap();

    assert_eq!(report.status, "ok");
    assert!(env.runtime_current().exists() || env.runtime_current().symlink_metadata().is_ok());
    assert!(env
        .agent_plugin_root(AgentKind::Codex)
        .join(".loom-mcp-install.json")
        .exists());
    assert!(!fixture
        .user_home
        .join(".codex/plugins/cache/local-plugins/loom")
        .join("0.1.0/skills/loom/references/delivery")
        .exists());
    assert!(fixture
        .user_home
        .join(".codex/plugins/cache/local-plugins/loom/0.1.0/skills/loom/SKILL.md")
        .exists());
    assert!(!fixture.loom_home.join("bin/loom-cli").exists());
    assert!(!env.agent_mcp_registration_path(AgentKind::Codex).exists());

    let codex_config_text = fs::read_to_string(env.codex_config_path()).unwrap();
    let codex_config = codex_config_text.parse::<DocumentMut>().unwrap();
    assert_eq!(codex_config["model"].as_str(), Some("gpt-5"));
    assert_eq!(
        codex_config["mcp_servers"]["existing"]["command"].as_str(),
        Some("existing-server")
    );
    assert_eq!(
        codex_config["mcp_servers"]["loom"]["command"].as_str(),
        Some(path_string_for_test(&env.runtime_current().join("bin/loom-mcp-server")).as_str())
    );
    assert_eq!(
        codex_config["mcp_servers"]["loom"]["env"]["LOOM_HOST"].as_str(),
        Some("codex")
    );
    assert_eq!(
        report
            .checks
            .iter()
            .find(|check| check.name == "codex.mcpRegistration")
            .unwrap()
            .status,
        "passed"
    );
}

#[test]
fn install_projects_shared_references_to_agent_read_paths() {
    let fixture = Fixture::new("install_shared_references");
    fixture.write_package();
    let env = fixture.env();
    write_json(
        &env.claude_config_path(),
        &serde_json::json!({
            "existing": true,
            "mcpServers": {
                "existing-server": {
                    "type": "stdio",
                    "command": "existing"
                }
            }
        }),
    );
    write_file(
        &env.opencode_config_path(),
        "{\n  \"$schema\": \"https://opencode.ai/config.json\",\n  \"mcp\": {\n    \"existing-server\": { \"type\": \"local\", \"command\": [\"existing\"], },\n  },\n}\n",
    );

    let report = install(&env, &AgentKind::all()).unwrap();

    for agent in [AgentKind::Codex, AgentKind::ClaudeCode] {
        let root = env.agent_plugin_root(agent);
        assert!(root.join("skills/loom/references/uix/core.md").exists());
        assert!(root
            .join("skills/loom/references/uix/templates/tokens.css.tpl")
            .exists());
        assert!(root
            .join("skills/loom/references/uix/templates/tokens.tailwind.tpl")
            .exists());
        assert!(root
            .join("skills/loom/references/uix/stacks/svelte.md")
            .exists());
        assert!(root
            .join("skills/loom/references/uix/stacks/uniapp.md")
            .exists());
        assert!(root
            .join("skills/loom/references/tech/arch/core.md")
            .exists());
        assert!(root
            .join("skills/loom/references/tech/arch/nfr.md")
            .exists());
        assert!(root
            .join("skills/loom/references/tech/arch/adr.md")
            .exists());
        assert!(root
            .join("skills/loom/references/tech/api/core.md")
            .exists());
        assert!(root
            .join("skills/loom/references/tech/api/contract.md")
            .exists());
        for file in REVIEW_REFERENCE_FILES {
            assert!(root
                .join(format!("skills/loom/references/tech/review/{file}.md"))
                .exists());
        }
        for file in PLAYWRIGHT_REFERENCE_FILES {
            assert!(root
                .join(format!(
                    "skills/loom/references/tech/test/playwright/{file}.md"
                ))
                .exists());
        }
        assert!(root
            .join("skills/loom/references/tech/code/common.md")
            .exists());
        for file in CODE_REFERENCE_FILES {
            assert!(root
                .join(format!("skills/loom/references/tech/code/{file}.md"))
                .exists());
        }
        for file in BACKEND_REFERENCE_FILES {
            assert!(root
                .join(format!("skills/loom/references/tech/backend/{file}.md"))
                .exists());
        }
        for file in FRONTEND_REFERENCE_FILES {
            assert!(root
                .join(format!("skills/loom/references/tech/frontend/{file}.md"))
                .exists());
        }
        assert!(!root.join("skills/loom/references/delivery").exists());
        assert!(root
            .join("skills/loom-deploy/references/compose.md")
            .exists());
        assert!(root
            .join("skills/loom-deploy/references/matrix.md")
            .exists());
        assert!(root
            .join("skills/loom-deploy/references/source-model.md")
            .exists());
        assert!(root
            .join("skills/loom-deploy/references/topology.md")
            .exists());
    }

    assert!(env
        .opencode_home
        .join("references/loom/uix/core.md")
        .exists());
    assert!(env
        .opencode_home
        .join("references/loom/uix/templates/tokens.css.tpl")
        .exists());
    assert!(env
        .opencode_home
        .join("references/loom/uix/templates/tokens.tailwind.tpl")
        .exists());
    assert!(env
        .opencode_home
        .join("references/loom/tech/arch/core.md")
        .exists());
    assert!(env
        .opencode_home
        .join("references/loom/tech/arch/nfr.md")
        .exists());
    assert!(env
        .opencode_home
        .join("references/loom/tech/arch/adr.md")
        .exists());
    assert!(env
        .opencode_home
        .join("references/loom/tech/api/core.md")
        .exists());
    assert!(env
        .opencode_home
        .join("references/loom/tech/api/contract.md")
        .exists());
    for file in REVIEW_REFERENCE_FILES {
        assert!(env
            .opencode_home
            .join(format!("references/loom/tech/review/{file}.md"))
            .exists());
    }
    for file in PLAYWRIGHT_REFERENCE_FILES {
        assert!(env
            .opencode_home
            .join(format!("references/loom/tech/test/playwright/{file}.md"))
            .exists());
    }
    assert!(env
        .opencode_home
        .join("references/loom/tech/code/common.md")
        .exists());
    for file in CODE_REFERENCE_FILES {
        assert!(env
            .opencode_home
            .join(format!("references/loom/tech/code/{file}.md"))
            .exists());
    }
    for file in BACKEND_REFERENCE_FILES {
        assert!(env
            .opencode_home
            .join(format!("references/loom/tech/backend/{file}.md"))
            .exists());
    }
    for file in FRONTEND_REFERENCE_FILES {
        assert!(env
            .opencode_home
            .join(format!("references/loom/tech/frontend/{file}.md"))
            .exists());
    }
    assert!(!env.opencode_home.join("references/loom/delivery").exists());
    assert!(env
        .opencode_home
        .join("references/loom-deploy/compose.md")
        .exists());
    assert!(env
        .opencode_home
        .join("references/loom-deploy/matrix.md")
        .exists());
    assert!(env
        .opencode_home
        .join("references/loom-deploy/source-model.md")
        .exists());
    assert!(env
        .opencode_home
        .join("references/loom-deploy/topology.md")
        .exists());
    assert!(env
        .opencode_home
        .join("references/loom/.loom-mcp-install.json")
        .exists());
    assert!(env
        .opencode_home
        .join("references/loom-deploy/.loom-mcp-install.json")
        .exists());
    assert!(!env
        .agent_mcp_registration_path(AgentKind::ClaudeCode)
        .exists());
    assert!(!env
        .agent_mcp_registration_path(AgentKind::Opencode)
        .exists());

    let claude_config = read_json(&env.claude_config_path());
    assert_eq!(claude_config["existing"].as_bool(), Some(true));
    assert_eq!(
        claude_config["mcpServers"]["existing-server"]["command"].as_str(),
        Some("existing")
    );
    assert_eq!(
        claude_config["mcpServers"]["loom"]["command"].as_str(),
        Some(path_string_for_test(&env.runtime_current().join("bin/loom-mcp-server")).as_str())
    );
    assert_eq!(
        claude_config["mcpServers"]["loom"]["env"]["LOOM_HOST"].as_str(),
        Some("claude-code")
    );

    let opencode_config = read_json(&env.opencode_config_path());
    assert_eq!(
        opencode_config["mcp"]["existing-server"]["command"][0].as_str(),
        Some("existing")
    );
    assert_eq!(
        opencode_config["mcp"]["loom"]["command"][0].as_str(),
        Some(path_string_for_test(&env.runtime_current().join("bin/loom-mcp-server")).as_str())
    );
    assert_eq!(
        opencode_config["mcp"]["loom"]["environment"]["LOOM_HOST"].as_str(),
        Some("opencode")
    );
    for check_name in ["claude-code.mcpRegistration", "opencode.mcpRegistration"] {
        assert_eq!(
            report
                .checks
                .iter()
                .find(|check| check.name == check_name)
                .unwrap()
                .status,
            "passed"
        );
    }
}

#[test]
fn install_blocks_unowned_existing_plugin() {
    let fixture = Fixture::new("install_blocks_unowned");
    fixture.write_package();
    let unowned = fixture.user_home.join("plugins/loom");
    fs::create_dir_all(&unowned).unwrap();
    fs::write(unowned.join("README.md"), "user owned plugin").unwrap();

    let error = install(&fixture.env(), &[AgentKind::Codex]).unwrap_err();
    match error {
        SetupError::LegacyCleanupBlocked(blocked) => {
            assert_eq!(blocked.len(), 1);
            assert!(blocked[0].path.contains("plugins/loom"));
        }
        other => panic!("expected LegacyCleanupBlocked, got {other:?}"),
    }
}

#[test]
fn purge_removes_user_runtime_but_preserves_project_state() {
    let fixture = Fixture::new("purge_preserves_project");
    fixture.write_package();
    let env = fixture.env();
    install(&env, &AgentKind::all()).unwrap();
    let project_state = fixture.root.join("project/.loom/status.json");
    fs::create_dir_all(project_state.parent().unwrap()).unwrap();
    fs::write(&project_state, "{}").unwrap();
    let opencode_user_config = env.opencode_home.join("settings.json");
    fs::create_dir_all(opencode_user_config.parent().unwrap()).unwrap();
    fs::write(&opencode_user_config, "{\"theme\":\"user\"}").unwrap();

    let report = purge(&env).unwrap();

    assert_eq!(report.command, "purge");
    assert!(!env.runtime_root().exists());
    assert!(project_state.exists());
    assert!(opencode_user_config.exists());
}

#[test]
fn release_package_names_cover_supported_platforms() {
    let names = package_file_names(VERSION);
    assert_eq!(
        names,
        vec![
            format!("loom-{VERSION}-darwin-arm64.tar.gz"),
            format!("loom-{VERSION}-darwin-x64.tar.gz"),
            format!("loom-{VERSION}-linux-x64.tar.gz"),
            format!("loom-{VERSION}-linux-arm64.tar.gz"),
            format!("loom-{VERSION}-windows-x64.zip"),
        ]
    );
    let artifacts = release_artifact_file_names(VERSION);
    assert_eq!(
        artifacts,
        vec![
            format!("loom-{VERSION}-darwin-arm64.tar.gz"),
            format!("loom-{VERSION}-darwin-arm64.tar.gz.sha256"),
            format!("loom-{VERSION}-darwin-x64.tar.gz"),
            format!("loom-{VERSION}-darwin-x64.tar.gz.sha256"),
            format!("loom-{VERSION}-linux-x64.tar.gz"),
            format!("loom-{VERSION}-linux-x64.tar.gz.sha256"),
            format!("loom-{VERSION}-linux-arm64.tar.gz"),
            format!("loom-{VERSION}-linux-arm64.tar.gz.sha256"),
            format!("loom-{VERSION}-windows-x64.zip"),
            format!("loom-{VERSION}-windows-x64.zip.sha256"),
        ]
    );
}

#[test]
fn archive_package_layout_writes_windows_zip_artifact() {
    let fixture = Fixture::new("archive_package");
    fixture.write_package();
    let output_dir = fixture.root.join("release");
    let archive = archive_package_layout(
        &fixture.package_root,
        &output_dir,
        TargetPlatform::WindowsX64,
    )
    .unwrap();
    assert_eq!(
        archive.file_name().unwrap().to_string_lossy(),
        format!("loom-{VERSION}-windows-x64.zip")
    );
    assert!(archive.exists());
    let checksum = archive.with_file_name(format!(
        "{}.sha256",
        archive.file_name().unwrap().to_string_lossy()
    ));
    assert!(checksum.exists());
    let checksum_text = fs::read_to_string(checksum).unwrap();
    assert!(checksum_text.contains(&sha256(&archive)));
    assert!(checksum_text.contains(&format!("loom-{VERSION}-windows-x64.zip")));
}

#[test]
fn install_sh_release_plan_resolves_platform_assets_and_checksums() {
    let repo = repo_root();
    let script = repo.join("install.sh");
    let mac_output = Command::new("sh")
        .arg(&script)
        .args([
            "--agent",
            "claude-code",
            "--version",
            "9.8.7",
            "--print-plan",
        ])
        .env("LOOM_INSTALL_TEST_OS", "Darwin")
        .env("LOOM_INSTALL_TEST_ARCH", "arm64")
        .output()
        .unwrap();
    assert!(
        mac_output.status.success(),
        "{}",
        String::from_utf8_lossy(&mac_output.stderr)
    );
    let mac_plan: serde_json::Value = serde_json::from_slice(&mac_output.stdout).unwrap();
    assert_eq!(mac_plan["agent"], "claude-code");
    assert_eq!(mac_plan["platform"], "darwin-arm64");
    assert_eq!(mac_plan["package"], "loom-9.8.7-darwin-arm64.tar.gz");
    assert_eq!(
        mac_plan["packageUrl"],
        "https://github.com/valkor-ai/loom/releases/download/v9.8.7/loom-9.8.7-darwin-arm64.tar.gz"
    );
    assert_eq!(
        mac_plan["checksumUrl"],
        "https://github.com/valkor-ai/loom/releases/download/v9.8.7/loom-9.8.7-darwin-arm64.tar.gz.sha256"
    );
    assert_eq!(mac_plan["archiveChecksumRequired"], true);

    let linux_output = Command::new("sh")
        .arg(&script)
        .args([
            "--agent",
            "all",
            "--version",
            "9.8.7",
            "--base-url",
            "https://mirror.example/loom",
            "--print-plan",
        ])
        .env("LOOM_INSTALL_TEST_OS", "linux")
        .env("LOOM_INSTALL_TEST_ARCH", "x86_64")
        .output()
        .unwrap();
    assert!(
        linux_output.status.success(),
        "{}",
        String::from_utf8_lossy(&linux_output.stderr)
    );
    let linux_plan: serde_json::Value = serde_json::from_slice(&linux_output.stdout).unwrap();
    assert_eq!(linux_plan["agent"], "all");
    assert_eq!(linux_plan["platform"], "linux-x64");
    assert_eq!(
        linux_plan["packageUrl"],
        "https://mirror.example/loom/loom-9.8.7-linux-x64.tar.gz"
    );
    assert_eq!(
        linux_plan["checksumUrl"],
        "https://mirror.example/loom/loom-9.8.7-linux-x64.tar.gz.sha256"
    );
}

#[test]
fn install_ps1_release_contract_uses_windows_zip_checksum_and_doctor() {
    let script = fs::read_to_string(repo_root().join("install.ps1")).unwrap();
    assert!(script.contains("loom-$Version-$platform.zip"));
    assert!(script.contains("$packageUrl.sha256"));
    assert!(script.contains("Get-FileHash -Algorithm SHA256"));
    assert!(script.contains("windows-x64"));
    assert!(script.contains("install --agent $Agent --package-root"));
    assert!(script.contains("doctor --agent $Agent --package-root"));
}

#[test]
fn release_workflow_uploads_installers_packages_and_checksums() {
    let workflow = fs::read_to_string(repo_root().join(".github/workflows/release.yml")).unwrap();
    assert!(workflow.contains("tags:"));
    assert!(workflow.contains("- \"v*\""));
    assert!(workflow.contains("contents: write"));
    for platform in [
        "darwin-arm64",
        "darwin-x64",
        "linux-x64",
        "linux-arm64",
        "windows-x64",
    ] {
        assert!(workflow.contains(platform), "missing platform {platform}");
    }
    assert!(workflow.contains("loom-setup.exe"));
    assert!(workflow.contains("install.sh"));
    assert!(workflow.contains("install.ps1"));
    assert!(workflow.contains("install.sh.sha256"));
    assert!(workflow.contains("install.ps1.sha256"));
    assert!(workflow.contains(".tar.gz.sha256"));
    assert!(workflow.contains(".zip.sha256"));
    assert!(workflow.contains("softprops/action-gh-release@v3"));
    assert!(workflow.contains("make_latest: true"));
}

#[test]
fn archive_package_layout_rejects_legacy_typescript_runtime_entries() {
    let fixture = Fixture::new("archive_rejects_legacy_runtime");
    fixture.write_package();
    write_file(
        &fixture.package_root.join("src/ts/cli.ts"),
        "console.log('legacy cli');\n",
    );
    fixture.write_checksums();

    let error = archive_package_layout(
        &fixture.package_root,
        &fixture.root.join("release"),
        TargetPlatform::DarwinArm64,
    )
    .unwrap_err();
    match error {
        SetupError::InvalidArgument(message) => {
            assert!(message.contains("release package must not include"));
            assert!(message.contains("src/ts/cli.ts"));
        }
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
}

#[test]
fn archive_package_layout_rejects_legacy_cli_launcher_entries() {
    let fixture = Fixture::new("archive_rejects_legacy_launcher");
    fixture.write_package();
    write_file(&fixture.package_root.join("bin/loom-cli"), "#!/bin/sh\n");
    fixture.write_checksums();

    let error = archive_package_layout(
        &fixture.package_root,
        &fixture.root.join("release"),
        TargetPlatform::LinuxX64,
    )
    .unwrap_err();
    match error {
        SetupError::InvalidArgument(message) => {
            assert!(message.contains("release package must not include"));
            assert!(message.contains("bin/loom-cli"));
        }
        other => panic!("expected InvalidArgument, got {other:?}"),
    }
}

#[test]
fn package_layout_copies_current_runtime_and_plugin_sources() {
    let fixture = Fixture::new("package_layout_sources");
    let binary_dir = fixture.root.join("built-bin");
    write_file(&binary_dir.join("loom-mcp-server"), "#!/bin/sh\n");
    write_file(&binary_dir.join("loom-setup"), "#!/bin/sh\n");
    std::env::set_var("LOOM_SETUP_BINARY_DIR", &binary_dir);

    let package = write_package_layout(
        &fixture.root.join("package-out"),
        TargetPlatform::DarwinArm64,
    );
    std::env::remove_var("LOOM_SETUP_BINARY_DIR");
    let package = package.unwrap();
    assert!(package.join("bin/loom-mcp-server").is_file());
    assert!(package.join("bin/loom-setup").is_file());
    assert!(package.join("python/algorithms/worker.py").is_file());
    assert!(package.join("python/runtime/README").is_file());
    assert!(package.join("plugins/codex/skills/loom/SKILL.md").is_file());
    assert!(package
        .join("plugins/claude-code/commands/loom.md")
        .is_file());
    assert!(package
        .join("plugins/opencode/.opencode/plugins/loom.js")
        .is_file());
    assert!(package
        .join("plugins/shared/loom/references/uix/core.md")
        .is_file());
    assert!(package
        .join("plugins/shared/loom-deploy/references/compose.md")
        .is_file());
    assert!(package
        .join("plugins/shared/loom-deploy/references/matrix.md")
        .is_file());
    assert!(package
        .join("plugins/shared/loom-deploy/references/source-model.md")
        .is_file());
    assert!(package
        .join("plugins/shared/loom-deploy/references/topology.md")
        .is_file());
    assert!(package.join("checksums.txt").is_file());
}

#[test]
fn plugin_templates_do_not_expose_legacy_protocol_terms() {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let plugin_root = repo.join("plugins");
    let files = [
        "codex/skills/loom/SKILL.md",
        "codex/skills/loom-deploy/SKILL.md",
        "codex/.codex-plugin/plugin.json",
        "claude-code/commands/loom.md",
        "claude-code/commands/loom-deploy.md",
        "claude-code/skills/loom/SKILL.md",
        "claude-code/skills/loom-deploy/SKILL.md",
        "claude-code/hooks/loom-workflow-guard.js",
        "opencode/.opencode/commands/loom.md",
        "opencode/.opencode/commands/loom-deploy.md",
        "opencode/.opencode/plugins/loom.js",
    ];
    let forbidden = [
        "loom-cli",
        "LOOM_AGENT_PROFILE",
        "LOOM_COMPACT_OUTPUT",
        "commandInvocation",
        "submitCommand.argv",
        "CLI envelope",
    ];
    for file in files {
        let path = plugin_root.join(file);
        let content = fs::read_to_string(&path).unwrap();
        for term in forbidden {
            assert!(
                !content.contains(term),
                "{} must not contain legacy term {term}",
                path.display()
            );
        }
    }
}

#[test]
fn deploy_plugin_templates_obey_active_operation_policy_fields() {
    let repo = repo_root();
    let plugin_root = repo.join("plugins");
    for file in [
        "codex/skills/loom-deploy/SKILL.md",
        "claude-code/commands/loom-deploy.md",
        "claude-code/skills/loom-deploy/SKILL.md",
        "opencode/.opencode/commands/loom-deploy.md",
    ] {
        let content = fs::read_to_string(plugin_root.join(file)).unwrap();
        for required in [
            "observationPolicy",
            "forbiddenActions",
            "finalResponsePolicy",
        ] {
            assert!(
                content.contains(required),
                "{file} must require deploy active_operation policy field {required}"
            );
        }
    }
}

#[test]
fn opencode_commands_expose_mcp_result_discipline() {
    let repo = repo_root();
    let command_root = repo.join("plugins/opencode/.opencode/commands");
    let loom = fs::read_to_string(command_root.join("loom.md")).unwrap();
    let deploy = fs::read_to_string(command_root.join("loom-deploy.md")).unwrap();

    for required in [
        "loom.inspectRequest",
        "loom.readFieldGroup",
        "requestReadPlan.groups",
        "GenerateKnowledgeSemanticsNext",
        "loom.knowledgeInspectChunk",
        "ExecuteTaskNext",
        "RunLoomToolNext",
        "retryTool",
        "DeployRepairAssetsNext",
        "Do not copy field-level contracts",
    ] {
        assert!(
            loom.contains(required),
            "opencode loom.md missing {required}"
        );
    }
    assert!(
        !loom.contains("loom.readRequestFields"),
        "opencode loom.md must not expose readRequestFields"
    );

    for required in [
        "Reference profiles:",
        "Reference discipline:",
        "referenceLoadPlan",
        "Resolve `path` relative to `../references/loom/`",
        "Load exactly the listed paths",
        "Do not derive paths from group names",
        "scan reference directories",
        "external language/API/architecture/UI skills",
        "do not paste reference prose or template bodies",
        "Delivery planning, design, review, repair, and handoff rules are supplied by the current MCP request/result",
        "Do not load separate delivery reference files",
    ] {
        assert!(
            loom.contains(required),
            "opencode loom.md missing optional reference guidance {required}"
        );
    }
    for forbidden in [
        "MCP-selected references:",
        "uiQualityContract.referenceProfile.groups",
        "../references/loom/uix/core.md",
        "`groups.core`",
        "../references/loom/uix/anti-patterns.md",
        "../references/loom/uix/templates/tokens.css.tpl",
        "Focus references are contract-selected group/items",
        "skill_reference_by_group",
    ] {
        assert!(
            !loom.contains(forbidden),
            "opencode loom.md must not retain hard-coded reference map fragment {forbidden}"
        );
    }

    for required in [
        "active_operation",
        "DeployRepairAssetsNext",
        "deploy execution repair",
        "loom.inspectRequest",
        "loom.readFieldGroup",
        "deployReferenceProfile",
        "../references/loom-deploy/",
        "referenceLoadPlan",
        "requestReadPlan.groups",
        "Do not copy deployment stack rules",
    ] {
        assert!(
            deploy.contains(required),
            "opencode loom-deploy.md missing {required}"
        );
    }
}

#[test]
fn agent_templates_expose_reference_loading_protocol() {
    let repo = repo_root();
    let plugin_root = repo.join("plugins");
    let files = [
        "codex/skills/loom/SKILL.md",
        "claude-code/skills/loom/SKILL.md",
        "opencode/.opencode/commands/loom.md",
    ];

    for file in files {
        let content = fs::read_to_string(plugin_root.join(file)).unwrap();
        assert!(
            !content.contains("## Optional References"),
            "{file} must use Reference Loading instead of Optional References"
        );
        assert!(
            !content.contains("UIX references:"),
            "{file} must not keep the old broad UIX references section"
        );
        for required in [
            "## Reference Loading",
            "Protocol:",
            "Reference discipline:",
            "After reading the current request group",
            "referenceLoadPlan",
            "Reference profiles:",
            "Do not derive paths from group names",
            "scan reference directories",
            "If a referenced file is not selected by the MCP contract",
            "do not paste reference prose or template bodies",
        ] {
            assert!(
                content.contains(required),
                "{file} missing reference loading protocol fragment {required}"
            );
        }
        for forbidden in [
            "MCP-selected references:",
            "uiQualityContract.referenceProfile.groups",
            "uiQualityContract.designTokenAssetPlan.templateId",
            "../references/loom/uix/core.md",
            "`groups.core`",
            "`groups.scenarios`",
            "`groups.tokens`",
            "`groups.stacks`",
            "`groups.templates`",
            "skill_reference_by_group",
        ] {
            assert!(
                !content.contains(forbidden),
                "{file} must not retain hard-coded reference map fragment {forbidden}"
            );
        }
    }

    for file in [
        "codex/skills/loom-deploy/SKILL.md",
        "claude-code/skills/loom-deploy/SKILL.md",
        "opencode/.opencode/commands/loom-deploy.md",
    ] {
        let content = fs::read_to_string(plugin_root.join(file)).unwrap();
        assert!(
            !content.contains("## Optional References"),
            "{file} must use Reference Loading instead of Optional References"
        );
        for required in [
            "## Reference Loading",
            "Protocol:",
            "deployReferenceProfile.referenceLoadPlan",
            "Each `referenceLoadPlan` entry contains `refId`, `path`, and `reason`",
            "Do not infer extra files",
            "do not paste reference prose",
            "If the current deploy action has no `deployReferenceProfile`",
        ] {
            assert!(
                content.contains(required),
                "{file} missing deploy reference loading protocol fragment {required}"
            );
        }
        for forbidden in [
            "deployReferenceProfile.referenceIds",
            "MCP-selected deploy references:",
            "`deploy.providers` ->",
            "`deploy.matrix` ->",
            "`deploy.stacks.java`",
        ] {
            assert!(
                !content.contains(forbidden),
                "{file} must not retain deploy reference id map fragment {forbidden}"
            );
        }
        assert!(
            !content.contains("external-references"),
            "{file} must not expose maintainer research as a deploy reference"
        );
    }

    let claude_deploy_command =
        fs::read_to_string(plugin_root.join("claude-code/commands/loom-deploy.md")).unwrap();
    assert!(
        claude_deploy_command.contains("load the installed `loom-deploy` skill"),
        "claude-code/commands/loom-deploy.md must delegate deploy reference loading to the installed skill"
    );
    assert!(
        claude_deploy_command.contains("must not maintain a separate deploy reference path map"),
        "claude-code/commands/loom-deploy.md must document that the command is not a second reference map"
    );
    for forbidden in [
        "## Reference Loading",
        "MCP-selected deploy references:",
        "`deploy.providers` ->",
        "`deploy.matrix` ->",
        "`deploy.stacks.java`",
        "../references/loom-deploy/",
        "../skills/loom/skills/loom-deploy/references/",
    ] {
        assert!(
            !claude_deploy_command.contains(forbidden),
            "claude-code/commands/loom-deploy.md must not retain duplicated deploy reference map fragment {forbidden}"
        );
    }
}

#[test]
fn loom_code_references_are_operational_and_load_plan_driven() {
    let repo = repo_root();
    let code_root = repo.join("plugins/shared/loom/references/tech/code");
    let backend_root = repo.join("plugins/shared/loom/references/tech/backend");
    let frontend_root = repo.join("plugins/shared/loom/references/tech/frontend");
    let common = fs::read_to_string(code_root.join("common.md")).unwrap();
    for required in [
        "Position In Loom",
        "Repository Adaptation",
        "Delivery Rules",
        "Verification Rules",
        "Evidence Rules",
        "Common Anti-Patterns",
    ] {
        assert!(
            common.contains(required),
            "common code reference missing shared section {required}"
        );
    }
    let mut files = Vec::new();
    collect_markdown_files(&code_root, &mut files);
    assert!(
        files.len() >= 71,
        "expected language reference coverage for all supported code profiles"
    );

    for path in files {
        if path.file_name().is_some_and(|name| name == "common.md") {
            continue;
        }
        let content = fs::read_to_string(&path).unwrap();
        let line_count = content.lines().count();
        let code_profile = path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str());
        let minimum_lines = if code_profile == Some("cpp") { 65 } else { 25 };
        assert!(
            line_count >= minimum_lines,
            "{} is too thin to act as a topic code reference: {line_count} lines",
            path.display()
        );
        for required in [
            "When To Use",
            "Implementation Focus",
            "Verification Focus",
            "Evidence Focus",
        ] {
            assert!(
                content.contains(required),
                "{} missing code reference section or boundary {required}",
                path.display()
            );
        }
        if code_profile == Some("cpp") {
            assert!(
                content.contains("## Unsafe Defaults"),
                "{} missing enhanced C++ unsafe-default guidance",
                path.display()
            );
        }
        for forbidden in [
            "referenceLoadPlan",
            "readFieldGroup",
            "requestReadPlan",
            "techReferenceProfile",
            "skill_reference_by_group",
            "Load this file only when `techReferenceProfile.groups",
            "Load only the listed group/items",
            "Source Coverage",
            "Repository Adaptation",
            "Delivery Patterns",
            "## Evidence\n",
            "## Anti-Patterns",
        ] {
            assert!(
                !content.contains(forbidden),
                "{} retained old group-to-path reference loading fragment {forbidden}",
                path.display()
            );
        }
    }
    let java_core = fs::read_to_string(code_root.join("java/core.md")).unwrap();
    assert!(java_core.contains("app.<project_slug>"));
    assert!(java_core.contains("app.generated"));
    assert!(java_core.contains("com.example"));
    let java_spring = fs::read_to_string(code_root.join("java/spring.md")).unwrap();
    assert!(java_spring.contains("component scanning"));
    assert!(java_spring.contains("app.<project_slug>"));
    assert!(java_spring.contains("@Qualifier"));
    let cpp_core = fs::read_to_string(code_root.join("cpp/core.md")).unwrap();
    assert!(cpp_core.contains("std::string_view"));
    assert!(cpp_core.contains("One Definition Rule"));
    let cpp_modern = fs::read_to_string(code_root.join("cpp/modern.md")).unwrap();
    assert!(cpp_modern.contains("feature-test macros"));
    assert!(cpp_modern.contains("std::expected"));
    let cpp_templates = fs::read_to_string(code_root.join("cpp/templates.md")).unwrap();
    assert!(cpp_templates.contains("reference collapsing"));
    assert!(cpp_templates.contains("explicit instantiation"));
    let cpp_performance = fs::read_to_string(code_root.join("cpp/performance.md")).unwrap();
    assert!(cpp_performance.contains("anti-optimization"));
    assert!(cpp_performance.contains("ISA-specific"));
    let cpp_concurrency = fs::read_to_string(code_root.join("cpp/concurrency.md")).unwrap();
    assert!(cpp_concurrency.contains("std::jthread"));
    assert!(cpp_concurrency.contains("happens-before"));
    let cpp_build = fs::read_to_string(code_root.join("cpp/build.md")).unwrap();
    assert!(cpp_build.contains("generator expressions"));
    assert!(cpp_build.contains("target_compile_features"));
    let cpp_testing = fs::read_to_string(code_root.join("cpp/testing.md")).unwrap();
    assert!(cpp_testing.contains("ASan"));
    assert!(cpp_testing.contains("Fuzz targets"));
    let spring_runtime = fs::read_to_string(backend_root.join("springboot/runtime.md")).unwrap();
    assert!(spring_runtime.contains("@ConfigurationProperties"));
    assert!(spring_runtime.contains("graceful shutdown"));
    let spring_async = fs::read_to_string(backend_root.join("springboot/async.md")).unwrap();
    assert!(spring_async.contains("@Async"));
    assert!(spring_async.contains("same-class self-invocation"));
    let spring_cache = fs::read_to_string(backend_root.join("springboot/cache.md")).unwrap();
    assert!(spring_cache.contains("@Cacheable"));
    assert!(spring_cache.contains("source of truth"));

    let mut backend_files = Vec::new();
    collect_markdown_files(&backend_root, &mut backend_files);
    assert!(
        backend_files.len() >= BACKEND_REFERENCE_FILES.len(),
        "expected backend framework reference coverage for selected framework profiles"
    );
    for path in backend_files {
        let content = fs::read_to_string(&path).unwrap();
        let line_count = content.lines().count();
        let backend_profile = path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str());
        let is_spring_boot = backend_profile == Some("springboot");
        let is_nestjs = backend_profile == Some("nestjs");
        let is_aspnet_core = backend_profile == Some("aspnetcore");
        let minimum_lines = if matches!(
            backend_profile,
            Some("springboot" | "fastapi" | "django" | "nestjs" | "aspnetcore")
        ) {
            65
        } else {
            25
        };
        assert!(
            line_count >= minimum_lines,
            "{} is too thin to act as a backend framework reference: {line_count} lines",
            path.display()
        );
        if is_spring_boot {
            for required in ["Verification Focus", "Unsafe Defaults"] {
                assert!(
                    content.contains(required),
                    "{} missing Spring Boot engineering section {required}",
                    path.display()
                );
            }
        } else if is_nestjs || is_aspnet_core {
            for required in [
                "## Verification",
                "## Delivery Evidence",
                "## Unsafe Defaults",
            ] {
                assert!(
                    content.contains(required),
                    "{} missing enhanced backend engineering section {required}",
                    path.display()
                );
            }
        } else {
            for required in [
                "When To Use",
                "Implementation Focus",
                "Verification Focus",
                "Evidence Focus",
            ] {
                assert!(
                    content.contains(required),
                    "{} missing backend framework reference section or boundary {required}",
                    path.display()
                );
            }
        }
        for forbidden in [
            "referenceLoadPlan",
            "readFieldGroup",
            "requestReadPlan",
            "techReferenceProfile",
            "skill_reference_by_group",
            "Load this file only when `techReferenceProfile.groups",
            "Load only the listed group/items",
            "Source Coverage",
            "Repository Adaptation",
            "Delivery Patterns",
            "## Evidence\n",
            "## Anti-Patterns",
        ] {
            assert!(
                !content.contains(forbidden),
                "{} retained protocol or duplicated shared reference fragment {forbidden}",
                path.display()
            );
        }
    }
    let spring_web = fs::read_to_string(backend_root.join("springboot/web.md")).unwrap();
    assert!(spring_web.contains("real Spring Boot base package"));
    assert!(spring_web.contains("com.example"));
    let fastapi_routing = fs::read_to_string(backend_root.join("fastapi/routing.md")).unwrap();
    assert!(fastapi_routing.contains("APIRouter"));
    assert!(fastapi_routing.contains("BackgroundTasks"));
    let fastapi_schemas = fs::read_to_string(backend_root.join("fastapi/schemas.md")).unwrap();
    assert!(fastapi_schemas.contains("model_fields_set"));
    assert!(fastapi_schemas.contains("from_attributes"));
    let fastapi_data = fs::read_to_string(backend_root.join("fastapi/data.md")).unwrap();
    assert!(fastapi_data.contains("async_sessionmaker"));
    assert!(fastapi_data.contains("Alembic"));
    let fastapi_security = fs::read_to_string(backend_root.join("fastapi/security.md")).unwrap();
    assert!(fastapi_security.contains("OAuth2PasswordBearer"));
    assert!(fastapi_security.contains("issuer"));
    let fastapi_testing = fs::read_to_string(backend_root.join("fastapi/testing.md")).unwrap();
    assert!(fastapi_testing.contains("ASGITransport"));
    assert!(fastapi_testing.contains("dependency_overrides"));
    let fastapi_migration = fs::read_to_string(backend_root.join("fastapi/migration.md")).unwrap();
    assert!(fastapi_migration.contains("ViewSet"));
    assert!(fastapi_migration.contains("parity"));
    let django_models = fs::read_to_string(backend_root.join("django/models.md")).unwrap();
    assert!(django_models.contains("select_related"));
    assert!(django_models.contains("apps.get_model"));
    let django_serializers =
        fs::read_to_string(backend_root.join("django/serializers.md")).unwrap();
    assert!(django_serializers.contains("SerializerMethodField"));
    assert!(django_serializers.contains("partial=True"));
    let django_views = fs::read_to_string(backend_root.join("django/views.md")).unwrap();
    assert!(django_views.contains("get_queryset"));
    assert!(django_views.contains("object-level checks"));
    let django_security = fs::read_to_string(backend_root.join("django/security.md")).unwrap();
    assert!(django_security.contains("SimpleJWT"));
    assert!(django_security.contains("CSRF"));
    let django_testing = fs::read_to_string(backend_root.join("django/testing.md")).unwrap();
    assert!(django_testing.contains("TransactionTestCase"));
    assert!(django_testing.contains("assertNumQueries"));
    let nest_controllers = fs::read_to_string(backend_root.join("nestjs/controllers.md")).unwrap();
    assert!(nest_controllers.contains("ParseUUIDPipe"));
    assert!(nest_controllers.contains("@Res()"));
    let nest_dtos = fs::read_to_string(backend_root.join("nestjs/dtos.md")).unwrap();
    assert!(nest_dtos.contains("PartialType"));
    assert!(nest_dtos.contains("bigint"));
    let nest_services = fs::read_to_string(backend_root.join("nestjs/services.md")).unwrap();
    assert!(nest_services.contains("useExisting"));
    assert!(nest_services.contains("forwardRef"));
    let nest_security = fs::read_to_string(backend_root.join("nestjs/security.md")).unwrap();
    assert!(nest_security.contains("getAllAndOverride"));
    assert!(nest_security.contains("APP_GUARD"));
    let nest_testing = fs::read_to_string(backend_root.join("nestjs/testing.md")).unwrap();
    assert!(nest_testing.contains("TestingModule"));
    assert!(nest_testing.contains("app.getHttpServer()"));
    let nest_migration = fs::read_to_string(backend_root.join("nestjs/migration.md")).unwrap();
    assert!(nest_migration.contains("parity matrix"));
    assert!(nest_migration.contains("routing owner"));
    let aspnet_architecture =
        fs::read_to_string(backend_root.join("aspnetcore/architecture.md")).unwrap();
    assert!(aspnet_architecture.contains("MediatR"));
    assert!(aspnet_architecture.contains("IServiceProvider"));
    let aspnet_minimal = fs::read_to_string(backend_root.join("aspnetcore/minimal.md")).unwrap();
    assert!(aspnet_minimal.contains("TypedResults"));
    assert!(aspnet_minimal.contains("WebApplicationFactory"));
    let aspnet_data = fs::read_to_string(backend_root.join("aspnetcore/data.md")).unwrap();
    assert!(aspnet_data.contains("IEntityTypeConfiguration"));
    assert!(aspnet_data.contains("DbUpdateConcurrencyException"));
    let aspnet_security = fs::read_to_string(backend_root.join("aspnetcore/security.md")).unwrap();
    assert!(aspnet_security.contains("IAuthorizationRequirement"));
    assert!(aspnet_security.contains("UseAuthentication"));
    let aspnet_runtime = fs::read_to_string(backend_root.join("aspnetcore/runtime.md")).unwrap();
    assert!(aspnet_runtime.contains("ValidateOnStart"));
    assert!(aspnet_runtime.contains("IHttpClientFactory"));
    assert!(aspnet_runtime.contains("HybridCache"));
    let aspnet_testing = fs::read_to_string(backend_root.join("aspnetcore/testing.md")).unwrap();
    assert!(aspnet_testing.contains("WebApplicationFactory<Program>"));
    assert!(aspnet_testing.contains("EF InMemory"));

    let mut frontend_files = Vec::new();
    collect_markdown_files(&frontend_root, &mut frontend_files);
    assert!(
        frontend_files.len() >= FRONTEND_REFERENCE_FILES.len(),
        "expected frontend framework reference coverage for selected framework profiles"
    );
    for path in frontend_files {
        let content = fs::read_to_string(&path).unwrap();
        let line_count = content.lines().count();
        let frontend_profile = path
            .parent()
            .and_then(|parent| parent.file_name())
            .and_then(|name| name.to_str());
        let is_angular = frontend_profile == Some("angular");
        let is_flutter = frontend_profile == Some("flutter");
        let is_nextjs = frontend_profile == Some("nextjs");
        let is_react = frontend_profile == Some("react");
        let is_react_native = frontend_profile == Some("react-native");
        let is_vue = frontend_profile == Some("vue");
        let is_enhanced_frontend =
            is_angular || is_flutter || is_nextjs || is_react || is_react_native || is_vue;
        let minimum_lines = if is_enhanced_frontend { 65 } else { 25 };
        assert!(
            line_count >= minimum_lines,
            "{} is too thin to act as a frontend framework reference: {line_count} lines",
            path.display()
        );
        if is_enhanced_frontend {
            for required in [
                "## Verification",
                "## Delivery Evidence",
                "## Unsafe Defaults",
            ] {
                assert!(
                    content.contains(required),
                    "{} missing enhanced frontend engineering section {required}",
                    path.display()
                );
            }
        } else {
            for required in [
                "When To Use",
                "Implementation Focus",
                "Verification Focus",
                "Evidence Focus",
            ] {
                assert!(
                    content.contains(required),
                    "{} missing frontend framework reference section {required}",
                    path.display()
                );
            }
        }
        for forbidden in [
            "referenceLoadPlan",
            "readFieldGroup",
            "requestReadPlan",
            "techReferenceProfile",
            "skill_reference_by_group",
            "Load this file only when `techReferenceProfile.groups",
            "Load only the listed group/items",
            "Source Coverage",
            "Repository Adaptation",
            "Delivery Patterns",
            "## Evidence\n",
            "## Anti-Patterns",
        ] {
            assert!(
                !content.contains(forbidden),
                "{} retained protocol or duplicated shared reference fragment {forbidden}",
                path.display()
            );
        }
    }
    let angular_core = fs::read_to_string(frontend_root.join("angular/core.md")).unwrap();
    assert!(angular_core.contains("ChangeDetectionStrategy.OnPush"));
    assert!(angular_core.contains("getRawValue()"));
    let angular_components =
        fs::read_to_string(frontend_root.join("angular/components.md")).unwrap();
    assert!(angular_components.contains("input.required"));
    assert!(angular_components.contains("DestroyRef"));
    let angular_routing = fs::read_to_string(frontend_root.join("angular/routing.md")).unwrap();
    assert!(angular_routing.contains("RouterTestingHarness"));
    assert!(angular_routing.contains("withComponentInputBinding"));
    let angular_rxjs = fs::read_to_string(frontend_root.join("angular/rxjs.md")).unwrap();
    assert!(angular_rxjs.contains("takeUntilDestroyed"));
    assert!(angular_rxjs.contains("shareReplay"));
    let angular_ngrx = fs::read_to_string(frontend_root.join("angular/ngrx.md")).unwrap();
    assert!(angular_ngrx.contains("createEntityAdapter"));
    assert!(angular_ngrx.contains("concatLatestFrom"));
    let angular_testing = fs::read_to_string(frontend_root.join("angular/testing.md")).unwrap();
    assert!(angular_testing.contains("provideHttpClientTesting"));
    assert!(angular_testing.contains("TestScheduler.run"));
    let flutter_core = fs::read_to_string(frontend_root.join("flutter/core.md")).unwrap();
    assert!(flutter_core.contains("if (!mounted)"));
    assert!(flutter_core.contains("Platform.is"));
    let flutter_widgets = fs::read_to_string(frontend_root.join("flutter/widgets.md")).unwrap();
    assert!(flutter_widgets.contains("ValueKey"));
    assert!(flutter_widgets.contains("SliverList"));
    let flutter_structure = fs::read_to_string(frontend_root.join("flutter/structure.md")).unwrap();
    assert!(flutter_structure.contains("build_runner"));
    assert!(flutter_structure.contains("conditional imports"));
    let flutter_navigation =
        fs::read_to_string(frontend_root.join("flutter/navigation.md")).unwrap();
    assert!(flutter_navigation.contains("stateful shell"));
    assert!(flutter_navigation.contains("pathParameters"));
    let flutter_riverpod = fs::read_to_string(frontend_root.join("flutter/riverpod.md")).unwrap();
    assert!(flutter_riverpod.contains("AsyncNotifierProvider"));
    assert!(flutter_riverpod.contains("copyWithPrevious"));
    let flutter_bloc = fs::read_to_string(frontend_root.join("flutter/bloc.md")).unwrap();
    assert!(flutter_bloc.contains("BlocProvider.value"));
    assert!(flutter_bloc.contains("event transformers"));
    let flutter_performance =
        fs::read_to_string(frontend_root.join("flutter/performance.md")).unwrap();
    assert!(flutter_performance.contains("profile mode"));
    assert!(flutter_performance.contains("RepaintBoundary"));
    let flutter_testing = fs::read_to_string(frontend_root.join("flutter/testing.md")).unwrap();
    assert!(flutter_testing.contains("ProviderContainer"));
    assert!(flutter_testing.contains("pumpAndSettle"));
    let next_core = fs::read_to_string(frontend_root.join("nextjs/core.md")).unwrap();
    assert!(next_core.contains("NEXT_PUBLIC_*"));
    assert!(next_core.contains("server-only"));
    let next_router = fs::read_to_string(frontend_root.join("nextjs/app-router.md")).unwrap();
    assert!(next_router.contains("intercepting routes"));
    assert!(next_router.contains("notFound()"));
    let next_server_components =
        fs::read_to_string(frontend_root.join("nextjs/server-components.md")).unwrap();
    assert!(next_server_components.contains("Serializable Handoff"));
    assert!(next_server_components.contains("suppressHydrationWarning"));
    let next_actions = fs::read_to_string(frontend_root.join("nextjs/actions.md")).unwrap();
    assert!(next_actions.contains("useActionState"));
    assert!(next_actions.contains("revalidateTag"));
    let next_data = fs::read_to_string(frontend_root.join("nextjs/data.md")).unwrap();
    assert!(next_data.contains("React `cache()`"));
    assert!(next_data.contains("cross-user/tenant"));
    let next_runtime = fs::read_to_string(frontend_root.join("nextjs/runtime.md")).unwrap();
    assert!(next_runtime.contains("output: 'standalone'"));
    assert!(next_runtime.contains("Edge"));
    let next_testing = fs::read_to_string(frontend_root.join("nextjs/testing.md")).unwrap();
    assert!(next_testing.contains("production `next build`"));
    assert!(next_testing.contains("Server Action"));
    let react_core = fs::read_to_string(frontend_root.join("react/core.md")).unwrap();
    assert!(react_core.contains("stable domain keys"));
    assert!(react_core.contains("dangerouslySetInnerHTML"));
    let react_hooks = fs::read_to_string(frontend_root.join("react/hooks.md")).unwrap();
    assert!(react_hooks.contains("AbortController"));
    assert!(react_hooks.contains("useSyncExternalStore"));
    let react_state = fs::read_to_string(frontend_root.join("react/state.md")).unwrap();
    assert!(react_state.contains("TanStack Query"));
    assert!(react_state.contains("query keys"));
    let react_migration = fs::read_to_string(frontend_root.join("react/migration.md")).unwrap();
    assert!(react_migration.contains("componentDidCatch"));
    assert!(react_migration.contains("behavior assertions proving parity"));
    let react_performance = fs::read_to_string(frontend_root.join("react/performance.md")).unwrap();
    assert!(react_performance.contains("representative workload"));
    assert!(react_performance.contains("Virtualized rows"));
    let react_19 = fs::read_to_string(frontend_root.join("react/react19.md")).unwrap();
    assert!(react_19.contains("useActionState"));
    assert!(react_19.contains("stable operation identity"));
    let react_server =
        fs::read_to_string(frontend_root.join("react/server-components.md")).unwrap();
    assert!(react_server.contains("Serializable Handoff"));
    assert!(react_server.contains("server-only"));
    let react_testing = fs::read_to_string(frontend_root.join("react/testing.md")).unwrap();
    assert!(react_testing.contains("userEvent"));
    assert!(react_testing.contains("Strict Mode"));
    let rn_core = fs::read_to_string(frontend_root.join("react-native/core.md")).unwrap();
    assert!(rn_core.contains("development-client rebuild"));
    assert!(rn_core.contains("stable target identity"));
    let rn_structure = fs::read_to_string(frontend_root.join("react-native/structure.md")).unwrap();
    assert!(rn_structure.contains("generated native output"));
    assert!(rn_structure.contains("Metro"));
    let rn_navigation =
        fs::read_to_string(frontend_root.join("react-native/navigation.md")).unwrap();
    assert!(rn_navigation.contains("singular/array forms"));
    assert!(rn_navigation.contains("hardware back"));
    let rn_platform = fs::read_to_string(frontend_root.join("react-native/platform.md")).unwrap();
    assert!(rn_platform.contains("Platform.select"));
    assert!(rn_platform.contains("permanently denied"));
    let rn_lists = fs::read_to_string(frontend_root.join("react-native/lists.md")).unwrap();
    assert!(rn_lists.contains("getItemLayout"));
    assert!(rn_lists.contains("onEndReached"));
    let rn_storage = fs::read_to_string(frontend_root.join("react-native/storage.md")).unwrap();
    assert!(rn_storage.contains("schemaVersion"));
    assert!(rn_storage.contains("late hydration"));
    let rn_testing = fs::read_to_string(frontend_root.join("react-native/testing.md")).unwrap();
    assert!(rn_testing.contains("React Native Testing Library"));
    assert!(rn_testing.contains("both-platform coverage"));
    let vue_core = fs::read_to_string(frontend_root.join("vue/core.md")).unwrap();
    assert!(vue_core.contains("watchEffect"));
    assert!(vue_core.contains("effectScope"));
    let vue_components = fs::read_to_string(frontend_root.join("vue/components.md")).unwrap();
    assert!(vue_components.contains("defineModel"));
    assert!(vue_components.contains("InjectionKey"));
    let vue_state = fs::read_to_string(frontend_root.join("vue/state.md")).unwrap();
    assert!(vue_state.contains("storeToRefs"));
    assert!(vue_state.contains("process-global"));
    let vue_typescript = fs::read_to_string(frontend_root.join("vue/typescript.md")).unwrap();
    assert!(vue_typescript.contains("vue-tsc"));
    assert!(vue_typescript.contains("defineExpose"));
    let vue_nuxt = fs::read_to_string(frontend_root.join("vue/nuxt.md")).unwrap();
    assert!(vue_nuxt.contains("useAsyncData"));
    assert!(vue_nuxt.contains("runtimeConfig.public"));
    let vue_build = fs::read_to_string(frontend_root.join("vue/build.md")).unwrap();
    assert!(vue_build.contains("VITE_*"));
    assert!(vue_build.contains("Manual chunks"));
    let vue_mobile = fs::read_to_string(frontend_root.join("vue/mobile.md")).unwrap();
    assert!(vue_mobile.contains("Capacitor"));
    assert!(vue_mobile.contains("network-only"));
    let vue_testing = fs::read_to_string(frontend_root.join("vue/testing.md")).unwrap();
    assert!(vue_testing.contains("Vue Test Utils"));
    assert!(vue_testing.contains("testing Pinia"));
}

#[test]
fn loom_review_references_are_operational_without_protocol_duplication() {
    let repo = repo_root();
    let review_root = repo.join("plugins/shared/loom/references/tech/review");
    let expected = [
        ("core.md", "Review Posture"),
        ("spec-compliance.md", "Missing Requirement Checks"),
        ("defect-patterns.md", "Functional Correctness"),
        ("test-evidence.md", "Strong Evidence"),
        ("finding-quality.md", "Finding Content"),
    ];

    for (file, required_section) in expected {
        let path = review_root.join(file);
        let content = fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("read review reference {}: {error}", path.display()));
        let line_count = content.lines().count();
        assert!(
            line_count >= 65,
            "{} is too thin to guide review decisions: {line_count} lines",
            path.display()
        );
        for required in ["Use this reference", required_section] {
            assert!(
                content.contains(required),
                "{} missing review reference section {required}",
                path.display()
            );
        }
        for forbidden in [
            "referenceLoadPlan",
            "readFieldGroup",
            "requestReadPlan",
            "techReferenceProfile",
            "skill_reference_by_group",
            "Full Review Report Template",
            "Code Review: [PR Title]",
            "Receiving Feedback",
            "Load this file only",
        ] {
            assert!(
                !content.contains(forbidden),
                "{} retained protocol, markdown report template, or human-feedback fragment {forbidden}",
                path.display()
            );
        }
    }
    let defect_patterns = fs::read_to_string(review_root.join("defect-patterns.md")).unwrap();
    assert!(defect_patterns.contains("placeholder namespaces"));
    assert!(defect_patterns.contains("com.example"));
    assert!(defect_patterns.contains("Mass assignment"));
    assert!(defect_patterns.contains("idempotency scope"));
    let core = fs::read_to_string(review_root.join("core.md")).unwrap();
    assert!(core.contains("Risk-Based Depth"));
    assert!(core.contains("Change Interaction"));
    let spec = fs::read_to_string(review_root.join("spec-compliance.md")).unwrap();
    assert!(spec.contains("Contract Pair Checks"));
    assert!(spec.contains("cross-surface closure"));
    let evidence = fs::read_to_string(review_root.join("test-evidence.md")).unwrap();
    assert!(evidence.contains("Claim Mapping"));
    assert!(evidence.contains("stale evidence"));
    let findings = fs::read_to_string(review_root.join("finding-quality.md")).unwrap();
    assert!(findings.contains("Severity By Impact"));
    assert!(findings.contains("Root Cause"));
}

#[test]
fn loom_api_references_preserve_production_contract_depth_without_policy_duplication() {
    let root = repo_root().join("plugins/shared/loom/references/tech/api");
    let contract = fs::read_to_string(root.join("contract.md")).unwrap();
    for required in [
        "Operation Objects",
        "OpenAPI 3.1 Schema Semantics",
        "Validation And Generation",
        "operationId",
        "additionalProperties",
        "do not use the OpenAPI 3.0 `nullable` keyword",
    ] {
        assert!(
            contract.contains(required),
            "API contract reference missing production rule {required}"
        );
    }

    let resource = fs::read_to_string(root.join("resource.md")).unwrap();
    for required in [
        "Method Semantics",
        "Success Status And Headers",
        "`202`",
        "`204`",
        "JSON Merge Patch",
        "Content-Type",
    ] {
        assert!(
            resource.contains(required),
            "API resource reference missing HTTP rule {required}"
        );
    }

    let pagination = fs::read_to_string(root.join("pagination.md")).unwrap();
    for required in [
        "Cursor And Keyset Contract",
        "unique tie-breaker",
        "opaque client tokens",
        "cursor/filter or cursor/sort mismatch",
        "malformed or tampered cursors",
    ] {
        assert!(
            pagination.contains(required),
            "API pagination reference missing production rule {required}"
        );
    }

    let errors = fs::read_to_string(root.join("errors.md")).unwrap();
    let operations = fs::read_to_string(root.join("operations.md")).unwrap();
    assert!(errors.contains("Error Code Ownership"));
    assert!(errors.contains("This reference owns error categories"));
    for duplicated_policy in [
        "## Request Tracking And Retry Guidance",
        "X-Request-ID",
        "Retry-After",
    ] {
        assert!(
            !errors.contains(duplicated_policy),
            "errors.md must not duplicate operational policy {duplicated_policy}"
        );
    }
    assert!(operations.contains("This file owns operational policy"));
    assert!(operations.contains("Retry And Availability Responses"));
    assert!(operations.contains("Request Tracing"));
}

#[test]
fn loom_tech_references_do_not_duplicate_mcp_contract_terms() {
    let repo = repo_root();
    let tech_root = repo.join("plugins/shared/loom/references/tech");
    let mut files = Vec::new();
    collect_markdown_files(&tech_root, &mut files);
    let forbidden = [
        "TaskResult",
        "ReviewResult",
        "TaskPlan",
        "AAC",
        "PGC",
        "apiContractEvidence",
        "apiContractRequirements",
        "architectureQualityEvidence",
        "architectureQualityRequirementRefs",
        "architectureQuality.",
        "interfaces[]",
        "interfaceId",
        "type: \"http_api\"",
        "recommendedNextAction",
        "nextAction",
        "execution_repair",
        "taskplan_repair",
        "architecture_artifact_repair",
        "manual_review",
        "continue_to_next_phase",
        "needs_user_decision",
        "approved_with_notes",
        "changes_requested",
        "requestReadPlan",
        "readFieldGroup",
        "referenceLoadPlan",
        "techReferenceProfile",
        "outputContract",
        "resultTemplate",
        "enumRefs",
        "schemaShape",
        "writeTargets",
        "MCP request",
        ".loom",
        "loom.",
    ];

    for path in files {
        let content = fs::read_to_string(&path).unwrap();
        for term in forbidden {
            assert!(
                !content.contains(term),
                "{} must not duplicate MCP contract term {term}",
                path.display()
            );
        }
    }
}

#[test]
fn loom_uix_references_do_not_duplicate_mcp_contract_terms() {
    let repo = repo_root();
    let uix_root = repo.join("plugins/shared/loom/references/uix");
    let mut files = Vec::new();
    collect_markdown_files(&uix_root, &mut files);
    let forbidden = [
        "TaskResult",
        "ReviewResult",
        "TaskPlan",
        "frontendQualitySelfCheck",
        "frontendExperienceRequirement",
        "uiQualityContract",
        "uiTaskQualityGates",
        "gateResults",
        "referenceGroupsChecked",
        "referenceFilesChecked",
        "designTokenEvidence",
        "designTokenAssetPlan",
        "requestRef",
        "requestReadPlan",
        "readFieldGroup",
        "readRequestFields",
        "referenceLoadPlan",
        "outputContract",
        "resultTemplate",
        "resultRules",
        "enumRefs",
        "schemaShape",
        "writeTargets",
        "MCP",
        "Loom",
        "MCP request",
        ".loom",
        "loom.",
    ];

    for path in files {
        let relative = path
            .strip_prefix(&uix_root)
            .expect("relative uix path")
            .to_string_lossy()
            .to_string();
        let content = fs::read_to_string(&path).unwrap();
        for term in forbidden {
            assert!(
                !content.contains(term),
                "{} must not duplicate MCP contract term {term}",
                relative
            );
        }
    }
}

fn collect_markdown_files(dir: &Path, output: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_markdown_files(&path, output);
        } else if path.extension().is_some_and(|extension| extension == "md") {
            output.push(path);
        }
    }
}

#[test]
fn deploy_references_explain_profile_and_provider_fallback_without_external_runtime_loading() {
    let repo = repo_root();
    let deploy_refs = repo.join("plugins/shared/loom-deploy/references");
    let providers = fs::read_to_string(deploy_refs.join("providers.md")).unwrap();
    let compose = fs::read_to_string(deploy_refs.join("compose.md")).unwrap();
    let dockerfile = fs::read_to_string(deploy_refs.join("dockerfile.md")).unwrap();
    let environment = fs::read_to_string(deploy_refs.join("environment.md")).unwrap();
    let repair = fs::read_to_string(deploy_refs.join("repair.md")).unwrap();
    let workspaces = fs::read_to_string(deploy_refs.join("workspaces.md")).unwrap();
    let matrix = fs::read_to_string(deploy_refs.join("matrix.md")).unwrap();
    let source_model = fs::read_to_string(deploy_refs.join("source-model.md")).unwrap();
    let topology = fs::read_to_string(deploy_refs.join("topology.md")).unwrap();
    let external =
        fs::read_to_string(repo.join("docs/maintainer/deploy-external-research.md")).unwrap();

    for required in [
        "Existing assets are tried first when the user did not force a provider",
        "may fall back to the generated provider",
        "When the user explicitly selected `compose-existing` or `dockerfile-existing`, fallback is not allowed",
    ] {
        assert!(
            providers.contains(required),
            "providers.md missing fallback rule {required}"
        );
    }
    assert!(
        !providers.contains("Do not automatically switch provider after a failure.\n"),
        "providers.md must not contradict unforced generated fallback"
    );

    for required in [
        "DeploymentSpec.runtime.ports",
        "`hostPort` is the real available local port chosen by Loom",
        "build.context",
        "build.dockerfile",
        "Frontend plus backend projects",
        "Topology-Aware Compose Contract",
        "publicEntryServiceId",
        "Internal backend",
        "Multi-port",
    ] {
        assert!(
            compose.contains(required),
            "compose.md missing generation guardrail {required}"
        );
    }

    for required in [
        "Source Root, Build Context, Workdir, And COPY Closure",
        "Backend-served frontend",
        "Existing Dockerfile wrapper",
    ] {
        assert!(
            dockerfile.contains(required),
            "dockerfile.md missing context closure guardrail {required}"
        );
    }

    for required in [
        "Environment Fact Flow",
        "File database handling is not SQLite-specific",
        "Service Dependency URLs",
        "Framework Local Safety Defaults",
    ] {
        assert!(
            environment.contains(required),
            "environment.md missing environment guardrail {required}"
        );
    }

    for required in [
        "Repair Decision Tree",
        "Generation-First Repair Posture",
        "sourceModelRef",
        "topologyRef",
        "Ask the user only when",
        "Protected Asset Boundary",
    ] {
        assert!(
            repair.contains(required),
            "repair.md missing repair posture {required}"
        );
    }

    for required in [
        "App Path And Build Context Matrix",
        "The selected app path is not always the build context",
        "Source Root Repair Boundary",
    ] {
        assert!(
            workspaces.contains(required),
            "workspaces.md missing workspace matrix {required}"
        );
    }

    for (file, content, required) in [
        (
            "matrix.md",
            &matrix,
            "Do not collapse the matrix into a single \"one container serves everything\" assumption",
        ),
        (
            "source-model.md",
            &source_model,
            "`DeploymentSourceModel` is generated by Loom and is the authority",
        ),
        (
            "topology.md",
            &topology,
            "`DeploymentTopology` is generated from Loom deploy facts",
        ),
    ] {
        assert!(
            content.contains(required),
            "{file} missing deploy fact authority statement {required}"
        );
    }

    for name in [
        "node", "java", "python", "go", "dotnet", "php", "ruby", "static",
    ] {
        let content = fs::read_to_string(deploy_refs.join(format!("{name}.md"))).unwrap();
        for required in [
            "Scanner Signals To Deploy Facts",
            "Generated Asset Expectations",
            "Repair Boundary",
        ] {
            assert!(
                content.contains(required),
                "{name}.md missing stack deploy closure section {required}"
            );
        }
    }

    assert!(
        external.contains("Maintainer-only research note"),
        "deploy external research doc must be clearly maintainer-only"
    );
    assert!(
        !deploy_refs.join("external-references.md").exists(),
        "maintainer research must not live under runtime deploy references"
    );
}

#[test]
fn agent_templates_expose_knowledge_direct_route_and_semantic_pack_discipline() {
    let repo = repo_root();
    let plugin_root = repo.join("plugins");
    let files = [
        "codex/skills/loom/SKILL.md",
        "claude-code/commands/loom.md",
        "claude-code/skills/loom/SKILL.md",
        "opencode/.opencode/commands/loom.md",
    ];

    for file in files {
        let content = fs::read_to_string(plugin_root.join(file)).unwrap();
        assert!(
            content.contains("knowledge"),
            "{file} must expose knowledge routing"
        );
        assert!(
            content.contains("loom.knowledge*") || content.contains("loom.knowledgeInspectChunk"),
            "{file} must route knowledge through MCP tools"
        );
        if file.contains("SKILL.md") || file.contains("opencode") {
            for required in [
                "GenerateKnowledgeSemanticsNext",
                "loom.knowledgeInspectChunk",
                "loom.knowledgeSemanticSubmitFile",
            ] {
                assert!(content.contains(required), "{file} missing {required}");
            }
        }
    }
}

#[test]
fn agent_templates_expose_run_loom_tool_next_discipline() {
    let repo = repo_root();
    let plugin_root = repo.join("plugins");
    let files = [
        "codex/skills/loom/SKILL.md",
        "claude-code/commands/loom.md",
        "claude-code/skills/loom/SKILL.md",
        "opencode/.opencode/commands/loom.md",
    ];

    for file in files {
        let content = fs::read_to_string(plugin_root.join(file)).unwrap();
        for required in [
            "RunLoomToolNext",
            "inspect the requestRef",
            "read only the returned readGroups",
            "call the returned Loom MCP tool",
            "retry the returned retryTool",
        ] {
            assert!(content.contains(required), "{file} missing {required}");
        }
    }

    let opencode_plugin =
        fs::read_to_string(plugin_root.join("opencode/.opencode/plugins/loom.js")).unwrap();
    for required in [
        "run_loom_tool",
        "read only the returned readGroups",
        "retry the returned retryTool",
    ] {
        assert!(
            opencode_plugin.contains(required),
            "opencode plugin missing auto-continue prompt discipline {required}"
        );
    }
}

#[test]
fn product_docs_do_not_expose_legacy_install_or_protocol_paths() {
    let repo = repo_root();
    let files = [
        "README.md",
        "README.zh-CN.md",
        "scripts/README.md",
        "tests/README.md",
    ];
    let forbidden = [
        "npm run plugin:",
        "loom-cli",
        "LOOM_AGENT_PROFILE",
        "LOOM_COMPACT_OUTPUT",
        "commandInvocation",
        "submitCommand.argv",
        "retryCommand.argv",
        "CLI envelope",
        "dist/cli.js",
        "agent-neutral CLI",
        "next-task",
        "readCommand.argv",
        "agentAction.read",
        ".refs",
    ];
    for file in files {
        let path = repo.join(file);
        let content = fs::read_to_string(&path).unwrap();
        for term in forbidden {
            assert!(
                !content.contains(term),
                "{} must not contain legacy product term {term}",
                path.display()
            );
        }
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

struct Fixture {
    root: PathBuf,
    user_home: PathBuf,
    loom_home: PathBuf,
    package_root: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("loom-setup-{name}-{unique}"));
        let user_home = root.join("home");
        let loom_home = user_home.join(".loom");
        let package_root = root.join("package");
        fs::create_dir_all(&package_root).unwrap();
        Self {
            root,
            user_home,
            loom_home,
            package_root,
        }
    }

    fn env(&self) -> SetupEnvironment {
        SetupEnvironment::for_test(
            self.user_home.clone(),
            self.loom_home.clone(),
            self.package_root.clone(),
        )
    }

    fn write_package(&self) {
        write_file(
            &self.package_root.join("bin/loom-mcp-server"),
            "#!/bin/sh\n",
        );
        write_file(&self.package_root.join("bin/loom-setup"), "#!/bin/sh\n");
        fs::create_dir_all(self.package_root.join("python/runtime")).unwrap();
        write_file(
            &self.package_root.join("python/algorithms/worker.py"),
            "print('{\"ok\": true}')\n",
        );
        self.write_codex_template();
        self.write_claude_template();
        self.write_opencode_template();
        self.write_shared_references();
        let manifest = ReleaseManifest::for_platform(TargetPlatform::DarwinArm64);
        write_json(&self.package_root.join("manifest.json"), &manifest);
        self.write_checksums();
    }

    fn write_codex_template(&self) {
        write_json(
            &self
                .package_root
                .join("plugins/codex/.codex-plugin/plugin.json"),
            &serde_json::json!({"name":"loom","version":"0.1.0"}),
        );
        write_file(
            &self.package_root.join("plugins/codex/skills/loom/SKILL.md"),
            "Loom MCP-only Codex skill\n",
        );
    }

    fn write_claude_template(&self) {
        write_json(
            &self
                .package_root
                .join("plugins/claude-code/.claude-plugin/plugin.json"),
            &serde_json::json!({"name":"loom","version":"0.1.0"}),
        );
        write_file(
            &self
                .package_root
                .join("plugins/claude-code/commands/loom.md"),
            "Loom MCP-only Claude command\n",
        );
        write_file(
            &self
                .package_root
                .join("plugins/claude-code/commands/loom-deploy.md"),
            "Loom MCP-only Claude deploy command\n",
        );
        write_file(
            &self
                .package_root
                .join("plugins/claude-code/skills/loom/SKILL.md"),
            "Loom MCP-only Claude skill\n",
        );
    }

    fn write_opencode_template(&self) {
        write_file(
            &self
                .package_root
                .join("plugins/opencode/.opencode/commands/loom.md"),
            "Loom MCP-only OpenCode command\n",
        );
        write_file(
            &self
                .package_root
                .join("plugins/opencode/.opencode/commands/loom-deploy.md"),
            "Loom MCP-only OpenCode deploy command\n",
        );
        write_file(
            &self
                .package_root
                .join("plugins/opencode/.opencode/plugins/loom.js"),
            "export const LoomPlugin = async () => ({});\n",
        );
    }

    fn write_shared_references(&self) {
        for name in [
            "anti-patterns",
            "content",
            "core",
            "data",
            "frameworks",
            "interaction",
            "mobile",
            "system",
            "verification",
        ] {
            write_file(
                &self
                    .package_root
                    .join(format!("plugins/shared/loom/references/uix/{name}.md")),
                &format!("# {name} Reference\n"),
            );
        }
        for path in [
            "scenarios/admin-dashboard",
            "scenarios/consumer-app",
            "scenarios/corporate-site",
            "scenarios/data-console",
            "scenarios/developer-tool",
            "scenarios/docs-site",
            "scenarios/fintech-consumer-app",
            "scenarios/fintech-workstation",
            "scenarios/immersive-3d",
            "scenarios/marketing-site",
            "scenarios/mobile-native",
            "scenarios/mobile-responsive",
            "stacks/native-mobile",
            "stacks/plain-html",
            "stacks/react",
            "stacks/svelte",
            "stacks/threejs",
            "stacks/uniapp",
            "stacks/vue",
            "tokens/color-system",
            "tokens/layout-grid",
            "tokens/motion",
            "tokens/radius-elevation",
            "tokens/spacing",
            "tokens/typography",
        ] {
            write_file(
                &self
                    .package_root
                    .join(format!("plugins/shared/loom/references/uix/{path}.md")),
                &format!("# {path} Reference\n"),
            );
        }
        for path in ["templates/tokens.css.tpl", "templates/tokens.tailwind.tpl"] {
            write_file(
                &self
                    .package_root
                    .join(format!("plugins/shared/loom/references/uix/{path}")),
                &format!("/* {path} */\n"),
            );
        }
        for path in [
            "adr", "core", "data", "failure", "nfr", "patterns", "system",
        ] {
            write_file(
                &self.package_root.join(format!(
                    "plugins/shared/loom/references/tech/arch/{path}.md"
                )),
                &format!("# {path} Architecture Reference\n"),
            );
        }
        for path in [
            "contract",
            "core",
            "errors",
            "evolution",
            "operations",
            "pagination",
            "resource",
            "security",
        ] {
            write_file(
                &self
                    .package_root
                    .join(format!("plugins/shared/loom/references/tech/api/{path}.md")),
                &format!("# {path} API Reference\n"),
            );
        }
        for path in REVIEW_REFERENCE_FILES {
            write_file(
                &self.package_root.join(format!(
                    "plugins/shared/loom/references/tech/review/{path}.md"
                )),
                &format!("# {path} Review Reference\n"),
            );
        }
        for path in PLAYWRIGHT_REFERENCE_FILES {
            write_file(
                &self.package_root.join(format!(
                    "plugins/shared/loom/references/tech/test/playwright/{path}.md"
                )),
                &format!("# {path} Playwright Reference\n"),
            );
        }
        write_file(
            &self
                .package_root
                .join("plugins/shared/loom/references/tech/code/common.md"),
            "# Common Code Reference\n",
        );
        for path in CODE_REFERENCE_FILES {
            write_file(
                &self.package_root.join(format!(
                    "plugins/shared/loom/references/tech/code/{path}.md"
                )),
                &format!("# {path} Code Reference\n"),
            );
        }
        for path in BACKEND_REFERENCE_FILES {
            write_file(
                &self.package_root.join(format!(
                    "plugins/shared/loom/references/tech/backend/{path}.md"
                )),
                &format!("# {path} Backend Reference\n"),
            );
        }
        for path in FRONTEND_REFERENCE_FILES {
            write_file(
                &self.package_root.join(format!(
                    "plugins/shared/loom/references/tech/frontend/{path}.md"
                )),
                &format!("# {path} Frontend Reference\n"),
            );
        }
        for name in [
            "bootstrap",
            "compose",
            "dockerfile",
            "dotnet",
            "environment",
            "go",
            "java",
            "matrix",
            "node",
            "php",
            "providers",
            "python",
            "repair",
            "ruby",
            "source-model",
            "static",
            "topology",
            "workspaces",
        ] {
            write_file(
                &self
                    .package_root
                    .join(format!("plugins/shared/loom-deploy/references/{name}.md")),
                &format!("# {name} Reference\n"),
            );
        }
    }

    fn write_checksums(&self) {
        let mut files = collect_files(&self.package_root);
        files.retain(|path| {
            path.file_name().and_then(|name| name.to_str()) != Some("checksums.txt")
        });
        let mut lines = Vec::new();
        for file in files {
            let relative = file.strip_prefix(&self.package_root).unwrap();
            lines.push(format!("{}  {}", sha256(&file), relative.to_string_lossy()));
        }
        lines.sort();
        write_file(
            &self.package_root.join("checksums.txt"),
            &format!("{}\n", lines.join("\n")),
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn write_file(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

fn write_json<T: serde::Serialize>(path: &Path, value: &T) {
    write_file(
        path,
        &format!("{}\n", serde_json::to_string_pretty(value).unwrap()),
    );
}

fn read_json(path: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn path_string_for_test(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn collect_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.extend(collect_files(&path));
        } else if path.is_file() {
            out.push(path);
        }
    }
    out
}

fn sha256(path: &Path) -> String {
    let bytes = fs::read(path).unwrap();
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
