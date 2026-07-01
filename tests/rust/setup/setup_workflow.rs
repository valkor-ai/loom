use setup::{
    archive_package_layout, install, package_file_names, purge, release_artifact_file_names,
    write_package_layout, AgentKind, ReleaseManifest, SetupEnvironment, SetupError, TargetPlatform,
    VERSION,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use toml_edit::DocumentMut;

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
        assert!(!root.join("skills/loom/references/delivery").exists());
        assert!(root
            .join("skills/loom-deploy/references/compose.md")
            .exists());
    }

    assert!(env
        .opencode_home
        .join("references/loom/uix/core.md")
        .exists());
    assert!(!env.opencode_home.join("references/loom/delivery").exists());
    assert!(env
        .opencode_home
        .join("references/loom-deploy/compose.md")
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
        "loom.readRequestFields",
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

    for required in [
        "../references/loom/uix/core.md",
        "../references/loom/uix/interaction.md",
        "../references/loom/uix/system.md",
        "../references/loom/uix/mobile.md",
        "../references/loom/uix/frameworks.md",
        "../references/loom/uix/content.md",
        "../references/loom/uix/data.md",
        "../references/loom/uix/verification.md",
        "../references/loom/uix/anti-patterns.md",
        "../references/loom/uix/scenarios/*.md",
        "../references/loom/uix/tokens/*.md",
        "../references/loom/uix/stacks/*.md",
        "writing or reviewing user-visible frontend artifacts",
        "forms, flows, search/filter, loading, empty, error, or recovery states",
        "Delivery planning, design, review, repair, and handoff rules are supplied by the current MCP request/result",
        "Do not load separate delivery reference files",
    ] {
        assert!(
            loom.contains(required),
            "opencode loom.md missing optional reference guidance {required}"
        );
    }

    for required in [
        "active_operation",
        "DeployRepairAssetsNext",
        "deploy execution repair",
        "loom.inspectRequest",
        "loom.readFieldGroup",
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
            "stacks/threejs",
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
        for name in [
            "bootstrap",
            "compose",
            "dockerfile",
            "dotnet",
            "environment",
            "external-references",
            "go",
            "java",
            "node",
            "php",
            "providers",
            "python",
            "repair",
            "ruby",
            "static",
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
