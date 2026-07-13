use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, SystemTime};
use toml_edit::{DocumentMut, Item};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PACKAGE_SCHEMA_VERSION: &str = "1.0";
pub const PLAYWRIGHT_DEFAULT_VERSION: &str = "1.61.1";
const PLAYWRIGHT_RUNTIME_SCHEMA_VERSION: &str = "1.0";
const PLAYWRIGHT_RUNTIME_REVISION: &str = "2";
const BROWSER_RUNTIME_MAX_RUNNERS: usize = 8;
const BROWSER_RUNTIME_MAX_AGE: Duration = Duration::from_secs(45 * 24 * 60 * 60);
const BROWSER_RUNTIME_CAPACITY_MIN_AGE: Duration = Duration::from_secs(7 * 24 * 60 * 60);
const BROWSER_RUNTIME_STAGING_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);
const PLAYWRIGHT_LOCK_WAIT: Duration = Duration::from_secs(10 * 60);
const PLAYWRIGHT_LOCK_STALE_AFTER: Duration = Duration::from_secs(30 * 60);
const PLAYWRIGHT_LOCK_POLL: Duration = Duration::from_millis(250);
const INSTALL_STAMP: &str = ".loom-mcp-install.json";
const SHARED_LOOM_REFERENCES: &str = "plugins/shared/loom/references";
const SHARED_DEPLOY_REFERENCES: &str = "plugins/shared/loom-deploy/references";
const REQUIRED_SHARED_REFERENCE_FILES: &[&str] = &[
    "plugins/shared/loom/references/uix/anti-patterns.md",
    "plugins/shared/loom/references/uix/content.md",
    "plugins/shared/loom/references/uix/core.md",
    "plugins/shared/loom/references/uix/data.md",
    "plugins/shared/loom/references/uix/frameworks.md",
    "plugins/shared/loom/references/uix/interaction.md",
    "plugins/shared/loom/references/uix/mobile.md",
    "plugins/shared/loom/references/uix/system.md",
    "plugins/shared/loom/references/uix/verification.md",
    "plugins/shared/loom/references/uix/scenarios/admin-dashboard.md",
    "plugins/shared/loom/references/uix/scenarios/consumer-app.md",
    "plugins/shared/loom/references/uix/scenarios/corporate-site.md",
    "plugins/shared/loom/references/uix/scenarios/data-console.md",
    "plugins/shared/loom/references/uix/scenarios/developer-tool.md",
    "plugins/shared/loom/references/uix/scenarios/docs-site.md",
    "plugins/shared/loom/references/uix/scenarios/fintech-consumer-app.md",
    "plugins/shared/loom/references/uix/scenarios/fintech-workstation.md",
    "plugins/shared/loom/references/uix/scenarios/immersive-3d.md",
    "plugins/shared/loom/references/uix/scenarios/marketing-site.md",
    "plugins/shared/loom/references/uix/scenarios/mobile-native.md",
    "plugins/shared/loom/references/uix/scenarios/mobile-responsive.md",
    "plugins/shared/loom/references/uix/stacks/native-mobile.md",
    "plugins/shared/loom/references/uix/stacks/plain-html.md",
    "plugins/shared/loom/references/uix/stacks/react.md",
    "plugins/shared/loom/references/uix/stacks/svelte.md",
    "plugins/shared/loom/references/uix/stacks/threejs.md",
    "plugins/shared/loom/references/uix/stacks/uniapp.md",
    "plugins/shared/loom/references/uix/stacks/vue.md",
    "plugins/shared/loom/references/uix/templates/tokens.css.tpl",
    "plugins/shared/loom/references/uix/templates/tokens.tailwind.tpl",
    "plugins/shared/loom/references/uix/tokens/color-system.md",
    "plugins/shared/loom/references/uix/tokens/layout-grid.md",
    "plugins/shared/loom/references/uix/tokens/motion.md",
    "plugins/shared/loom/references/uix/tokens/radius-elevation.md",
    "plugins/shared/loom/references/uix/tokens/spacing.md",
    "plugins/shared/loom/references/uix/tokens/typography.md",
    "plugins/shared/loom/references/tech/arch/adr.md",
    "plugins/shared/loom/references/tech/arch/core.md",
    "plugins/shared/loom/references/tech/arch/data.md",
    "plugins/shared/loom/references/tech/arch/failure.md",
    "plugins/shared/loom/references/tech/arch/nfr.md",
    "plugins/shared/loom/references/tech/arch/patterns.md",
    "plugins/shared/loom/references/tech/arch/system.md",
    "plugins/shared/loom/references/tech/api/contract.md",
    "plugins/shared/loom/references/tech/api/core.md",
    "plugins/shared/loom/references/tech/api/errors.md",
    "plugins/shared/loom/references/tech/api/evolution.md",
    "plugins/shared/loom/references/tech/api/operations.md",
    "plugins/shared/loom/references/tech/api/pagination.md",
    "plugins/shared/loom/references/tech/api/resource.md",
    "plugins/shared/loom/references/tech/api/security.md",
    "plugins/shared/loom/references/tech/review/core.md",
    "plugins/shared/loom/references/tech/review/defect-patterns.md",
    "plugins/shared/loom/references/tech/review/finding-quality.md",
    "plugins/shared/loom/references/tech/review/spec-compliance.md",
    "plugins/shared/loom/references/tech/review/test-evidence.md",
    "plugins/shared/loom/references/tech/test/playwright/accessibility.md",
    "plugins/shared/loom/references/tech/test/playwright/configuration.md",
    "plugins/shared/loom/references/tech/test/playwright/core.md",
    "plugins/shared/loom/references/tech/test/playwright/fixtures.md",
    "plugins/shared/loom/references/tech/test/playwright/locators.md",
    "plugins/shared/loom/references/tech/test/playwright/network.md",
    "plugins/shared/loom/references/tech/test/playwright/reliability.md",
    "plugins/shared/loom/references/tech/test/playwright/visual.md",
    "plugins/shared/loom/references/tech/backend/aspnetcore/architecture.md",
    "plugins/shared/loom/references/tech/backend/aspnetcore/data.md",
    "plugins/shared/loom/references/tech/backend/aspnetcore/minimal.md",
    "plugins/shared/loom/references/tech/backend/aspnetcore/runtime.md",
    "plugins/shared/loom/references/tech/backend/aspnetcore/security.md",
    "plugins/shared/loom/references/tech/backend/aspnetcore/testing.md",
    "plugins/shared/loom/references/tech/backend/django/models.md",
    "plugins/shared/loom/references/tech/backend/django/security.md",
    "plugins/shared/loom/references/tech/backend/django/serializers.md",
    "plugins/shared/loom/references/tech/backend/django/testing.md",
    "plugins/shared/loom/references/tech/backend/django/views.md",
    "plugins/shared/loom/references/tech/backend/fastapi/data.md",
    "plugins/shared/loom/references/tech/backend/fastapi/migration.md",
    "plugins/shared/loom/references/tech/backend/fastapi/routing.md",
    "plugins/shared/loom/references/tech/backend/fastapi/schemas.md",
    "plugins/shared/loom/references/tech/backend/fastapi/security.md",
    "plugins/shared/loom/references/tech/backend/fastapi/testing.md",
    "plugins/shared/loom/references/tech/backend/nestjs/controllers.md",
    "plugins/shared/loom/references/tech/backend/nestjs/dtos.md",
    "plugins/shared/loom/references/tech/backend/nestjs/migration.md",
    "plugins/shared/loom/references/tech/backend/nestjs/security.md",
    "plugins/shared/loom/references/tech/backend/nestjs/services.md",
    "plugins/shared/loom/references/tech/backend/nestjs/testing.md",
    "plugins/shared/loom/references/tech/backend/springboot/cloud.md",
    "plugins/shared/loom/references/tech/backend/springboot/data.md",
    "plugins/shared/loom/references/tech/backend/springboot/runtime.md",
    "plugins/shared/loom/references/tech/backend/springboot/security.md",
    "plugins/shared/loom/references/tech/backend/springboot/testing.md",
    "plugins/shared/loom/references/tech/backend/springboot/web.md",
    "plugins/shared/loom/references/tech/frontend/angular/components.md",
    "plugins/shared/loom/references/tech/frontend/angular/core.md",
    "plugins/shared/loom/references/tech/frontend/angular/ngrx.md",
    "plugins/shared/loom/references/tech/frontend/angular/routing.md",
    "plugins/shared/loom/references/tech/frontend/angular/rxjs.md",
    "plugins/shared/loom/references/tech/frontend/angular/testing.md",
    "plugins/shared/loom/references/tech/frontend/flutter/bloc.md",
    "plugins/shared/loom/references/tech/frontend/flutter/core.md",
    "plugins/shared/loom/references/tech/frontend/flutter/navigation.md",
    "plugins/shared/loom/references/tech/frontend/flutter/performance.md",
    "plugins/shared/loom/references/tech/frontend/flutter/riverpod.md",
    "plugins/shared/loom/references/tech/frontend/flutter/structure.md",
    "plugins/shared/loom/references/tech/frontend/flutter/testing.md",
    "plugins/shared/loom/references/tech/frontend/flutter/widgets.md",
    "plugins/shared/loom/references/tech/frontend/react/core.md",
    "plugins/shared/loom/references/tech/frontend/react/hooks.md",
    "plugins/shared/loom/references/tech/frontend/react/migration.md",
    "plugins/shared/loom/references/tech/frontend/react/performance.md",
    "plugins/shared/loom/references/tech/frontend/react/react19.md",
    "plugins/shared/loom/references/tech/frontend/react/server-components.md",
    "plugins/shared/loom/references/tech/frontend/react/state.md",
    "plugins/shared/loom/references/tech/frontend/react/testing.md",
    "plugins/shared/loom/references/tech/frontend/react-native/core.md",
    "plugins/shared/loom/references/tech/frontend/react-native/lists.md",
    "plugins/shared/loom/references/tech/frontend/react-native/navigation.md",
    "plugins/shared/loom/references/tech/frontend/react-native/platform.md",
    "plugins/shared/loom/references/tech/frontend/react-native/storage.md",
    "plugins/shared/loom/references/tech/frontend/react-native/structure.md",
    "plugins/shared/loom/references/tech/frontend/react-native/testing.md",
    "plugins/shared/loom/references/tech/frontend/nextjs/actions.md",
    "plugins/shared/loom/references/tech/frontend/nextjs/app-router.md",
    "plugins/shared/loom/references/tech/frontend/nextjs/core.md",
    "plugins/shared/loom/references/tech/frontend/nextjs/data.md",
    "plugins/shared/loom/references/tech/frontend/nextjs/runtime.md",
    "plugins/shared/loom/references/tech/frontend/nextjs/server-components.md",
    "plugins/shared/loom/references/tech/frontend/nextjs/testing.md",
    "plugins/shared/loom/references/tech/frontend/vue/build.md",
    "plugins/shared/loom/references/tech/frontend/vue/components.md",
    "plugins/shared/loom/references/tech/frontend/vue/core.md",
    "plugins/shared/loom/references/tech/frontend/vue/mobile.md",
    "plugins/shared/loom/references/tech/frontend/vue/nuxt.md",
    "plugins/shared/loom/references/tech/frontend/vue/state.md",
    "plugins/shared/loom/references/tech/frontend/vue/testing.md",
    "plugins/shared/loom/references/tech/frontend/vue/typescript.md",
    "plugins/shared/loom/references/tech/code/common.md",
    "plugins/shared/loom/references/tech/code/cpp/build.md",
    "plugins/shared/loom/references/tech/code/cpp/concurrency.md",
    "plugins/shared/loom/references/tech/code/cpp/core.md",
    "plugins/shared/loom/references/tech/code/cpp/modern.md",
    "plugins/shared/loom/references/tech/code/cpp/performance.md",
    "plugins/shared/loom/references/tech/code/cpp/templates.md",
    "plugins/shared/loom/references/tech/code/cpp/testing.md",
    "plugins/shared/loom/references/tech/code/csharp/aspnet.md",
    "plugins/shared/loom/references/tech/code/csharp/blazor.md",
    "plugins/shared/loom/references/tech/code/csharp/core.md",
    "plugins/shared/loom/references/tech/code/csharp/performance.md",
    "plugins/shared/loom/references/tech/code/csharp/persistence.md",
    "plugins/shared/loom/references/tech/code/csharp/testing.md",
    "plugins/shared/loom/references/tech/code/go/concurrency.md",
    "plugins/shared/loom/references/tech/code/go/core.md",
    "plugins/shared/loom/references/tech/code/go/generics.md",
    "plugins/shared/loom/references/tech/code/go/interfaces.md",
    "plugins/shared/loom/references/tech/code/go/structure.md",
    "plugins/shared/loom/references/tech/code/go/testing.md",
    "plugins/shared/loom/references/tech/code/java/core.md",
    "plugins/shared/loom/references/tech/code/java/persistence.md",
    "plugins/shared/loom/references/tech/code/java/reactive.md",
    "plugins/shared/loom/references/tech/code/java/security.md",
    "plugins/shared/loom/references/tech/code/java/spring.md",
    "plugins/shared/loom/references/tech/code/java/testing.md",
    "plugins/shared/loom/references/tech/code/javascript/async.md",
    "plugins/shared/loom/references/tech/code/javascript/browser.md",
    "plugins/shared/loom/references/tech/code/javascript/core.md",
    "plugins/shared/loom/references/tech/code/javascript/modules.md",
    "plugins/shared/loom/references/tech/code/javascript/node.md",
    "plugins/shared/loom/references/tech/code/javascript/testing.md",
    "plugins/shared/loom/references/tech/code/kotlin/compose.md",
    "plugins/shared/loom/references/tech/code/kotlin/core.md",
    "plugins/shared/loom/references/tech/code/kotlin/coroutines.md",
    "plugins/shared/loom/references/tech/code/kotlin/ktor.md",
    "plugins/shared/loom/references/tech/code/kotlin/multiplatform.md",
    "plugins/shared/loom/references/tech/code/kotlin/testing.md",
    "plugins/shared/loom/references/tech/code/php/async.md",
    "plugins/shared/loom/references/tech/code/php/core.md",
    "plugins/shared/loom/references/tech/code/php/laravel.md",
    "plugins/shared/loom/references/tech/code/php/symfony.md",
    "plugins/shared/loom/references/tech/code/php/testing.md",
    "plugins/shared/loom/references/tech/code/python/async.md",
    "plugins/shared/loom/references/tech/code/python/core.md",
    "plugins/shared/loom/references/tech/code/python/packaging.md",
    "plugins/shared/loom/references/tech/code/python/testing.md",
    "plugins/shared/loom/references/tech/code/python/typing.md",
    "plugins/shared/loom/references/tech/code/rust/async.md",
    "plugins/shared/loom/references/tech/code/rust/core.md",
    "plugins/shared/loom/references/tech/code/rust/errors.md",
    "plugins/shared/loom/references/tech/code/rust/ownership.md",
    "plugins/shared/loom/references/tech/code/rust/testing.md",
    "plugins/shared/loom/references/tech/code/rust/traits.md",
    "plugins/shared/loom/references/tech/code/sql/dialects.md",
    "plugins/shared/loom/references/tech/code/sql/optimization.md",
    "plugins/shared/loom/references/tech/code/sql/queries.md",
    "plugins/shared/loom/references/tech/code/sql/schema.md",
    "plugins/shared/loom/references/tech/code/sql/windows.md",
    "plugins/shared/loom/references/tech/code/swift/concurrency.md",
    "plugins/shared/loom/references/tech/code/swift/core.md",
    "plugins/shared/loom/references/tech/code/swift/memory.md",
    "plugins/shared/loom/references/tech/code/swift/protocols.md",
    "plugins/shared/loom/references/tech/code/swift/swiftui.md",
    "plugins/shared/loom/references/tech/code/swift/testing.md",
    "plugins/shared/loom/references/tech/code/typescript/config.md",
    "plugins/shared/loom/references/tech/code/typescript/core.md",
    "plugins/shared/loom/references/tech/code/typescript/guards.md",
    "plugins/shared/loom/references/tech/code/typescript/patterns.md",
    "plugins/shared/loom/references/tech/code/typescript/testing.md",
    "plugins/shared/loom/references/tech/code/typescript/types.md",
    "plugins/shared/loom-deploy/references/bootstrap.md",
    "plugins/shared/loom-deploy/references/compose.md",
    "plugins/shared/loom-deploy/references/dockerfile.md",
    "plugins/shared/loom-deploy/references/dotnet.md",
    "plugins/shared/loom-deploy/references/environment.md",
    "plugins/shared/loom-deploy/references/go.md",
    "plugins/shared/loom-deploy/references/java.md",
    "plugins/shared/loom-deploy/references/matrix.md",
    "plugins/shared/loom-deploy/references/node.md",
    "plugins/shared/loom-deploy/references/php.md",
    "plugins/shared/loom-deploy/references/providers.md",
    "plugins/shared/loom-deploy/references/python.md",
    "plugins/shared/loom-deploy/references/repair.md",
    "plugins/shared/loom-deploy/references/ruby.md",
    "plugins/shared/loom-deploy/references/source-model.md",
    "plugins/shared/loom-deploy/references/static.md",
    "plugins/shared/loom-deploy/references/topology.md",
    "plugins/shared/loom-deploy/references/workspaces.md",
];
const LEGACY_MARKERS: &[&str] = &[
    "~/.loom/bin/loom-cli",
    "/.loom/bin/loom-cli",
    "LOOM_AGENT_PROFILE",
    "LOOM_COMPACT_OUTPUT",
    "CLI envelope",
    "commandInvocation",
    "submitCommand",
    "Route Loom delivery, knowledge, and deploy commands through MCP",
    "Route Loom deployment commands through MCP",
    "export const LoomPlugin = async ({ client, directory })",
    "Loom MCP-only OpenCode",
    "Loom request artifacts remain the source of truth",
    "Use this reference when implementing or repairing loom deploy",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentKind {
    Codex,
    ClaudeCode,
    Opencode,
}

impl AgentKind {
    pub fn parse(raw: &str) -> Result<Self, SetupError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "codex" => Ok(Self::Codex),
            "claude" | "claude-code" | "claude_code" => Ok(Self::ClaudeCode),
            "opencode" | "open-code" | "open_code" => Ok(Self::Opencode),
            other => Err(SetupError::InvalidArgument(format!(
                "unsupported agent '{other}', expected codex, claude-code, opencode, or all"
            ))),
        }
    }

    pub fn all() -> [Self; 3] {
        [Self::Codex, Self::ClaudeCode, Self::Opencode]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude-code",
            Self::Opencode => "opencode",
        }
    }

    fn host_env(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::ClaudeCode => "claude-code",
            Self::Opencode => "opencode",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TargetPlatform {
    DarwinArm64,
    DarwinX64,
    LinuxX64,
    LinuxArm64,
    WindowsX64,
}

impl TargetPlatform {
    pub fn parse(raw: &str) -> Result<Self, SetupError> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "darwin-arm64" => Ok(Self::DarwinArm64),
            "darwin-x64" => Ok(Self::DarwinX64),
            "linux-x64" | "linux-amd64" => Ok(Self::LinuxX64),
            "linux-arm64" | "linux-aarch64" => Ok(Self::LinuxArm64),
            "windows-x64" | "windows-amd64" => Ok(Self::WindowsX64),
            other => Err(SetupError::InvalidArgument(format!(
                "unsupported platform '{other}'"
            ))),
        }
    }

    pub fn all() -> [Self; 5] {
        [
            Self::DarwinArm64,
            Self::DarwinX64,
            Self::LinuxX64,
            Self::LinuxArm64,
            Self::WindowsX64,
        ]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::DarwinArm64 => "darwin-arm64",
            Self::DarwinX64 => "darwin-x64",
            Self::LinuxX64 => "linux-x64",
            Self::LinuxArm64 => "linux-arm64",
            Self::WindowsX64 => "windows-x64",
        }
    }

    pub fn package_file_name(self, version: &str) -> String {
        let extension = if matches!(self, Self::WindowsX64) {
            "zip"
        } else {
            "tar.gz"
        };
        format!(
            "loom-{version}-{platform}.{extension}",
            platform = self.as_str()
        )
    }

    pub fn package_checksum_file_name(self, version: &str) -> String {
        format!("{}.sha256", self.package_file_name(version))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseManifest {
    pub schema_version: String,
    pub version: String,
    pub platform: String,
    pub binaries: BinaryManifest,
    pub python: PythonManifest,
    pub plugins: PluginManifest,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BinaryManifest {
    pub mcp_server: String,
    pub setup: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PythonManifest {
    pub runtime: String,
    pub algorithms: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PluginManifest {
    pub codex: String,
    pub claude_code: String,
    pub opencode: String,
}

impl ReleaseManifest {
    pub fn for_platform(platform: TargetPlatform) -> Self {
        Self {
            schema_version: PACKAGE_SCHEMA_VERSION.to_string(),
            version: VERSION.to_string(),
            platform: platform.as_str().to_string(),
            binaries: BinaryManifest {
                mcp_server: executable_path("bin/loom-mcp-server"),
                setup: executable_path("bin/loom-setup"),
            },
            python: PythonManifest {
                runtime: "python/runtime".to_string(),
                algorithms: "python/algorithms".to_string(),
            },
            plugins: PluginManifest {
                codex: "plugins/codex".to_string(),
                claude_code: "plugins/claude-code".to_string(),
                opencode: "plugins/opencode".to_string(),
            },
        }
    }

    fn plugin_path(&self, agent: AgentKind) -> &str {
        match agent {
            AgentKind::Codex => &self.plugins.codex,
            AgentKind::ClaudeCode => &self.plugins.claude_code,
            AgentKind::Opencode => &self.plugins.opencode,
        }
    }
}

fn executable_path(base: &str) -> String {
    if cfg!(windows) {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

#[derive(Debug, Clone)]
pub struct SetupEnvironment {
    pub loom_home: PathBuf,
    pub user_home: PathBuf,
    pub package_root: PathBuf,
    pub codex_home: PathBuf,
    pub claude_home: PathBuf,
    pub opencode_home: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserRuntimePrepareOptions {
    pub requested_versions: Vec<String>,
    pub requested_browsers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub npm_program: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node_program: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_program: Option<PathBuf>,
}

impl Default for BrowserRuntimePrepareOptions {
    fn default() -> Self {
        Self {
            requested_versions: vec![PLAYWRIGHT_DEFAULT_VERSION.to_string()],
            requested_browsers: vec!["chromium".to_string()],
            npm_program: None,
            node_program: None,
            container_program: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserRuntimePrepareReport {
    pub status: String,
    pub platform: String,
    pub cache_root: String,
    pub browsers_path: String,
    pub runtimes: Vec<BrowserRuntimeEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserRuntimeEntry {
    pub status: String,
    pub backend: String,
    pub runtime_id: String,
    pub requested_version: String,
    pub resolved_version: String,
    pub platform: String,
    pub browsers: Vec<String>,
    pub runner_path: String,
    pub manifest_path: String,
    pub reused: bool,
    pub doctor_checks: Vec<BrowserRuntimeDoctorCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub managed_container: Option<ManagedBrowserContainer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedBrowserContainer {
    pub image: String,
    pub browser_path: String,
    pub project_mount_path: String,
    pub host_gateway: String,
    pub command_prefix: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BrowserRuntimeManifest {
    schema_version: String,
    runtime_revision: String,
    runtime_id: String,
    requested_version: String,
    resolved_version: String,
    platform: String,
    browsers: Vec<String>,
    package_lock_checksum: String,
    runner_relative_path: String,
    browser_entries: Vec<String>,
    prepared_at: String,
    last_used_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserRuntimeDoctorCheck {
    pub check_id: String,
    pub scope: String,
    pub status: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub diagnostic: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remediation: Option<String>,
}

impl SetupEnvironment {
    pub fn from_env(package_root_arg: Option<PathBuf>) -> Result<Self, SetupError> {
        let user_home = env_path("LOOM_SETUP_USER_HOME")
            .or_else(|| env_path("HOME"))
            .or_else(|| env_path("USERPROFILE"))
            .ok_or_else(|| SetupError::InvalidArgument("HOME/USERPROFILE is required".into()))?;
        let loom_home = env_path("LOOM_HOME").unwrap_or_else(|| user_home.join(".loom"));
        let package_root = package_root_arg
            .or_else(|| env_path("LOOM_SETUP_PACKAGE_ROOT"))
            .or_else(package_root_from_current_exe)
            .ok_or_else(|| {
                SetupError::InvalidArgument(
                    "package root is required; set LOOM_SETUP_PACKAGE_ROOT or pass --package-root"
                        .into(),
                )
            })?;
        let codex_home = env_path("CODEX_HOME").unwrap_or_else(|| user_home.join(".codex"));
        let claude_home = env_path("CLAUDE_HOME").unwrap_or_else(|| user_home.join(".claude"));
        let opencode_home =
            env_path("OPENCODE_CONFIG_HOME").unwrap_or_else(|| user_home.join(".config/opencode"));
        Ok(Self {
            loom_home,
            user_home,
            package_root,
            codex_home,
            claude_home,
            opencode_home,
        })
    }

    pub fn for_test(user_home: PathBuf, loom_home: PathBuf, package_root: PathBuf) -> Self {
        Self {
            codex_home: user_home.join(".codex"),
            claude_home: user_home.join(".claude"),
            opencode_home: user_home.join(".config/opencode"),
            user_home,
            loom_home,
            package_root,
        }
    }

    pub fn bin_dir(&self) -> PathBuf {
        self.loom_home.join("bin")
    }

    pub fn runtime_root(&self) -> PathBuf {
        self.loom_home.join("runtime")
    }

    pub fn runtime_current(&self) -> PathBuf {
        self.runtime_root().join("current")
    }

    pub fn playwright_cache_root(&self) -> PathBuf {
        self.loom_home.join("runtime-cache/playwright")
    }

    pub fn install_registry_path(&self) -> PathBuf {
        self.loom_home.join("install-registry.json")
    }

    pub fn agent_session_root(&self, agent: AgentKind) -> PathBuf {
        self.loom_home.join("agent-sessions").join(agent.as_str())
    }

    pub fn agent_plugin_root(&self, agent: AgentKind) -> PathBuf {
        match agent {
            AgentKind::Codex => self.user_home.join("plugins/loom"),
            AgentKind::ClaudeCode => self.claude_home.join("skills/loom"),
            AgentKind::Opencode => self.opencode_home.join("plugins/loom.js"),
        }
    }

    pub fn agent_mcp_registration_path(&self, agent: AgentKind) -> PathBuf {
        match agent {
            AgentKind::Codex => self.codex_home.join("mcp/loom.json"),
            AgentKind::ClaudeCode => self.claude_home.join("mcp/loom.json"),
            AgentKind::Opencode => self.opencode_home.join("mcp/loom.json"),
        }
    }

    pub fn codex_config_path(&self) -> PathBuf {
        self.codex_home.join("config.toml")
    }

    pub fn claude_config_path(&self) -> PathBuf {
        self.user_home.join(".claude.json")
    }

    pub fn opencode_config_path(&self) -> PathBuf {
        self.opencode_home.join("opencode.jsonc")
    }

    pub fn common_registration_path(&self, agent: AgentKind) -> PathBuf {
        self.loom_home
            .join("mcp-registrations")
            .join(agent.as_str())
            .join("loom.json")
    }
}

fn env_path(name: &str) -> Option<PathBuf> {
    std::env::var_os(name)
        .map(PathBuf::from)
        .filter(|path| !path.as_os_str().is_empty())
}

fn package_root_from_current_exe() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    exe.parent()?.parent().map(Path::to_path_buf)
}

fn repo_root() -> Result<PathBuf, SetupError> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .map(Path::to_path_buf)
        .ok_or_else(|| SetupError::InvalidArgument("failed to resolve repository root".into()))
}

fn current_binary_dir() -> Result<PathBuf, SetupError> {
    if let Some(path) = env_path("LOOM_SETUP_BINARY_DIR") {
        return Ok(path);
    }
    let exe = std::env::current_exe().map_err(|source| SetupError::Io {
        path: PathBuf::from("<current_exe>"),
        source,
    })?;
    let dir = exe
        .parent()
        .ok_or_else(|| SetupError::InvalidArgument("current executable has no parent".into()))?;
    if dir.file_name().and_then(|name| name.to_str()) == Some("deps") {
        return dir.parent().map(Path::to_path_buf).ok_or_else(|| {
            SetupError::InvalidArgument("failed to resolve cargo target directory".into())
        });
    }
    Ok(dir.to_path_buf())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupReport {
    pub status: String,
    pub command: String,
    pub version: String,
    pub agents: Vec<String>,
    pub installed_runtime: Option<String>,
    pub removed: Vec<String>,
    pub blocked: Vec<LegacyBlockedPath>,
    pub checks: Vec<DoctorCheck>,
}

impl SetupReport {
    fn new(command: &str, agents: &[AgentKind]) -> Self {
        Self {
            status: "ok".to_string(),
            command: command.to_string(),
            version: VERSION.to_string(),
            agents: agents
                .iter()
                .map(|agent| agent.as_str().to_string())
                .collect(),
            installed_runtime: None,
            removed: Vec::new(),
            blocked: Vec::new(),
            checks: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyBlockedPath {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctorCheck {
    pub name: String,
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct InstallRegistry {
    pub schema_version: u32,
    pub current_version: Option<String>,
    pub runtime_current: Option<String>,
    pub installed_agents: BTreeMap<String, InstalledAgent>,
    pub legacy_cleanup: Option<LegacyCleanupRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledAgent {
    pub agent: String,
    pub plugin_root: String,
    pub mcp_registration: String,
    pub installed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LegacyCleanupRecord {
    pub ran_at: String,
    pub agents: Vec<String>,
    pub removed: Vec<String>,
    pub blocked: Vec<LegacyBlockedPath>,
    pub kept_shared_cli_launcher: bool,
}

#[derive(Debug)]
pub enum SetupError {
    InvalidArgument(String),
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    Toml {
        path: PathBuf,
        source: toml_edit::TomlError,
    },
    ChecksumMismatch {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    MissingPackageEntry(PathBuf),
    LegacyCleanupBlocked(Vec<LegacyBlockedPath>),
    DoctorFailed(Vec<DoctorCheck>),
    CommandFailed {
        program: String,
        status: i32,
        stdout: String,
        stderr: String,
    },
}

impl fmt::Display for SetupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument(message) => write!(formatter, "{message}"),
            Self::Io { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Json { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::Toml { path, source } => write!(formatter, "{}: {source}", path.display()),
            Self::ChecksumMismatch {
                path,
                expected,
                actual,
            } => write!(
                formatter,
                "checksum mismatch for {}: expected {expected}, got {actual}",
                path.display()
            ),
            Self::MissingPackageEntry(path) => {
                write!(
                    formatter,
                    "release package entry is missing: {}",
                    path.display()
                )
            }
            Self::LegacyCleanupBlocked(blocked) => {
                write!(
                    formatter,
                    "legacy cleanup blocked for {} path(s)",
                    blocked.len()
                )
            }
            Self::DoctorFailed(checks) => write!(
                formatter,
                "doctor failed: {} check(s) did not pass",
                checks
                    .iter()
                    .filter(|check| check.status != "passed")
                    .count()
            ),
            Self::CommandFailed {
                program,
                status,
                stderr,
                ..
            } => write!(
                formatter,
                "{program} failed with exit status {status}: {}",
                stderr.trim()
            ),
        }
    }
}

impl std::error::Error for SetupError {}

pub fn parse_agent_selection(raw: &str) -> Result<Vec<AgentKind>, SetupError> {
    if raw.trim().eq_ignore_ascii_case("all") {
        return Ok(AgentKind::all().to_vec());
    }
    Ok(vec![AgentKind::parse(raw)?])
}

pub fn prepare_browser_runtime(
    env: &SetupEnvironment,
    options: &BrowserRuntimePrepareOptions,
) -> Result<BrowserRuntimePrepareReport, SetupError> {
    let platform = browser_runtime_platform();
    let cache_root = env.playwright_cache_root().join(&platform);
    let runners_root = cache_root.join("runners");
    let browsers_path = cache_root.join("browsers");
    let locks_root = cache_root.join("locks");
    let staging_root = cache_root.join("staging");
    for path in [&runners_root, &browsers_path, &locks_root, &staging_root] {
        fs::create_dir_all(path).map_err(|source| SetupError::Io {
            path: path.to_path_buf(),
            source,
        })?;
    }
    let mut versions = options
        .requested_versions
        .iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>();
    if versions.is_empty() {
        versions.insert(PLAYWRIGHT_DEFAULT_VERSION.to_string());
    }
    let mut browsers = options
        .requested_browsers
        .iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>();
    if browsers.is_empty() {
        browsers.insert("chromium".to_string());
    }
    for browser in &browsers {
        if !matches!(browser.as_str(), "chromium" | "firefox" | "webkit") {
            return Err(SetupError::InvalidArgument(format!(
                "unsupported Playwright browser `{browser}`; use chromium, firefox, or webkit"
            )));
        }
    }
    let browsers = browsers.into_iter().collect::<Vec<_>>();
    let npm_program = options
        .npm_program
        .clone()
        .unwrap_or_else(|| PathBuf::from(if cfg!(windows) { "npm.cmd" } else { "npm" }));
    let node_program = options
        .node_program
        .clone()
        .unwrap_or_else(|| PathBuf::from(if cfg!(windows) { "node.exe" } else { "node" }));
    let container_program = options.container_program.clone().unwrap_or_else(|| {
        PathBuf::from(if cfg!(windows) {
            "docker.exe"
        } else {
            "docker"
        })
    });
    let mut runtimes = Vec::new();
    for requested_version in versions {
        validate_playwright_version_spec(&requested_version)?;
        runtimes.push(prepare_browser_runtime_version(
            &requested_version,
            &platform,
            &browsers,
            &npm_program,
            &node_program,
            &container_program,
            &runners_root,
            &browsers_path,
            &locks_root,
            &staging_root,
        )?);
    }
    garbage_collect_browser_runtime_cache(
        &runners_root,
        &browsers_path,
        &staging_root,
        &locks_root,
        &runtimes
            .iter()
            .map(|runtime| runtime.runtime_id.clone())
            .collect::<BTreeSet<_>>(),
    )?;
    let ready_count = runtimes
        .iter()
        .filter(|runtime| runtime.status == "ready")
        .count();
    let status = if runtimes.is_empty() {
        "unavailable"
    } else if ready_count == runtimes.len() {
        "ready"
    } else if ready_count > 0 {
        "partial"
    } else {
        "unavailable"
    };
    Ok(BrowserRuntimePrepareReport {
        status: status.to_string(),
        platform,
        cache_root: path_string(&cache_root),
        browsers_path: path_string(&browsers_path),
        runtimes,
    })
}

fn prepare_browser_runtime_version(
    requested_version: &str,
    platform: &str,
    browsers: &[String],
    npm_program: &Path,
    node_program: &Path,
    container_program: &Path,
    runners_root: &Path,
    browsers_path: &Path,
    locks_root: &Path,
    staging_root: &Path,
) -> Result<BrowserRuntimeEntry, SetupError> {
    let exact_requested = exact_playwright_version(requested_version);
    let resolution_id = playwright_resolution_id(requested_version, platform, browsers);
    let _resolution_lock = (!exact_requested)
        .then(|| {
            BrowserRuntimeLock::acquire(&locks_root.join(format!("resolve-{resolution_id}.lock")))
        })
        .transpose()?;
    let exact_runtime_id = exact_requested.then(|| {
        playwright_runtime_id(
            requested_version.trim_start_matches('v'),
            platform,
            browsers,
        )
    });
    let mut final_lock = if let Some(runtime_id) = &exact_runtime_id {
        Some(BrowserRuntimeLock::acquire(
            &locks_root.join(format!("{runtime_id}.lock")),
        )?)
    } else {
        None
    };
    if let Some(runtime_id) = &exact_runtime_id {
        let runtime_root = runners_root.join(runtime_id);
        if let Some(mut entry) = reusable_browser_runtime(
            &runtime_root,
            browsers_path,
            node_program,
            container_program,
        )? {
            entry.requested_version = requested_version.to_string();
            return Ok(entry);
        }
    }
    let staging = staging_root.join(format!(
        "{}-{}-{}",
        resolution_id,
        std::process::id(),
        chrono::Utc::now().timestamp_millis()
    ));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|source| SetupError::Io {
            path: staging.clone(),
            source,
        })?;
    }
    fs::create_dir_all(&staging).map_err(|source| SetupError::Io {
        path: staging.clone(),
        source,
    })?;
    let prepared = (|| {
        write_json(
            &staging.join("package.json"),
            &json!({
                "name": format!("loom-playwright-runtime-{resolution_id}"),
                "private": true,
                "version": "1.0.0"
            }),
        )?;
        let package_spec = format!("@playwright/test@{requested_version}");
        run_runtime_command(
            npm_program,
            &[
                "install",
                "--save-exact",
                "--ignore-scripts",
                "--no-audit",
                "--no-fund",
                package_spec.as_str(),
            ],
            &staging,
            &[],
        )?;
        let installed_package = staging.join("node_modules/@playwright/test/package.json");
        let installed: Value = read_json_value(&installed_package)?;
        let resolved_version = installed
            .get("version")
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                SetupError::InvalidArgument(format!(
                    "installed Playwright package has no version: {}",
                    installed_package.display()
                ))
            })?
            .to_string();
        let runtime_id = playwright_runtime_id(&resolved_version, platform, browsers);
        if final_lock.is_none() {
            final_lock = Some(BrowserRuntimeLock::acquire(
                &locks_root.join(format!("{runtime_id}.lock")),
            )?);
        }
        let runtime_root = runners_root.join(&runtime_id);
        if let Some(mut entry) = reusable_browser_runtime(
            &runtime_root,
            browsers_path,
            node_program,
            container_program,
        )? {
            entry.requested_version = requested_version.to_string();
            return Ok(entry);
        }
        if runtime_root.exists() {
            fs::remove_dir_all(&runtime_root).map_err(|source| SetupError::Io {
                path: runtime_root.clone(),
                source,
            })?;
        }
        let runner_relative_path = if cfg!(windows) {
            "node_modules/.bin/playwright.cmd"
        } else {
            "node_modules/.bin/playwright"
        };
        let runner = staging.join(runner_relative_path);
        if !runner.is_file() {
            return Err(SetupError::MissingPackageEntry(runner));
        }
        let browser_entries = {
            let _browser_cache_lock =
                BrowserRuntimeLock::acquire(&locks_root.join("browser-cache.lock"))?;
            let mut install_args = vec!["install"];
            install_args.extend(browsers.iter().map(String::as_str));
            run_runtime_command(
                &runner,
                &install_args,
                &staging,
                &[("PLAYWRIGHT_BROWSERS_PATH", browsers_path)],
            )?;
            browser_cache_entries_for_runtime(&staging, browsers_path, browsers)?
        };
        if browser_entries.is_empty() {
            return Err(SetupError::InvalidArgument(format!(
                "Playwright browser install produced no cache entries under {}",
                browsers_path.display()
            )));
        }
        let package_lock = staging.join("package-lock.json");
        let package_lock_checksum = sha256_file(&package_lock)?;
        write_json(
            &staging.join("manifest.json"),
            &BrowserRuntimeManifest {
                schema_version: PLAYWRIGHT_RUNTIME_SCHEMA_VERSION.to_string(),
                runtime_revision: PLAYWRIGHT_RUNTIME_REVISION.to_string(),
                runtime_id: runtime_id.clone(),
                requested_version: requested_version.to_string(),
                resolved_version,
                platform: platform.to_string(),
                browsers: browsers.to_vec(),
                package_lock_checksum,
                runner_relative_path: runner_relative_path.to_string(),
                browser_entries,
                prepared_at: now_string(),
                last_used_at: now_string(),
            },
        )?;
        fs::rename(&staging, &runtime_root).map_err(|source| SetupError::Io {
            path: runtime_root.clone(),
            source,
        })?;
        let mut entry = reusable_browser_runtime(
            &runtime_root,
            browsers_path,
            node_program,
            container_program,
        )?
        .ok_or_else(|| {
            SetupError::InvalidArgument(format!(
                "prepared Playwright runtime failed doctor checks: {}",
                runtime_root.display()
            ))
        })?;
        entry.reused = false;
        Ok(entry)
    })();
    if staging.exists() {
        let _ = fs::remove_dir_all(&staging);
    }
    prepared
}

fn reusable_browser_runtime(
    runtime_root: &Path,
    browsers_path: &Path,
    node_program: &Path,
    container_program: &Path,
) -> Result<Option<BrowserRuntimeEntry>, SetupError> {
    let manifest_path = runtime_root.join("manifest.json");
    if !manifest_path.is_file() {
        return Ok(None);
    }
    let manifest: BrowserRuntimeManifest = serde_json::from_value(read_json_value(&manifest_path)?)
        .map_err(|source| SetupError::Json {
            path: manifest_path.clone(),
            source,
        })?;
    let checks = browser_runtime_doctor(runtime_root, browsers_path, &manifest, node_program);
    if checks
        .iter()
        .any(|check| check.scope == "integrity" && check.status != "passed")
    {
        return Ok(None);
    }
    let mut manifest = manifest;
    manifest.last_used_at = now_string();
    write_json(&manifest_path, &manifest)?;
    let host_ready = checks
        .iter()
        .filter(|check| check.scope == "launch")
        .all(|check| check.status == "passed");
    let (container_checks, managed_container) = if host_ready {
        (Vec::new(), None)
    } else {
        managed_container_doctor(
            runtime_root,
            &manifest.resolved_version,
            &manifest.browsers,
            container_program,
        )
    };
    let container_ready = managed_container.is_some();
    let mut checks = checks;
    checks.extend(container_checks);
    Ok(Some(BrowserRuntimeEntry {
        status: if host_ready || container_ready {
            "ready".to_string()
        } else {
            "unavailable".to_string()
        },
        backend: if host_ready {
            "host".to_string()
        } else if container_ready {
            "managed_container".to_string()
        } else {
            "unavailable".to_string()
        },
        runtime_id: manifest.runtime_id,
        requested_version: manifest.requested_version,
        resolved_version: manifest.resolved_version,
        platform: manifest.platform,
        browsers: manifest.browsers,
        runner_path: path_string(runtime_root.join(manifest.runner_relative_path)),
        manifest_path: path_string(manifest_path),
        reused: true,
        doctor_checks: checks,
        managed_container,
    }))
}

fn browser_runtime_doctor(
    runtime_root: &Path,
    browsers_path: &Path,
    manifest: &BrowserRuntimeManifest,
    node_program: &Path,
) -> Vec<BrowserRuntimeDoctorCheck> {
    let mut checks = Vec::new();
    checks.push(browser_doctor_check(
        "manifest_version",
        manifest.schema_version == PLAYWRIGHT_RUNTIME_SCHEMA_VERSION
            && manifest.runtime_revision == PLAYWRIGHT_RUNTIME_REVISION,
        "Runtime manifest schema and Loom runtime revision match.",
    ));
    let lock_path = runtime_root.join("package-lock.json");
    let lock_matches = sha256_file(&lock_path)
        .map(|actual| actual == manifest.package_lock_checksum)
        .unwrap_or(false);
    checks.push(browser_doctor_check(
        "package_lock_checksum",
        lock_matches,
        "Runtime package lock checksum matches the manifest.",
    ));
    let installed_package = runtime_root.join("node_modules/@playwright/test/package.json");
    let installed_version_matches = read_json_value(&installed_package)
        .ok()
        .and_then(|value| {
            value
                .get("version")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .is_some_and(|version| version == manifest.resolved_version);
    checks.push(browser_doctor_check(
        "runner_package_version",
        installed_version_matches,
        "Installed @playwright/test version matches the manifest.",
    ));
    checks.push(browser_doctor_check(
        "runner_executable",
        runtime_root.join(&manifest.runner_relative_path).is_file(),
        "Playwright runner executable is present.",
    ));
    let browsers_present = !manifest.browser_entries.is_empty()
        && manifest
            .browser_entries
            .iter()
            .all(|entry| browsers_path.join(entry).exists());
    checks.push(browser_doctor_check(
        "browser_cache",
        browsers_present,
        "Required Playwright browser cache entries are present.",
    ));
    if checks.iter().all(|check| check.status == "passed") {
        checks.extend(manifest.browsers.iter().map(|browser| {
            browser_launch_doctor_check(runtime_root, browsers_path, node_program, browser)
        }));
    }
    checks
}

fn browser_doctor_check(check_id: &str, passed: bool, summary: &str) -> BrowserRuntimeDoctorCheck {
    BrowserRuntimeDoctorCheck {
        check_id: check_id.to_string(),
        scope: "integrity".to_string(),
        status: if passed { "passed" } else { "failed" }.to_string(),
        summary: summary.to_string(),
        failure_code: (!passed).then(|| "runtime_integrity_failed".to_string()),
        diagnostic: None,
        remediation: (!passed)
            .then(|| "Rebuild the Loom-managed Playwright runtime cache.".to_string()),
    }
}

fn browser_launch_doctor_check(
    runtime_root: &Path,
    browsers_path: &Path,
    node_program: &Path,
    browser: &str,
) -> BrowserRuntimeDoctorCheck {
    let script = format!(
        "const {{ {browser} }} = require('playwright'); (async () => {{ const instance = await {browser}.launch({{headless:true}}); const page = await instance.newPage(); await page.setContent('<main>loom-browser-smoke</main>'); if ((await page.textContent('main')) !== 'loom-browser-smoke') throw new Error('smoke content mismatch'); await instance.close(); }})().catch(error => {{ console.error(error && error.stack ? error.stack : String(error)); process.exit(1); }});"
    );
    let mut command = Command::new(node_program);
    command
        .current_dir(runtime_root)
        .args(["-e", script.as_str()])
        .env("PLAYWRIGHT_BROWSERS_PATH", browsers_path);
    match command.output() {
        Ok(output) if output.status.success() => BrowserRuntimeDoctorCheck {
            check_id: format!("launch_smoke_{browser}"),
            scope: "launch".to_string(),
            status: "passed".to_string(),
            summary: format!("{browser} launched, rendered a page, and closed successfully."),
            failure_code: None,
            diagnostic: None,
            remediation: None,
        },
        Ok(output) => {
            let diagnostic = bounded_command_output(&output.stderr);
            let failure_code = classify_browser_launch_failure(&diagnostic);
            BrowserRuntimeDoctorCheck {
                check_id: format!("launch_smoke_{browser}"),
                scope: "launch".to_string(),
                status: "failed".to_string(),
                summary: format!("{browser} could not launch on the host."),
                failure_code: Some(failure_code.to_string()),
                diagnostic: Some(diagnostic),
                remediation: Some(browser_launch_remediation(failure_code).to_string()),
            }
        }
        Err(error) => BrowserRuntimeDoctorCheck {
            check_id: format!("launch_smoke_{browser}"),
            scope: "launch".to_string(),
            status: "failed".to_string(),
            summary: format!("{browser} launch doctor could not start Node.js."),
            failure_code: Some("node_runtime_unavailable".to_string()),
            diagnostic: Some(error.to_string()),
            remediation: Some(
                "Install a compatible Node.js runtime or make it available on PATH.".to_string(),
            ),
        },
    }
}

fn classify_browser_launch_failure(diagnostic: &str) -> &'static str {
    let lower = diagnostic.to_ascii_lowercase();
    if lower.contains("missing dependencies")
        || lower.contains("missing shared libraries")
        || lower.contains("error while loading shared libraries")
    {
        "missing_system_dependencies"
    } else if lower.contains("executable doesn't exist")
        || lower.contains("executable does not exist")
    {
        "browser_executable_missing"
    } else if lower.contains("permission denied") || lower.contains("eacces") {
        "browser_launch_permission_denied"
    } else if lower.contains("wrong architecture") || lower.contains("bad cpu type") {
        "browser_platform_mismatch"
    } else {
        "browser_launch_failed"
    }
}

fn browser_launch_remediation(failure_code: &str) -> &'static str {
    match failure_code {
        "missing_system_dependencies" => {
            "Use Loom's managed Playwright container fallback or install the host browser system dependencies."
        }
        "browser_executable_missing" => "Reprepare the Loom browser runtime for this platform.",
        "browser_launch_permission_denied" => {
            "Correct host execution permissions or use Loom's managed Playwright container fallback."
        }
        "browser_platform_mismatch" => "Prepare the runtime on the current OS and CPU architecture.",
        _ => "Inspect the bounded launch diagnostic and use Loom's managed Playwright container fallback.",
    }
}

fn managed_container_doctor(
    runtime_root: &Path,
    resolved_version: &str,
    browsers: &[String],
    container_program: &Path,
) -> (
    Vec<BrowserRuntimeDoctorCheck>,
    Option<ManagedBrowserContainer>,
) {
    let mut last_failure = None;
    for distribution in ["noble", "jammy"] {
        let image = format!("mcr.microsoft.com/playwright:v{resolved_version}-{distribution}");
        match ensure_container_image(container_program, &image) {
            Ok(()) => {
                let script = browsers
                    .iter()
                    .map(|browser| {
                        format!(
                            "const {{ {browser} }} = require('/loom-runner/node_modules/playwright'); const b_{browser} = await {browser}.launch({{headless:true}}); const p_{browser} = await b_{browser}.newPage(); await p_{browser}.setContent('<main>loom-container-smoke</main>'); await b_{browser}.close();"
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                let script = format!(
                    "(async () => {{ {script} }})().catch(error => {{ console.error(error && error.stack ? error.stack : String(error)); process.exit(1); }});"
                );
                let mount = format!("{}:/loom-runner:ro", runtime_root.display());
                let output = Command::new(container_program)
                    .args([
                        "run",
                        "--rm",
                        "-v",
                        mount.as_str(),
                        "-e",
                        "PLAYWRIGHT_BROWSERS_PATH=/ms-playwright",
                        "--entrypoint",
                        "node",
                        image.as_str(),
                        "-e",
                        script.as_str(),
                    ])
                    .output();
                match output {
                    Ok(output) if output.status.success() => {
                        let descriptor = ManagedBrowserContainer {
                            image: image.clone(),
                            browser_path: "/ms-playwright".to_string(),
                            project_mount_path: "/work".to_string(),
                            host_gateway: "host.docker.internal".to_string(),
                            command_prefix: vec![
                                path_string(container_program),
                                "run".to_string(),
                                "--rm".to_string(),
                                "--add-host".to_string(),
                                "host.docker.internal:host-gateway".to_string(),
                                "-v".to_string(),
                                "${PROJECT_ROOT}:/work".to_string(),
                                "-w".to_string(),
                                "/work".to_string(),
                                "-e".to_string(),
                                "PLAYWRIGHT_BROWSERS_PATH=/ms-playwright".to_string(),
                                image,
                            ],
                        };
                        return (
                            vec![BrowserRuntimeDoctorCheck {
                                check_id: "managed_container_smoke".to_string(),
                                scope: "container".to_string(),
                                status: "passed".to_string(),
                                summary: "Managed Playwright container launched every requested browser successfully.".to_string(),
                                failure_code: None,
                                diagnostic: None,
                                remediation: None,
                            }],
                            Some(descriptor),
                        );
                    }
                    Ok(output) => {
                        last_failure = Some((
                            "managed_container_launch_failed",
                            bounded_command_output(&output.stderr),
                        ));
                    }
                    Err(error) => {
                        last_failure = Some(("container_runtime_unavailable", error.to_string()));
                    }
                }
            }
            Err((code, diagnostic)) => last_failure = Some((code, diagnostic)),
        }
    }
    let (failure_code, diagnostic) = last_failure.unwrap_or((
        "container_runtime_unavailable",
        "No managed container runtime candidate was available.".to_string(),
    ));
    (
        vec![BrowserRuntimeDoctorCheck {
            check_id: "managed_container_smoke".to_string(),
            scope: "container".to_string(),
            status: "failed".to_string(),
            summary: "Managed Playwright container fallback is unavailable.".to_string(),
            failure_code: Some(failure_code.to_string()),
            diagnostic: Some(diagnostic),
            remediation: Some(
                "Start a Docker-compatible container runtime, restore registry access, or provide external browser evidence."
                    .to_string(),
            ),
        }],
        None,
    )
}

fn ensure_container_image(
    container_program: &Path,
    image: &str,
) -> Result<(), (&'static str, String)> {
    match Command::new(container_program)
        .args(["image", "inspect", image])
        .output()
    {
        Ok(output) if output.status.success() => return Ok(()),
        Err(error) => return Err(("container_runtime_unavailable", error.to_string())),
        Ok(_) => {}
    }
    match Command::new(container_program)
        .args(["pull", image])
        .output()
    {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) => Err((
            "managed_container_image_unavailable",
            bounded_command_output(&output.stderr),
        )),
        Err(error) => Err(("container_runtime_unavailable", error.to_string())),
    }
}

fn validate_playwright_version_spec(value: &str) -> Result<(), SetupError> {
    let lower = value.to_ascii_lowercase();
    if value.is_empty()
        || value.chars().any(char::is_whitespace)
        || lower.starts_with("file:")
        || lower.starts_with("link:")
        || lower.starts_with("workspace:")
        || lower.starts_with("git")
        || lower.starts_with("http:")
        || lower.starts_with("https:")
    {
        return Err(SetupError::InvalidArgument(format!(
            "unsupported Playwright version spec `{value}`; use a registry version, range, or tag"
        )));
    }
    Ok(())
}

fn exact_playwright_version(value: &str) -> bool {
    let core = value
        .trim()
        .trim_start_matches('v')
        .split(['-', '+'])
        .next()
        .unwrap_or_default();
    let parts = core.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty() && part.chars().all(|character| character.is_ascii_digit())
        })
}

fn browser_runtime_platform() -> String {
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

fn playwright_resolution_id(
    requested_version: &str,
    platform: &str,
    browsers: &[String],
) -> String {
    runtime_hash_id("resolve", requested_version, platform, browsers)
}

fn playwright_runtime_id(resolved_version: &str, platform: &str, browsers: &[String]) -> String {
    runtime_hash_id("pw", resolved_version, platform, browsers)
}

fn runtime_hash_id(prefix: &str, version: &str, platform: &str, browsers: &[String]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(PLAYWRIGHT_RUNTIME_REVISION.as_bytes());
    hasher.update([0]);
    hasher.update(version.as_bytes());
    hasher.update([0]);
    hasher.update(platform.as_bytes());
    for browser in browsers {
        hasher.update([0]);
        hasher.update(browser.as_bytes());
    }
    let digest = format!("{:x}", hasher.finalize());
    format!("{prefix}-{}", &digest[..16])
}

fn run_runtime_command(
    program: &Path,
    args: &[&str],
    cwd: &Path,
    envs: &[(&str, &Path)],
) -> Result<(), SetupError> {
    let mut command = Command::new(program);
    command.current_dir(cwd).args(args);
    for (name, value) in envs {
        command.env(name, value);
    }
    let output = command.output().map_err(|source| SetupError::Io {
        path: program.to_path_buf(),
        source,
    })?;
    if output.status.success() {
        return Ok(());
    }
    Err(SetupError::CommandFailed {
        program: path_string(program),
        status: output.status.code().unwrap_or(-1),
        stdout: bounded_command_output(&output.stdout),
        stderr: bounded_command_output(&output.stderr),
    })
}

fn bounded_command_output(bytes: &[u8]) -> String {
    const LIMIT: usize = 16 * 1024;
    let start = bytes.len().saturating_sub(LIMIT);
    String::from_utf8_lossy(&bytes[start..]).into_owned()
}

fn directory_entry_names(path: &Path) -> Result<Vec<String>, SetupError> {
    let mut entries = fs::read_dir(path)
        .map_err(|source| SetupError::Io {
            path: path.to_path_buf(),
            source,
        })?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect::<Vec<_>>();
    entries.sort();
    Ok(entries)
}

fn browser_cache_entries_for_runtime(
    runtime_root: &Path,
    browsers_path: &Path,
    requested_browsers: &[String],
) -> Result<Vec<String>, SetupError> {
    let browser_manifest = runtime_root.join("node_modules/playwright-core/browsers.json");
    let expected = read_json_value(&browser_manifest)
        .ok()
        .and_then(|value| value.get("browsers").and_then(Value::as_array).cloned())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|browser| {
            let name = browser.get("name").and_then(Value::as_str)?;
            let revision = browser.get("revision").and_then(Value::as_str)?;
            let selected = requested_browsers.iter().any(|requested| {
                name == requested || (requested == "chromium" && name == "chromium-headless-shell")
            }) || name == "ffmpeg";
            selected.then(|| format!("{}-{revision}", name.replace('-', "_")))
        })
        .filter(|entry| browsers_path.join(entry).exists())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if expected.is_empty() {
        return directory_entry_names(browsers_path);
    }
    Ok(expected)
}

fn garbage_collect_browser_runtime_cache(
    runners_root: &Path,
    browsers_path: &Path,
    staging_root: &Path,
    locks_root: &Path,
    active_runtime_ids: &BTreeSet<String>,
) -> Result<(), SetupError> {
    let _lock = BrowserRuntimeLock::acquire(&locks_root.join("gc.lock"))?;
    remove_aged_directories(staging_root, BROWSER_RUNTIME_STAGING_MAX_AGE)?;

    let mut runners = directory_metadata(runners_root)?;
    runners.sort_by(|left, right| right.1.cmp(&left.1));
    for (index, (name, _, path)) in runners.iter().enumerate() {
        let active = active_runtime_ids.contains(name);
        let runtime_lock = locks_root.join(format!("{name}.lock"));
        let locked = runtime_lock.exists() && !lock_is_stale(&runtime_lock);
        let age = path_age(path);
        let expired = age.is_some_and(|age| age > BROWSER_RUNTIME_MAX_AGE);
        let over_capacity = index >= BROWSER_RUNTIME_MAX_RUNNERS
            && age.is_some_and(|age| age > BROWSER_RUNTIME_CAPACITY_MIN_AGE);
        if !active && !locked && (expired || over_capacity) {
            fs::remove_dir_all(path).map_err(|source| SetupError::Io {
                path: path.clone(),
                source,
            })?;
        }
    }

    let referenced_browser_entries = fs::read_dir(runners_root)
        .map_err(|source| SetupError::Io {
            path: runners_root.to_path_buf(),
            source,
        })?
        .filter_map(Result::ok)
        .filter_map(|entry| read_json_value(&entry.path().join("manifest.json")).ok())
        .filter_map(|value| serde_json::from_value::<BrowserRuntimeManifest>(value).ok())
        .flat_map(|manifest| manifest.browser_entries)
        .collect::<BTreeSet<_>>();
    let _browser_lock = BrowserRuntimeLock::acquire(&locks_root.join("browser-cache.lock"))?;
    for entry in fs::read_dir(browsers_path).map_err(|source| SetupError::Io {
        path: browsers_path.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| SetupError::Io {
            path: browsers_path.to_path_buf(),
            source,
        })?;
        let name = entry.file_name().to_string_lossy().to_string();
        if entry.path().is_dir()
            && !name.starts_with('.')
            && !referenced_browser_entries.contains(&name)
        {
            fs::remove_dir_all(entry.path()).map_err(|source| SetupError::Io {
                path: entry.path(),
                source,
            })?;
        }
    }
    Ok(())
}

fn remove_aged_directories(root: &Path, max_age: Duration) -> Result<(), SetupError> {
    for (_, _, path) in directory_metadata(root)? {
        if path_age(&path).is_some_and(|age| age > max_age) {
            fs::remove_dir_all(&path).map_err(|source| SetupError::Io {
                path: path.clone(),
                source,
            })?;
        }
    }
    Ok(())
}

fn directory_metadata(root: &Path) -> Result<Vec<(String, SystemTime, PathBuf)>, SetupError> {
    let mut values = Vec::new();
    for entry in fs::read_dir(root).map_err(|source| SetupError::Io {
        path: root.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| SetupError::Io {
            path: root.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let activity_path = path.join("manifest.json");
        let modified = fs::metadata(if activity_path.is_file() {
            &activity_path
        } else {
            &path
        })
        .and_then(|metadata| metadata.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);
        values.push((
            entry.file_name().to_string_lossy().to_string(),
            modified,
            path,
        ));
    }
    Ok(values)
}

fn path_age(path: &Path) -> Option<Duration> {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
}

struct BrowserRuntimeLock {
    path: PathBuf,
    running: Arc<AtomicBool>,
    heartbeat: Option<thread::JoinHandle<()>>,
}

impl BrowserRuntimeLock {
    fn acquire(path: &Path) -> Result<Self, SetupError> {
        let started = SystemTime::now();
        loop {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
            {
                Ok(mut file) => {
                    writeln!(
                        file,
                        "pid={} preparedAt={}",
                        std::process::id(),
                        now_string()
                    )
                    .map_err(|source| SetupError::Io {
                        path: path.to_path_buf(),
                        source,
                    })?;
                    let running = Arc::new(AtomicBool::new(true));
                    let heartbeat_running = running.clone();
                    let heartbeat_path = path.to_path_buf();
                    let heartbeat = thread::spawn(move || {
                        while heartbeat_running.load(Ordering::Acquire) {
                            thread::park_timeout(Duration::from_secs(3));
                            if !heartbeat_running.load(Ordering::Acquire) {
                                break;
                            }
                            let _ = fs::write(
                                &heartbeat_path,
                                format!(
                                    "pid={} heartbeatAt={}\n",
                                    std::process::id(),
                                    now_string()
                                ),
                            );
                        }
                    });
                    return Ok(Self {
                        path: path.to_path_buf(),
                        running,
                        heartbeat: Some(heartbeat),
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    if lock_is_stale(path) {
                        let _ = fs::remove_file(path);
                        continue;
                    }
                    if started.elapsed().unwrap_or_default() >= PLAYWRIGHT_LOCK_WAIT {
                        return Err(SetupError::InvalidArgument(format!(
                            "timed out waiting for Playwright runtime lock {}",
                            path.display()
                        )));
                    }
                    thread::sleep(PLAYWRIGHT_LOCK_POLL);
                }
                Err(source) => {
                    return Err(SetupError::Io {
                        path: path.to_path_buf(),
                        source,
                    })
                }
            }
        }
    }
}

impl Drop for BrowserRuntimeLock {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);
        if let Some(heartbeat) = self.heartbeat.take() {
            heartbeat.thread().unpark();
            let _ = heartbeat.join();
        }
        let _ = fs::remove_file(&self.path);
    }
}

fn lock_is_stale(path: &Path) -> bool {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .is_some_and(|age| age >= PLAYWRIGHT_LOCK_STALE_AFTER)
}

pub fn install(env: &SetupEnvironment, agents: &[AgentKind]) -> Result<SetupReport, SetupError> {
    let mut report = SetupReport::new("install", agents);
    let manifest = read_manifest(&env.package_root)?;
    validate_package(&env.package_root, &manifest)?;
    verify_checksums(&env.package_root)?;

    let cleanup = cleanup_legacy(env, agents)?;
    report.removed.extend(cleanup.removed.iter().cloned());
    report.blocked.extend(cleanup.blocked.iter().cloned());
    if !cleanup.blocked.is_empty() {
        return Err(SetupError::LegacyCleanupBlocked(cleanup.blocked));
    }

    install_runtime(env, &manifest)?;
    install_setup_shim(env, &manifest)?;
    for agent in agents {
        install_agent_plugin(env, &manifest, *agent)?;
        write_mcp_registration(env, *agent, &manifest)?;
        cleanup_agent_session(env, *agent)?;
    }

    let mut registry = read_registry(env)?;
    registry.schema_version = 1;
    registry.current_version = Some(manifest.version.clone());
    registry.runtime_current = Some(path_string(env.runtime_current()));
    registry.legacy_cleanup = Some(LegacyCleanupRecord {
        ran_at: now_string(),
        agents: agents
            .iter()
            .map(|agent| agent.as_str().to_string())
            .collect(),
        removed: cleanup.removed,
        blocked: cleanup.blocked,
        kept_shared_cli_launcher: cleanup.kept_shared_cli_launcher,
    });
    for agent in agents {
        registry.installed_agents.insert(
            agent.as_str().to_string(),
            InstalledAgent {
                agent: agent.as_str().to_string(),
                plugin_root: path_string(env.agent_plugin_root(*agent)),
                mcp_registration: path_string(effective_mcp_registration_path(env, *agent)),
                installed_at: now_string(),
            },
        );
    }
    write_registry(env, &registry)?;
    report.installed_runtime = Some(path_string(env.runtime_current()));
    report.checks = doctor(env, agents, false)?.checks;
    Ok(report)
}

pub fn uninstall(env: &SetupEnvironment, agents: &[AgentKind]) -> Result<SetupReport, SetupError> {
    let mut report = SetupReport::new("uninstall", agents);
    let mut registry = read_registry(env)?;
    for agent in agents {
        let removed = uninstall_agent(env, *agent)?;
        report.removed.extend(removed);
        registry.installed_agents.remove(agent.as_str());
    }
    write_registry(env, &registry)?;
    Ok(report)
}

pub fn purge(env: &SetupEnvironment) -> Result<SetupReport, SetupError> {
    let agents = AgentKind::all();
    let mut report = uninstall(env, &agents)?;
    report.command = "purge".to_string();
    for path in [
        env.runtime_root(),
        env.bin_dir().join(executable_path("loom-setup")),
        env.install_registry_path(),
        env.loom_home.join("knowledge"),
        env.loom_home.join("agent-sessions"),
        env.loom_home.join("runtime-cache"),
        env.loom_home.join("mcp-registrations"),
    ] {
        if path.exists() {
            remove_path(&path)?;
            report.removed.push(path_string(path));
        }
    }
    Ok(report)
}

pub fn doctor(
    env: &SetupEnvironment,
    agents: &[AgentKind],
    strict: bool,
) -> Result<SetupReport, SetupError> {
    let mut report = SetupReport::new("doctor", agents);
    let current = env.runtime_current();
    let server_binary = current.join(executable_path("bin/loom-mcp-server"));
    report.checks.push(check_path(
        "runtime.current",
        &current,
        "current runtime directory or symlink",
    ));
    report.checks.push(check_path(
        "runtime.mcpServer",
        &server_binary,
        "loom-mcp-server binary",
    ));
    report.checks.push(check_mcp_surface());
    report.checks.push(check_python_worker(env));
    for agent in agents {
        report.checks.push(check_path(
            &format!("{}.plugin", agent.as_str()),
            &env.agent_plugin_root(*agent),
            "agent plugin files",
        ));
        report
            .checks
            .push(check_agent_mcp_registration(env, *agent, &server_binary));
    }
    let failed: Vec<DoctorCheck> = report
        .checks
        .iter()
        .filter(|check| check.status != "passed" && check.status != "skipped")
        .cloned()
        .collect();
    if strict && !failed.is_empty() {
        return Err(SetupError::DoctorFailed(report.checks));
    }
    if !failed.is_empty() {
        report.status = "warning".to_string();
    }
    Ok(report)
}

pub fn write_package_layout(
    output_dir: &Path,
    platform: TargetPlatform,
) -> Result<PathBuf, SetupError> {
    let package_dir = output_dir.join(format!("loom-{}-{}", VERSION, platform.as_str()));
    if package_dir.exists() {
        remove_path(&package_dir)?;
    }
    let repo = repo_root()?;
    let binary_dir = current_binary_dir()?;
    let manifest = ReleaseManifest::for_platform(platform);
    let mcp_server_source = binary_dir.join(executable_path("loom-mcp-server"));
    let setup_source = binary_dir.join(executable_path("loom-setup"));

    copy_required(
        &mcp_server_source,
        &package_dir.join(&manifest.binaries.mcp_server),
    )?;
    copy_required(&setup_source, &package_dir.join(&manifest.binaries.setup))?;
    set_executable(&package_dir.join(&manifest.binaries.mcp_server))?;
    set_executable(&package_dir.join(&manifest.binaries.setup))?;

    copy_required(
        &repo.join("src/python/algorithms"),
        &package_dir.join(&manifest.python.algorithms),
    )?;
    remove_pycache_dirs(&package_dir.join(&manifest.python.algorithms))?;
    fs::create_dir_all(package_dir.join(&manifest.python.runtime)).map_err(|source| {
        SetupError::Io {
            path: package_dir.join(&manifest.python.runtime),
            source,
        }
    })?;
    write_text(
        &package_dir.join(&manifest.python.runtime).join("README"),
        "This local development package uses the host python3 runtime when a bundled Python runtime is not present.\n",
    )?;

    copy_required(
        &repo.join(&manifest.plugins.codex),
        &package_dir.join(&manifest.plugins.codex),
    )?;
    copy_required(
        &repo.join(&manifest.plugins.claude_code),
        &package_dir.join(&manifest.plugins.claude_code),
    )?;
    copy_required(
        &repo.join(&manifest.plugins.opencode),
        &package_dir.join(&manifest.plugins.opencode),
    )?;
    copy_required(
        &repo.join(SHARED_LOOM_REFERENCES),
        &package_dir.join(SHARED_LOOM_REFERENCES),
    )?;
    copy_required(
        &repo.join(SHARED_DEPLOY_REFERENCES),
        &package_dir.join(SHARED_DEPLOY_REFERENCES),
    )?;

    write_json(&package_dir.join("manifest.json"), &manifest)?;
    write_checksums(&package_dir)?;
    validate_package(&package_dir, &manifest)?;
    verify_checksums(&package_dir)?;
    Ok(package_dir)
}

pub fn archive_package_layout(
    package_dir: &Path,
    output_dir: &Path,
    platform: TargetPlatform,
) -> Result<PathBuf, SetupError> {
    let manifest = read_manifest(package_dir)?;
    validate_package(package_dir, &manifest)?;
    verify_checksums(package_dir)?;
    fs::create_dir_all(output_dir).map_err(|source| SetupError::Io {
        path: output_dir.to_path_buf(),
        source,
    })?;
    let archive = output_dir.join(platform.package_file_name(&manifest.version));
    if archive.exists() {
        remove_path(&archive)?;
    }
    if matches!(platform, TargetPlatform::WindowsX64) {
        write_zip_archive(package_dir, &archive)?;
    } else {
        write_tar_gz_archive(package_dir, output_dir, &archive)?;
    }
    write_archive_checksum(&archive)?;
    Ok(archive)
}

pub fn package_file_names(version: &str) -> Vec<String> {
    TargetPlatform::all()
        .iter()
        .map(|platform| platform.package_file_name(version))
        .collect()
}

pub fn release_artifact_file_names(version: &str) -> Vec<String> {
    TargetPlatform::all()
        .iter()
        .flat_map(|platform| {
            [
                platform.package_file_name(version),
                platform.package_checksum_file_name(version),
            ]
        })
        .collect()
}

fn read_manifest(package_root: &Path) -> Result<ReleaseManifest, SetupError> {
    let path = package_root.join("manifest.json");
    let bytes = fs::read(&path).map_err(|source| SetupError::Io {
        path: path.clone(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| SetupError::Json { path, source })
}

fn validate_package(package_root: &Path, manifest: &ReleaseManifest) -> Result<(), SetupError> {
    if manifest.schema_version != PACKAGE_SCHEMA_VERSION {
        return Err(SetupError::InvalidArgument(format!(
            "unsupported package schemaVersion {}",
            manifest.schema_version
        )));
    }
    let required = [
        manifest.binaries.mcp_server.as_str(),
        manifest.binaries.setup.as_str(),
        manifest.python.runtime.as_str(),
        manifest.python.algorithms.as_str(),
        manifest.plugins.codex.as_str(),
        manifest.plugins.claude_code.as_str(),
        manifest.plugins.opencode.as_str(),
        SHARED_LOOM_REFERENCES,
        SHARED_DEPLOY_REFERENCES,
    ];
    for relative in required {
        let path = package_root.join(relative);
        if !path.exists() {
            return Err(SetupError::MissingPackageEntry(path));
        }
    }
    for relative in REQUIRED_SHARED_REFERENCE_FILES {
        let path = package_root.join(relative);
        if !path.is_file() {
            return Err(SetupError::MissingPackageEntry(path));
        }
    }
    audit_package_contents(package_root)?;
    Ok(())
}

fn audit_package_contents(package_root: &Path) -> Result<(), SetupError> {
    let forbidden_prefixes = [
        "src/",
        "tests/",
        "node_modules/",
        "dist/",
        ".git/",
        "scripts/refresh-local-codex-plugin.js",
        "scripts/refresh-local-claude-plugin.js",
        "scripts/refresh-local-opencode-plugin.js",
        "scripts/uninstall-local-adapter.js",
        "scripts/lib/loom-user-install.js",
    ];
    let forbidden_exact = [
        "package-lock.json",
        "tsconfig.json",
        "dist/cli.js",
        "bin/loom-cli",
    ];
    for path in collect_files(package_root)? {
        let relative = path
            .strip_prefix(package_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if forbidden_exact.contains(&relative.as_str())
            || forbidden_prefixes
                .iter()
                .any(|prefix| relative.starts_with(prefix))
        {
            return Err(SetupError::InvalidArgument(format!(
                "release package must not include legacy or source-only entry: {relative}"
            )));
        }
    }
    Ok(())
}

fn verify_checksums(package_root: &Path) -> Result<(), SetupError> {
    let checksum_path = package_root.join("checksums.txt");
    if !checksum_path.exists() {
        return Err(SetupError::MissingPackageEntry(checksum_path));
    }
    let content = fs::read_to_string(&checksum_path).map_err(|source| SetupError::Io {
        path: checksum_path.clone(),
        source,
    })?;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut parts = trimmed.split_whitespace();
        let expected = parts
            .next()
            .ok_or_else(|| SetupError::InvalidArgument("invalid checksums.txt line".into()))?;
        let relative = parts
            .next()
            .ok_or_else(|| SetupError::InvalidArgument("invalid checksums.txt line".into()))?;
        let path = package_root.join(relative);
        let actual = sha256_file(&path)?;
        if expected != actual {
            return Err(SetupError::ChecksumMismatch {
                path,
                expected: expected.to_string(),
                actual,
            });
        }
    }
    Ok(())
}

fn install_runtime(env: &SetupEnvironment, manifest: &ReleaseManifest) -> Result<(), SetupError> {
    let target = env.runtime_root().join(&manifest.version);
    if target.exists() {
        remove_path(&target)?;
    }
    fs::create_dir_all(&target).map_err(|source| SetupError::Io {
        path: target.clone(),
        source,
    })?;
    for entry in ["bin", "python", "plugins", "manifest.json", "checksums.txt"] {
        let source = env.package_root.join(entry);
        let destination = target.join(entry);
        copy_path(&source, &destination)?;
    }
    set_executable(&target.join(&manifest.binaries.mcp_server))?;
    set_executable(&target.join(&manifest.binaries.setup))?;
    replace_current_runtime(&env.runtime_current(), &target)?;
    Ok(())
}

fn install_setup_shim(
    env: &SetupEnvironment,
    manifest: &ReleaseManifest,
) -> Result<(), SetupError> {
    fs::create_dir_all(env.bin_dir()).map_err(|source| SetupError::Io {
        path: env.bin_dir(),
        source,
    })?;
    let source = env.runtime_current().join(&manifest.binaries.setup);
    let target = env.bin_dir().join(executable_path("loom-setup"));
    copy_path(&source, &target)?;
    set_executable(&target)?;
    Ok(())
}

fn replace_current_runtime(current: &Path, target: &Path) -> Result<(), SetupError> {
    if current.exists() || current.symlink_metadata().is_ok() {
        remove_path(current)?;
    }
    if let Some(parent) = current.parent() {
        fs::create_dir_all(parent).map_err(|source| SetupError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, current).map_err(|source| SetupError::Io {
            path: current.to_path_buf(),
            source,
        })?;
        Ok(())
    }
    #[cfg(windows)]
    {
        match std::os::windows::fs::symlink_dir(target, current) {
            Ok(()) => Ok(()),
            Err(_) => copy_dir(target, current),
        }
    }
}

fn install_agent_plugin(
    env: &SetupEnvironment,
    manifest: &ReleaseManifest,
    agent: AgentKind,
) -> Result<(), SetupError> {
    let runtime_template = env.runtime_current().join(manifest.plugin_path(agent));
    match agent {
        AgentKind::Codex => install_codex_plugin(env, &runtime_template),
        AgentKind::ClaudeCode => install_claude_plugin(env, &runtime_template),
        AgentKind::Opencode => install_opencode_plugin(env, &runtime_template),
    }
}

fn install_codex_plugin(env: &SetupEnvironment, template: &Path) -> Result<(), SetupError> {
    let target = env.agent_plugin_root(AgentKind::Codex);
    cleanup_codex_plugin_cache(env)?;
    prepare_generated_target(&target)?;
    copy_dir(template, &target)?;
    install_skill_references(env, &target)?;
    write_install_stamp(&target, AgentKind::Codex)?;
    write_codex_plugin_cache(env, &target)?;
    update_codex_marketplace(env)?;
    Ok(())
}

fn cleanup_codex_plugin_cache(env: &SetupEnvironment) -> Result<(), SetupError> {
    for target in [
        env.codex_home.join("plugins/cache/local/loom"),
        env.codex_home.join("plugins/cache/local-plugins/loom"),
    ] {
        if target.exists() {
            remove_path(&target)?;
        }
    }
    Ok(())
}

fn write_codex_plugin_cache(env: &SetupEnvironment, plugin_root: &Path) -> Result<(), SetupError> {
    let manifest_path = plugin_root.join(".codex-plugin/plugin.json");
    let manifest = read_json_value(&manifest_path)?;
    let version = manifest
        .get("version")
        .and_then(Value::as_str)
        .ok_or_else(|| SetupError::MissingPackageEntry(manifest_path.clone()))?;
    let cache_root = env
        .codex_home
        .join("plugins/cache/local-plugins/loom")
        .join(version);
    copy_dir(plugin_root, &cache_root)
}

fn update_codex_marketplace(env: &SetupEnvironment) -> Result<(), SetupError> {
    let marketplace_path = env.user_home.join(".agents/plugins/marketplace.json");
    let mut value = if marketplace_path.exists() {
        read_json_value(&marketplace_path)?
    } else {
        json!({
            "name": "local-plugins",
            "interface": { "displayName": "Local Plugins" },
            "plugins": []
        })
    };
    value["name"] = json!("local-plugins");
    if !value["plugins"].is_array() {
        value["plugins"] = json!([]);
    }
    let plugins = value["plugins"].as_array_mut().expect("plugins is array");
    plugins.retain(|entry| entry.get("name").and_then(Value::as_str) != Some("loomline"));
    let entry = json!({
        "name": "loom",
        "source": { "source": "local", "path": "./plugins/loom" },
        "policy": { "installation": "AVAILABLE", "authentication": "ON_INSTALL" },
        "category": "Productivity"
    });
    if let Some(existing) = plugins
        .iter_mut()
        .find(|entry| entry.get("name").and_then(Value::as_str) == Some("loom"))
    {
        *existing = entry;
    } else {
        plugins.push(entry);
    }
    write_json(&marketplace_path, &value)
}

fn install_claude_plugin(env: &SetupEnvironment, template: &Path) -> Result<(), SetupError> {
    let target = env.agent_plugin_root(AgentKind::ClaudeCode);
    prepare_generated_target(&target)?;
    copy_dir(template, &target)?;
    install_skill_references(env, &target)?;
    write_install_stamp(&target, AgentKind::ClaudeCode)?;
    let commands_root = env.claude_home.join("commands");
    fs::create_dir_all(&commands_root).map_err(|source| SetupError::Io {
        path: commands_root.clone(),
        source,
    })?;
    for name in ["loom.md", "loom-deploy.md"] {
        let source = target.join("commands").join(name);
        if source.exists() {
            let command_target = commands_root.join(name);
            copy_path(&source, &command_target)?;
            write_file_marker(&command_target, "Loom MCP-only Claude command")?;
        }
    }
    Ok(())
}

fn install_opencode_plugin(env: &SetupEnvironment, template: &Path) -> Result<(), SetupError> {
    let command_root = env.opencode_home.join("commands");
    let plugin_root = env.opencode_home.join("plugins");
    fs::create_dir_all(&command_root).map_err(|source| SetupError::Io {
        path: command_root.clone(),
        source,
    })?;
    fs::create_dir_all(&plugin_root).map_err(|source| SetupError::Io {
        path: plugin_root.clone(),
        source,
    })?;
    for name in ["loom.md", "loom-deploy.md"] {
        let command_target = command_root.join(name);
        copy_path(
            &template.join(".opencode/commands").join(name),
            &command_target,
        )?;
        write_file_marker(&command_target, "Loom MCP-only OpenCode command")?;
    }
    let plugin_target = plugin_root.join("loom.js");
    copy_path(&template.join(".opencode/plugins/loom.js"), &plugin_target)?;
    write_js_file_marker(&plugin_target, "Loom MCP-only OpenCode plugin")?;
    install_standalone_references(
        env,
        SHARED_LOOM_REFERENCES,
        &env.opencode_home.join("references/loom"),
        AgentKind::Opencode,
    )?;
    install_standalone_references(
        env,
        SHARED_DEPLOY_REFERENCES,
        &env.opencode_home.join("references/loom-deploy"),
        AgentKind::Opencode,
    )?;
    write_json(
        &env.opencode_home.join(".loom-opencode-mcp-install.json"),
        &json!({
            "schemaVersion": 1,
            "agent": AgentKind::Opencode.as_str(),
            "installedBy": "loom-setup",
            "protocol": "mcp-only",
            "installedAt": now_string()
        }),
    )?;
    Ok(())
}

fn install_skill_references(env: &SetupEnvironment, plugin_root: &Path) -> Result<(), SetupError> {
    copy_shared_references(
        env,
        SHARED_LOOM_REFERENCES,
        &plugin_root.join("skills/loom/references"),
    )?;
    copy_shared_references(
        env,
        SHARED_DEPLOY_REFERENCES,
        &plugin_root.join("skills/loom-deploy/references"),
    )
}

fn install_standalone_references(
    env: &SetupEnvironment,
    shared_relative: &str,
    target: &Path,
    agent: AgentKind,
) -> Result<(), SetupError> {
    prepare_generated_target(target)?;
    copy_shared_references(env, shared_relative, target)?;
    write_install_stamp(target, agent)
}

fn copy_shared_references(
    env: &SetupEnvironment,
    shared_relative: &str,
    target: &Path,
) -> Result<(), SetupError> {
    let source = env.runtime_current().join(shared_relative);
    copy_dir(&source, target)
}

fn write_mcp_registration(
    env: &SetupEnvironment,
    agent: AgentKind,
    manifest: &ReleaseManifest,
) -> Result<(), SetupError> {
    let command = env.runtime_current().join(&manifest.binaries.mcp_server);
    let registration = json!({
        "name": "loom",
        "transport": "stdio",
        "command": path_string(&command),
        "args": [],
        "env": {
            "LOOM_RUNTIME_HOME": path_string(env.runtime_current()),
            "LOOM_HOME": path_string(&env.loom_home),
            "LOOM_HOST": agent.host_env()
        }
    });
    write_json(&env.common_registration_path(agent), &registration)?;
    match agent {
        AgentKind::Codex => write_codex_mcp_config(env, &command, agent)?,
        AgentKind::ClaudeCode => write_claude_mcp_config(env, &command, agent)?,
        AgentKind::Opencode => write_opencode_mcp_config(env, &command, agent)?,
    }
    Ok(())
}

fn effective_mcp_registration_path(env: &SetupEnvironment, agent: AgentKind) -> PathBuf {
    match agent {
        AgentKind::Codex => env.codex_config_path(),
        AgentKind::ClaudeCode => env.claude_config_path(),
        AgentKind::Opencode => env.opencode_config_path(),
    }
}

fn write_codex_mcp_config(
    env: &SetupEnvironment,
    command: &Path,
    agent: AgentKind,
) -> Result<(), SetupError> {
    let path = env.codex_config_path();
    let mut document = read_toml_document(&path)?;
    let snippet = format!(
        "[mcp_servers.loom]\ncommand = {command}\nargs = []\nstartup_timeout_sec = 30\n\n[mcp_servers.loom.env]\nLOOM_RUNTIME_HOME = {runtime}\nLOOM_HOME = {home}\nLOOM_HOST = {host}\n",
        command = toml_string(&path_string(command)),
        runtime = toml_string(&path_string(env.runtime_current())),
        home = toml_string(&path_string(&env.loom_home)),
        host = toml_string(agent.host_env()),
    );
    let snippet_document = parse_toml_document(&path, &snippet)?;
    document["mcp_servers"]["loom"] = snippet_document["mcp_servers"]["loom"].clone();
    write_toml_document(&path, &document)?;
    remove_generated_codex_mcp_json(env)?;
    Ok(())
}

fn write_claude_mcp_config(
    env: &SetupEnvironment,
    command: &Path,
    agent: AgentKind,
) -> Result<(), SetupError> {
    let path = env.claude_config_path();
    let mut value = read_json_if_exists(&path)?.unwrap_or_else(|| json!({}));
    ensure_object_root(&mut value);
    if !value.get("mcpServers").is_some_and(Value::is_object) {
        value["mcpServers"] = json!({});
    }
    value["mcpServers"]["loom"] = json!({
        "type": "stdio",
        "command": path_string(command),
        "args": [],
        "env": {
            "LOOM_RUNTIME_HOME": path_string(env.runtime_current()),
            "LOOM_HOME": path_string(&env.loom_home),
            "LOOM_HOST": agent.host_env()
        }
    });
    write_json(&path, &value)?;
    remove_generated_agent_mcp_json(env, agent)?;
    Ok(())
}

fn write_opencode_mcp_config(
    env: &SetupEnvironment,
    command: &Path,
    agent: AgentKind,
) -> Result<(), SetupError> {
    let path = env.opencode_config_path();
    let mut value = read_jsonc_if_exists(&path)?.unwrap_or_else(|| {
        json!({
            "$schema": "https://opencode.ai/config.json"
        })
    });
    ensure_object_root(&mut value);
    if !value.get("mcp").is_some_and(Value::is_object) {
        value["mcp"] = json!({});
    }
    value["mcp"]["loom"] = json!({
        "type": "local",
        "enabled": true,
        "command": [path_string(command)],
        "environment": {
            "LOOM_RUNTIME_HOME": path_string(env.runtime_current()),
            "LOOM_HOME": path_string(&env.loom_home),
            "LOOM_HOST": agent.host_env()
        }
    });
    write_json(&path, &value)?;
    remove_generated_agent_mcp_json(env, agent)?;
    Ok(())
}

fn read_toml_document(path: &Path) -> Result<DocumentMut, SetupError> {
    if !path.exists() {
        return Ok(DocumentMut::new());
    }
    let text = fs::read_to_string(path).map_err(|source| SetupError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    parse_toml_document(path, &text)
}

fn parse_toml_document(path: &Path, text: &str) -> Result<DocumentMut, SetupError> {
    text.parse::<DocumentMut>()
        .map_err(|source| SetupError::Toml {
            path: path.to_path_buf(),
            source,
        })
}

fn write_toml_document(path: &Path, document: &DocumentMut) -> Result<(), SetupError> {
    let mut text = document.to_string();
    if !text.ends_with('\n') {
        text.push('\n');
    }
    write_text(path, &text)
}

fn toml_string(value: &str) -> String {
    serde_json::to_string(value).expect("serializing a string cannot fail")
}

fn cleanup_agent_session(env: &SetupEnvironment, agent: AgentKind) -> Result<(), SetupError> {
    let root = env.agent_session_root(agent);
    if root.exists() {
        remove_path(&root)?;
    }
    Ok(())
}

#[derive(Debug, Default)]
struct LegacyCleanupOutcome {
    removed: Vec<String>,
    blocked: Vec<LegacyBlockedPath>,
    kept_shared_cli_launcher: bool,
}

fn cleanup_legacy(
    env: &SetupEnvironment,
    agents: &[AgentKind],
) -> Result<LegacyCleanupOutcome, SetupError> {
    let mut outcome = LegacyCleanupOutcome::default();
    for agent in agents {
        for path in legacy_paths_for_agent(env, *agent) {
            cleanup_legacy_path(&path, &mut outcome)?;
        }
        let adapter = env
            .loom_home
            .join("adapters")
            .join(agent.as_str())
            .join("refresh.json");
        if adapter.exists() {
            remove_path(&adapter)?;
            outcome.removed.push(path_string(adapter));
        }
    }
    let launcher = env.loom_home.join("bin/loom-cli");
    if launcher.exists() {
        if remaining_legacy_adapter_stamps(env, agents)?.is_empty() {
            cleanup_legacy_path(&launcher, &mut outcome)?;
        } else {
            outcome.kept_shared_cli_launcher = true;
        }
    }
    Ok(outcome)
}

fn legacy_paths_for_agent(env: &SetupEnvironment, agent: AgentKind) -> Vec<PathBuf> {
    match agent {
        AgentKind::Codex => vec![
            env.user_home.join("plugins/loom"),
            env.user_home.join("plugins/loomline"),
            env.codex_home.join("plugins/cache/local/loomline"),
            env.codex_home.join("plugins/cache/local-plugins/loomline"),
        ],
        AgentKind::ClaudeCode => vec![
            env.claude_home.join("skills/loom"),
            env.claude_home.join("skills/loomline"),
            env.claude_home.join("commands/loom.md"),
            env.claude_home.join("commands/loom-deploy.md"),
            env.claude_home.join("commands/loomline.md"),
            env.claude_home.join("commands/loomline-deploy.md"),
            env.claude_home.join("plugins/data/loomline-skills-dir"),
        ],
        AgentKind::Opencode => vec![
            env.opencode_home.join("commands/loom.md"),
            env.opencode_home.join("commands/loom-deploy.md"),
            env.opencode_home.join("commands/loomline.md"),
            env.opencode_home.join("commands/loomline-deploy.md"),
            env.opencode_home.join("command/loom.md"),
            env.opencode_home.join("command/loom-deploy.md"),
            env.opencode_home.join("plugins/loom.js"),
            env.opencode_home.join("plugins/loomline.js"),
            env.opencode_home.join("references/loomline"),
            env.opencode_home.join(".loom-opencode-refresh.json"),
            env.opencode_home.join(".loomline-opencode-refresh.json"),
        ],
    }
}

fn cleanup_legacy_path(path: &Path, outcome: &mut LegacyCleanupOutcome) -> Result<(), SetupError> {
    if !path.exists() {
        return Ok(());
    }
    if is_confirmed_loom_generated(path)? {
        remove_path(path)?;
        outcome.removed.push(path_string(path));
        return Ok(());
    }
    outcome.blocked.push(LegacyBlockedPath {
        path: path_string(path),
        reason: "existing path has no Loom stamp or legacy CLI marker".to_string(),
    });
    Ok(())
}

fn remaining_legacy_adapter_stamps(
    env: &SetupEnvironment,
    removing_agents: &[AgentKind],
) -> Result<Vec<PathBuf>, SetupError> {
    let removing: BTreeSet<String> = removing_agents
        .iter()
        .map(|agent| agent.as_str().to_string())
        .collect();
    let adapters_root = env.loom_home.join("adapters");
    if !adapters_root.exists() {
        return Ok(Vec::new());
    }
    let mut remaining = Vec::new();
    let entries = fs::read_dir(&adapters_root).map_err(|source| SetupError::Io {
        path: adapters_root.clone(),
        source,
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| SetupError::Io {
            path: adapters_root.clone(),
            source,
        })?;
        let name = entry.file_name().to_string_lossy().to_string();
        if removing.contains(&name) {
            continue;
        }
        let stamp = entry.path().join("refresh.json");
        if stamp.exists() {
            remaining.push(stamp);
        }
    }
    Ok(remaining)
}

fn is_confirmed_loom_generated(path: &Path) -> Result<bool, SetupError> {
    if path.is_dir() {
        for stamp in [
            INSTALL_STAMP,
            ".loom-codex-install-source.json",
            ".loom-claude-refresh.json",
            ".loom-opencode-refresh.json",
            ".loomline-opencode-refresh.json",
            "refresh.json",
        ] {
            if path.join(stamp).exists() {
                return Ok(true);
            }
        }
        return directory_contains_marker(path);
    }
    file_contains_marker(path)
}

fn directory_contains_marker(path: &Path) -> Result<bool, SetupError> {
    let entries = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(error) => {
            return Err(SetupError::Io {
                path: path.to_path_buf(),
                source: error,
            })
        }
    };
    for entry in entries.take(40) {
        let entry = entry.map_err(|source| SetupError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let child = entry.path();
        if child.is_dir() {
            if directory_contains_marker(&child)? {
                return Ok(true);
            }
        } else if child.is_file() && file_contains_marker(&child)? {
            return Ok(true);
        }
    }
    Ok(false)
}

fn file_contains_marker(path: &Path) -> Result<bool, SetupError> {
    let Ok(content) = fs::read_to_string(path) else {
        return Ok(false);
    };
    Ok(LEGACY_MARKERS.iter().any(|marker| content.contains(marker))
        || content.contains("Loom MCP-only"))
}

fn prepare_generated_target(target: &Path) -> Result<(), SetupError> {
    if target.exists() {
        if !is_confirmed_loom_generated(target)? {
            return Err(SetupError::LegacyCleanupBlocked(vec![LegacyBlockedPath {
                path: path_string(target),
                reason: "target exists and is not confirmed as Loom-generated".to_string(),
            }]));
        }
        remove_path(target)?;
    }
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(|source| SetupError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    Ok(())
}

fn uninstall_agent(env: &SetupEnvironment, agent: AgentKind) -> Result<Vec<String>, SetupError> {
    let mut removed = Vec::new();
    let plugin_root = env.agent_plugin_root(agent);
    if plugin_root.exists() && is_confirmed_loom_generated(&plugin_root)? {
        remove_path(&plugin_root)?;
        removed.push(path_string(plugin_root));
    }
    if matches!(agent, AgentKind::Codex) {
        if remove_codex_marketplace_entry(env)? {
            removed.push(path_string(
                env.user_home.join(".agents/plugins/marketplace.json"),
            ));
        }
        if remove_codex_mcp_config(env)? {
            removed.push(path_string(env.codex_config_path()));
        }
        if remove_generated_codex_mcp_json(env)? {
            removed.push(path_string(env.agent_mcp_registration_path(agent)));
        }
    }
    if matches!(agent, AgentKind::ClaudeCode) && remove_claude_mcp_config(env)? {
        removed.push(path_string(env.claude_config_path()));
    }
    if matches!(agent, AgentKind::Opencode) && remove_opencode_mcp_config(env)? {
        removed.push(path_string(env.opencode_config_path()));
    }
    for path in uninstall_files_for_agent(env, agent) {
        if path.exists() && is_confirmed_loom_generated(&path)? {
            remove_path(&path)?;
            removed.push(path_string(path));
        }
    }
    let mut cleanup_paths = vec![
        env.common_registration_path(agent),
        env.agent_session_root(agent),
    ];
    if !matches!(agent, AgentKind::Codex) {
        cleanup_paths.push(env.agent_mcp_registration_path(agent));
    }
    for path in cleanup_paths {
        if path.exists() {
            remove_path(&path)?;
            removed.push(path_string(path));
        }
    }
    Ok(removed)
}

fn uninstall_files_for_agent(env: &SetupEnvironment, agent: AgentKind) -> Vec<PathBuf> {
    match agent {
        AgentKind::Codex => vec![],
        AgentKind::ClaudeCode => vec![
            env.claude_home.join("commands/loom.md"),
            env.claude_home.join("commands/loom-deploy.md"),
        ],
        AgentKind::Opencode => vec![
            env.opencode_home.join("commands/loom.md"),
            env.opencode_home.join("commands/loom-deploy.md"),
            env.opencode_home.join("plugins/loom.js"),
            env.opencode_home.join("references/loom"),
            env.opencode_home.join("references/loom-deploy"),
            env.opencode_home.join(".loom-opencode-mcp-install.json"),
        ],
    }
}

fn remove_codex_marketplace_entry(env: &SetupEnvironment) -> Result<bool, SetupError> {
    let path = env.user_home.join(".agents/plugins/marketplace.json");
    if !path.exists() {
        return Ok(false);
    }
    let mut value = read_json_value(&path)?;
    let Some(plugins) = value.get_mut("plugins").and_then(Value::as_array_mut) else {
        return Ok(false);
    };
    let before = plugins.len();
    plugins.retain(|entry| entry.get("name").and_then(Value::as_str) != Some("loom"));
    if plugins.len() == before {
        return Ok(false);
    }
    write_json(&path, &value)?;
    Ok(true)
}

fn remove_codex_mcp_config(env: &SetupEnvironment) -> Result<bool, SetupError> {
    let path = env.codex_config_path();
    if !path.exists() {
        return Ok(false);
    }
    let mut document = read_toml_document(&path)?;
    let removed = document
        .get_mut("mcp_servers")
        .and_then(Item::as_table_like_mut)
        .and_then(|servers| servers.remove("loom"))
        .is_some();
    if removed {
        write_toml_document(&path, &document)?;
    }
    Ok(removed)
}

fn remove_claude_mcp_config(env: &SetupEnvironment) -> Result<bool, SetupError> {
    let path = env.claude_config_path();
    if !path.exists() {
        return Ok(false);
    }
    let mut value = read_json_value(&path)?;
    let removed = value
        .get_mut("mcpServers")
        .and_then(Value::as_object_mut)
        .and_then(|servers| servers.remove("loom"))
        .is_some();
    if removed {
        write_json(&path, &value)?;
    }
    Ok(removed)
}

fn remove_opencode_mcp_config(env: &SetupEnvironment) -> Result<bool, SetupError> {
    let path = env.opencode_config_path();
    if !path.exists() {
        return Ok(false);
    }
    let mut value = read_jsonc_if_exists(&path)?.unwrap_or_else(|| json!({}));
    let removed = value
        .get_mut("mcp")
        .and_then(Value::as_object_mut)
        .and_then(|servers| servers.remove("loom"))
        .is_some();
    if removed {
        write_json(&path, &value)?;
    }
    Ok(removed)
}

fn remove_generated_codex_mcp_json(env: &SetupEnvironment) -> Result<bool, SetupError> {
    remove_generated_agent_mcp_json(env, AgentKind::Codex)
}

fn remove_generated_agent_mcp_json(
    env: &SetupEnvironment,
    agent: AgentKind,
) -> Result<bool, SetupError> {
    let path = env.agent_mcp_registration_path(agent);
    if !path.exists() || !is_loom_mcp_registration_json(&path)? {
        return Ok(false);
    }
    remove_path(&path)?;
    Ok(true)
}

fn is_loom_mcp_registration_json(path: &Path) -> Result<bool, SetupError> {
    let bytes = fs::read(path).map_err(|source| SetupError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
        return Ok(false);
    };
    let is_loom = value.get("name").and_then(Value::as_str) == Some("loom");
    let command_matches = value
        .get("command")
        .and_then(Value::as_str)
        .is_some_and(|command| command.contains("loom-mcp-server"));
    Ok(is_loom && command_matches)
}

fn read_registry(env: &SetupEnvironment) -> Result<InstallRegistry, SetupError> {
    let path = env.install_registry_path();
    if !path.exists() {
        return Ok(InstallRegistry::default());
    }
    let bytes = fs::read(&path).map_err(|source| SetupError::Io {
        path: path.clone(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| SetupError::Json { path, source })
}

fn write_registry(env: &SetupEnvironment, registry: &InstallRegistry) -> Result<(), SetupError> {
    write_json(&env.install_registry_path(), registry)
}

fn check_mcp_surface() -> DoctorCheck {
    let server = mcp_server::LoomMcpServer::default();
    let tools: BTreeSet<String> = server
        .tool_registry()
        .list_tools()
        .into_iter()
        .map(|tool| tool.name.to_string())
        .collect();
    let required = [
        "plan",
        "continue",
        "readFieldGroup",
        "knowledgeAdd",
        "knowledgeBuild",
        "knowledgeBrainstormContext",
        "knowledgeDisable",
        "knowledgeDiscard",
        "knowledgeEnable",
        "knowledgeInspectChunk",
        "knowledgeList",
        "knowledgePending",
        "knowledgeRemove",
        "knowledgeResume",
        "knowledgeSearch",
        "knowledgeSemanticSubmitFile",
        "knowledgeStatus",
        "knowledgeUpdate",
        "deployRun",
        "deployRepair",
    ];
    let missing: Vec<&str> = required
        .iter()
        .copied()
        .filter(|name| !tools.contains(*name))
        .collect();
    if missing.is_empty() {
        DoctorCheck {
            name: "mcp.toolsResources".to_string(),
            status: "passed".to_string(),
            detail: "required MCP tools are registered".to_string(),
        }
    } else {
        DoctorCheck {
            name: "mcp.toolsResources".to_string(),
            status: "failed".to_string(),
            detail: format!("missing tools: {}", missing.join(", ")),
        }
    }
}

fn check_python_worker(env: &SetupEnvironment) -> DoctorCheck {
    let current = env.runtime_current();
    let algorithms = current.join("python/algorithms/worker.py");
    let python = current.join("python/runtime/bin/python");
    if !algorithms.exists() {
        return DoctorCheck {
            name: "python.worker".to_string(),
            status: "failed".to_string(),
            detail: format!("missing worker {}", algorithms.display()),
        };
    }
    if !python.exists() {
        return DoctorCheck {
            name: "python.worker".to_string(),
            status: "skipped".to_string(),
            detail: "bundled python executable is not present in this package".to_string(),
        };
    }
    let smoke = json!({
        "operation": "tokenize",
        "text": "Loom MCP doctor smoke"
    });
    let output = Command::new(&python)
        .arg(&algorithms)
        .env("PYTHONPATH", current.join("python"))
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            if let Some(stdin) = child.stdin.as_mut() {
                stdin.write_all(smoke.to_string().as_bytes())?;
                stdin.write_all(b"\n")?;
            }
            child.wait_with_output()
        });
    match output {
        Ok(output) if output.status.success() => DoctorCheck {
            name: "python.worker".to_string(),
            status: "passed".to_string(),
            detail: "tokenization smoke passed".to_string(),
        },
        Ok(output) => DoctorCheck {
            name: "python.worker".to_string(),
            status: "failed".to_string(),
            detail: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        },
        Err(error) => DoctorCheck {
            name: "python.worker".to_string(),
            status: "failed".to_string(),
            detail: error.to_string(),
        },
    }
}

fn check_agent_mcp_registration(
    env: &SetupEnvironment,
    agent: AgentKind,
    server_binary: &Path,
) -> DoctorCheck {
    match agent {
        AgentKind::Codex => check_codex_mcp_config(env, server_binary),
        AgentKind::ClaudeCode => check_claude_mcp_config(env, server_binary),
        AgentKind::Opencode => check_opencode_mcp_config(env, server_binary),
    }
}

fn check_codex_mcp_config(env: &SetupEnvironment, server_binary: &Path) -> DoctorCheck {
    let path = env.codex_config_path();
    let name = "codex.mcpRegistration".to_string();
    let document = match read_toml_document(&path) {
        Ok(document) => document,
        Err(error) => {
            return DoctorCheck {
                name,
                status: "failed".to_string(),
                detail: error.to_string(),
            }
        }
    };
    let command = document["mcp_servers"]["loom"]["command"].as_str();
    let host = document["mcp_servers"]["loom"]["env"]["LOOM_HOST"].as_str();
    if command == Some(path_string(server_binary).as_str()) && host == Some("codex") {
        DoctorCheck {
            name,
            status: "passed".to_string(),
            detail: format!(
                "Codex config contains [mcp_servers.loom]: {}",
                path.display()
            ),
        }
    } else {
        DoctorCheck {
            name,
            status: "failed".to_string(),
            detail: format!("missing or stale [mcp_servers.loom] in {}", path.display()),
        }
    }
}

fn check_claude_mcp_config(env: &SetupEnvironment, server_binary: &Path) -> DoctorCheck {
    let path = env.claude_config_path();
    let name = "claude-code.mcpRegistration".to_string();
    let value = match read_json_value(&path) {
        Ok(value) => value,
        Err(error) => {
            return DoctorCheck {
                name,
                status: "failed".to_string(),
                detail: error.to_string(),
            }
        }
    };
    let loom = &value["mcpServers"]["loom"];
    let command = loom["command"].as_str();
    let host = loom["env"]["LOOM_HOST"].as_str();
    let server_matches = command == Some(path_string(server_binary).as_str());
    if loom["type"].as_str() == Some("stdio") && server_matches && host == Some("claude-code") {
        DoctorCheck {
            name,
            status: "passed".to_string(),
            detail: format!("Claude config contains mcpServers.loom: {}", path.display()),
        }
    } else {
        DoctorCheck {
            name,
            status: "failed".to_string(),
            detail: format!("missing or stale mcpServers.loom in {}", path.display()),
        }
    }
}

fn check_opencode_mcp_config(env: &SetupEnvironment, server_binary: &Path) -> DoctorCheck {
    let path = env.opencode_config_path();
    let name = "opencode.mcpRegistration".to_string();
    let value = match read_jsonc_if_exists(&path) {
        Ok(Some(value)) => value,
        Ok(None) => {
            return DoctorCheck {
                name,
                status: "failed".to_string(),
                detail: format!("missing OpenCode config: {}", path.display()),
            }
        }
        Err(error) => {
            return DoctorCheck {
                name,
                status: "failed".to_string(),
                detail: error.to_string(),
            }
        }
    };
    let loom = &value["mcp"]["loom"];
    let command = loom["command"]
        .as_array()
        .and_then(|items| items.first())
        .and_then(Value::as_str);
    let host = loom["environment"]["LOOM_HOST"].as_str();
    let server_matches = command == Some(path_string(server_binary).as_str());
    if loom["type"].as_str() == Some("local")
        && loom["enabled"].as_bool() == Some(true)
        && server_matches
        && host == Some("opencode")
    {
        DoctorCheck {
            name,
            status: "passed".to_string(),
            detail: format!("OpenCode config contains mcp.loom: {}", path.display()),
        }
    } else {
        DoctorCheck {
            name,
            status: "failed".to_string(),
            detail: format!("missing or stale mcp.loom in {}", path.display()),
        }
    }
}

fn check_path(name: &str, path: &Path, detail: &str) -> DoctorCheck {
    if path.exists() || path.symlink_metadata().is_ok() {
        DoctorCheck {
            name: name.to_string(),
            status: "passed".to_string(),
            detail: format!("{detail}: {}", path.display()),
        }
    } else {
        DoctorCheck {
            name: name.to_string(),
            status: "failed".to_string(),
            detail: format!("missing {detail}: {}", path.display()),
        }
    }
}

fn write_checksums(root: &Path) -> Result<(), SetupError> {
    let mut lines = Vec::new();
    for path in collect_files(root)? {
        if path.file_name().and_then(|name| name.to_str()) == Some("checksums.txt") {
            continue;
        }
        let relative = path.strip_prefix(root).unwrap_or(&path);
        let hash = sha256_file(&path)?;
        lines.push(format!("{hash}  {}", relative.to_string_lossy()));
    }
    lines.sort();
    write_text(
        &root.join("checksums.txt"),
        &format!("{}\n", lines.join("\n")),
    )
}

fn write_archive_checksum(archive: &Path) -> Result<(), SetupError> {
    let hash = sha256_file(archive)?;
    let file_name = archive
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            SetupError::InvalidArgument(format!(
                "archive path has no valid file name: {}",
                archive.display()
            ))
        })?;
    let checksum_path = archive.with_file_name(format!("{file_name}.sha256"));
    write_text(&checksum_path, &format!("{hash}  {file_name}\n"))
}

fn write_zip_archive(package_dir: &Path, archive: &Path) -> Result<(), SetupError> {
    let file = fs::File::create(archive).map_err(|source| SetupError::Io {
        path: archive.to_path_buf(),
        source,
    })?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);
    let root_name = package_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| SetupError::InvalidArgument("package directory must have a name".into()))?;
    zip_dir_inner(package_dir, package_dir, root_name, &mut zip, options)?;
    zip.finish().map_err(|error| {
        SetupError::InvalidArgument(format!("failed to finish zip archive: {error}"))
    })?;
    Ok(())
}

fn zip_dir_inner(
    base: &Path,
    current: &Path,
    root_name: &str,
    zip: &mut ZipWriter<fs::File>,
    options: SimpleFileOptions,
) -> Result<(), SetupError> {
    let mut entries: Vec<PathBuf> = fs::read_dir(current)
        .map_err(|source| SetupError::Io {
            path: current.to_path_buf(),
            source,
        })?
        .map(|entry| {
            entry
                .map(|entry| entry.path())
                .map_err(|source| SetupError::Io {
                    path: current.to_path_buf(),
                    source,
                })
        })
        .collect::<Result<_, _>>()?;
    entries.sort();
    for path in entries {
        let relative = path.strip_prefix(base).unwrap_or(&path);
        let archive_name = format!(
            "{root_name}/{}",
            relative.to_string_lossy().replace('\\', "/")
        );
        if path.is_dir() {
            zip.add_directory(format!("{archive_name}/"), options)
                .map_err(|error| {
                    SetupError::InvalidArgument(format!("failed to add zip directory: {error}"))
                })?;
            zip_dir_inner(base, &path, root_name, zip, options)?;
        } else if path.is_file() {
            zip.start_file(archive_name, options).map_err(|error| {
                SetupError::InvalidArgument(format!("failed to add zip file: {error}"))
            })?;
            let bytes = fs::read(&path).map_err(|source| SetupError::Io {
                path: path.clone(),
                source,
            })?;
            zip.write_all(&bytes).map_err(|source| SetupError::Io {
                path: path.clone(),
                source,
            })?;
        }
    }
    Ok(())
}

fn write_tar_gz_archive(
    package_dir: &Path,
    output_dir: &Path,
    archive: &Path,
) -> Result<(), SetupError> {
    let package_name = package_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| SetupError::InvalidArgument("package directory must have a name".into()))?;
    let parent = package_dir.parent().ok_or_else(|| {
        SetupError::InvalidArgument("package directory must have a parent".into())
    })?;
    let status = Command::new("tar")
        .arg("-czf")
        .arg(archive)
        .arg("-C")
        .arg(parent)
        .arg(package_name)
        .status()
        .map_err(|source| SetupError::Io {
            path: output_dir.to_path_buf(),
            source,
        })?;
    if !status.success() {
        return Err(SetupError::InvalidArgument(format!(
            "tar failed with status {status}"
        )));
    }
    Ok(())
}

fn collect_files(root: &Path) -> Result<Vec<PathBuf>, SetupError> {
    let mut files = Vec::new();
    collect_files_inner(root, &mut files)?;
    Ok(files)
}

fn collect_files_inner(root: &Path, files: &mut Vec<PathBuf>) -> Result<(), SetupError> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root).map_err(|source| SetupError::Io {
        path: root.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| SetupError::Io {
            path: root.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            collect_files_inner(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, SetupError> {
    let bytes = fs::read(path).map_err(|source| SetupError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

fn copy_path(source: &Path, target: &Path) -> Result<(), SetupError> {
    if source.is_dir() {
        copy_dir(source, target)
    } else {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|source| SetupError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::copy(source, target).map_err(|source_error| SetupError::Io {
            path: target.to_path_buf(),
            source: source_error,
        })?;
        Ok(())
    }
}

fn copy_required(source: &Path, target: &Path) -> Result<(), SetupError> {
    if !source.exists() {
        return Err(SetupError::MissingPackageEntry(source.to_path_buf()));
    }
    copy_path(source, target)
}

fn copy_dir(source: &Path, target: &Path) -> Result<(), SetupError> {
    fs::create_dir_all(target).map_err(|source_error| SetupError::Io {
        path: target.to_path_buf(),
        source: source_error,
    })?;
    for entry in fs::read_dir(source).map_err(|source_error| SetupError::Io {
        path: source.to_path_buf(),
        source: source_error,
    })? {
        let entry = entry.map_err(|source_error| SetupError::Io {
            path: source.to_path_buf(),
            source: source_error,
        })?;
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_dir(&source_path, &target_path)?;
        } else if source_path.is_file() {
            copy_path(&source_path, &target_path)?;
        }
    }
    Ok(())
}

fn remove_path(path: &Path) -> Result<(), SetupError> {
    if path.is_dir() && !is_symlink(path) {
        fs::remove_dir_all(path).map_err(|source| SetupError::Io {
            path: path.to_path_buf(),
            source,
        })
    } else {
        fs::remove_file(path).map_err(|source| SetupError::Io {
            path: path.to_path_buf(),
            source,
        })
    }
}

fn remove_pycache_dirs(root: &Path) -> Result<(), SetupError> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root).map_err(|source| SetupError::Io {
        path: root.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| SetupError::Io {
            path: root.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|name| name.to_str()) == Some("__pycache__") {
                remove_path(&path)?;
            } else {
                remove_pycache_dirs(&path)?;
            }
        }
    }
    Ok(())
}

fn is_symlink(path: &Path) -> bool {
    path.symlink_metadata()
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false)
}

fn set_executable(path: &Path) -> Result<(), SetupError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if path.exists() {
            let mut permissions = fs::metadata(path)
                .map_err(|source| SetupError::Io {
                    path: path.to_path_buf(),
                    source,
                })?
                .permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(path, permissions).map_err(|source| SetupError::Io {
                path: path.to_path_buf(),
                source,
            })?;
        }
    }
    Ok(())
}

fn write_install_stamp(path: &Path, agent: AgentKind) -> Result<(), SetupError> {
    write_json(
        &path.join(INSTALL_STAMP),
        &json!({
            "schemaVersion": 1,
            "agent": agent.as_str(),
            "installedBy": "loom-setup",
            "protocol": "mcp-only",
            "installedAt": now_string()
        }),
    )
}

fn write_file_marker(path: &Path, marker: &str) -> Result<(), SetupError> {
    let mut text = fs::read_to_string(path).map_err(|source| SetupError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if !text.contains(marker) {
        text.push_str(&format!("\n<!-- {marker} -->\n"));
        write_text(path, &text)?;
    }
    Ok(())
}

fn write_js_file_marker(path: &Path, marker: &str) -> Result<(), SetupError> {
    let mut text = fs::read_to_string(path).map_err(|source| SetupError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if !text.contains(marker) {
        text.push_str(&format!("\n// {marker}\n"));
        write_text(path, &text)?;
    }
    Ok(())
}

fn read_json_value(path: &Path) -> Result<Value, SetupError> {
    let bytes = fs::read(path).map_err(|source| SetupError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| SetupError::Json {
        path: path.to_path_buf(),
        source,
    })
}

fn read_json_if_exists(path: &Path) -> Result<Option<Value>, SetupError> {
    if path.exists() {
        read_json_value(path).map(Some)
    } else {
        Ok(None)
    }
}

fn read_jsonc_if_exists(path: &Path) -> Result<Option<Value>, SetupError> {
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(path).map_err(|source| SetupError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    match serde_json::from_str(&text) {
        Ok(value) => Ok(Some(value)),
        Err(_) => {
            let stripped = strip_jsonc_trailing_commas(&strip_jsonc_comments(&text));
            serde_json::from_str(&stripped)
                .map(Some)
                .map_err(|source| SetupError::Json {
                    path: path.to_path_buf(),
                    source,
                })
        }
    }
}

fn ensure_object_root(value: &mut Value) {
    if !value.is_object() {
        *value = json!({});
    }
}

fn strip_jsonc_comments(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        if ch == '"' {
            in_string = true;
            output.push(ch);
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'/') {
            chars.next();
            for next in chars.by_ref() {
                if next == '\n' {
                    output.push('\n');
                    break;
                }
            }
            continue;
        }
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut previous = '\0';
            for next in chars.by_ref() {
                if next == '\n' {
                    output.push('\n');
                }
                if previous == '*' && next == '/' {
                    break;
                }
                previous = next;
            }
            continue;
        }
        output.push(ch);
    }
    output
}

fn strip_jsonc_trailing_commas(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut index = 0;
    let mut in_string = false;
    let mut escaped = false;
    while index < chars.len() {
        let ch = chars[index];
        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            index += 1;
            continue;
        }
        if ch == '"' {
            in_string = true;
            output.push(ch);
            index += 1;
            continue;
        }
        if ch == ',' {
            let mut lookahead = index + 1;
            while lookahead < chars.len() && chars[lookahead].is_whitespace() {
                lookahead += 1;
            }
            if lookahead < chars.len() && matches!(chars[lookahead], '}' | ']') {
                index += 1;
                continue;
            }
        }
        output.push(ch);
        index += 1;
    }
    output
}

fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<(), SetupError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|source| SetupError::Json {
        path: path.to_path_buf(),
        source,
    })?;
    write_text(path, &format!("{}\n", String::from_utf8_lossy(&bytes)))
}

fn write_text(path: &Path, text: &str) -> Result<(), SetupError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| SetupError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, text).map_err(|source| SetupError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn path_string(path: impl AsRef<Path>) -> String {
    path.as_ref().to_string_lossy().into_owned()
}

fn now_string() -> String {
    chrono::Utc::now().to_rfc3339()
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn browser_runtime_lock_heartbeats_and_stops_on_drop() {
        let root = std::env::temp_dir().join(format!(
            "loom-playwright-lock-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_millis()
        ));
        fs::create_dir_all(&root).unwrap();
        let lock_path = root.join("runtime.lock");
        let lock = BrowserRuntimeLock::acquire(&lock_path).unwrap();
        let initial = fs::metadata(&lock_path).unwrap().modified().unwrap();
        thread::sleep(Duration::from_millis(3300));
        let refreshed = fs::metadata(&lock_path).unwrap().modified().unwrap();
        assert!(refreshed > initial);
        drop(lock);
        assert!(!lock_path.exists());
        thread::sleep(Duration::from_millis(100));
        assert!(!lock_path.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
