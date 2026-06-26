use setup::{
    archive_package_layout, install, package_file_names, purge, write_package_layout, AgentKind,
    ReleaseManifest, SetupEnvironment, SetupError, TargetPlatform, VERSION,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
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

    let env = fixture.env();
    let report = install(&env, &[AgentKind::Codex]).unwrap();

    assert_eq!(report.status, "ok");
    assert!(env.runtime_current().exists() || env.runtime_current().symlink_metadata().is_ok());
    assert!(env
        .agent_plugin_root(AgentKind::Codex)
        .join(".loom-mcp-install.json")
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

    install(&env, &AgentKind::all()).unwrap();

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
}

#[test]
fn archive_package_layout_rejects_legacy_typescript_runtime_entries() {
    let fixture = Fixture::new("archive_rejects_legacy_runtime");
    fixture.write_package();
    write_file(
        &fixture.package_root.join("src/ts/reference/cli.ts"),
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
            assert!(message.contains("src/ts/reference/cli.ts"));
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
