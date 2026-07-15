use setup::{
    archive_package_layout, doctor, install, parse_agent_selection, prepare_browser_runtime, purge,
    release_artifact_file_names, uninstall, write_package_layout, BrowserRuntimePrepareOptions,
    SetupEnvironment, SetupError, TargetPlatform, VERSION,
};
use std::path::PathBuf;

fn main() {
    match run() {
        Ok(value) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&value).expect("report serializes")
            );
        }
        Err(error) => {
            let status = match &error {
                SetupError::LegacyCleanupBlocked(_) => "legacy_cleanup_blocked",
                SetupError::DoctorFailed(_) => "doctor_failed",
                _ => "failed",
            };
            let details = match &error {
                SetupError::LegacyCleanupBlocked(blocked) => {
                    serde_json::json!({ "blocked": blocked })
                }
                SetupError::DoctorFailed(checks) => serde_json::json!({ "checks": checks }),
                _ => serde_json::json!({}),
            };
            eprintln!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "status": status,
                    "error": error.to_string(),
                    "details": details
                }))
                .expect("error serializes")
            );
            std::process::exit(1);
        }
    }
}

fn run() -> Result<serde_json::Value, SetupError> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first().map(String::as_str) else {
        return Ok(serde_json::json!({
            "status": "ok",
            "version": VERSION,
            "usage": usage()
        }));
    };
    match command {
        "--version" | "version" => Ok(serde_json::json!({
            "status": "ok",
            "version": VERSION
        })),
        "install" => {
            let options = CliOptions::parse(&args[1..])?;
            let agents = parse_agent_selection(
                options
                    .agent
                    .as_deref()
                    .ok_or_else(|| SetupError::InvalidArgument("--agent is required".into()))?,
            )?;
            let env = SetupEnvironment::from_env(options.package_root)?;
            serde_json::to_value(install(&env, &agents)?).map_err(|source| SetupError::Json {
                path: PathBuf::from("<install-report>"),
                source,
            })
        }
        "doctor" => {
            let options = CliOptions::parse(&args[1..])?;
            let agents = match options.agent.as_deref() {
                Some(agent) => parse_agent_selection(agent)?,
                None => setup::AgentKind::all().to_vec(),
            };
            let env = SetupEnvironment::from_env(options.package_root)?;
            serde_json::to_value(doctor(&env, &agents, true)?).map_err(|source| SetupError::Json {
                path: PathBuf::from("<doctor-report>"),
                source,
            })
        }
        "uninstall" => {
            let options = CliOptions::parse(&args[1..])?;
            let agents = if options.all {
                setup::AgentKind::all().to_vec()
            } else {
                parse_agent_selection(options.agent.as_deref().ok_or_else(|| {
                    SetupError::InvalidArgument("--agent or --all is required".into())
                })?)?
            };
            let env = SetupEnvironment::from_env(options.package_root)?;
            serde_json::to_value(uninstall(&env, &agents)?).map_err(|source| SetupError::Json {
                path: PathBuf::from("<uninstall-report>"),
                source,
            })
        }
        "purge" => {
            let options = CliOptions::parse(&args[1..])?;
            let env = SetupEnvironment::from_env(options.package_root)?;
            serde_json::to_value(purge(&env)?).map_err(|source| SetupError::Json {
                path: PathBuf::from("<purge-report>"),
                source,
            })
        }
        "browser-runtime" => {
            if args.get(1).map(String::as_str) != Some("prepare") {
                return Err(SetupError::InvalidArgument(
                    "browser-runtime requires the prepare subcommand".into(),
                ));
            }
            let options = CliOptions::parse(&args[2..])?;
            let env = SetupEnvironment::from_env(options.package_root)?;
            serde_json::to_value(prepare_browser_runtime(
                &env,
                &BrowserRuntimePrepareOptions {
                    requested_versions: options.playwright_versions,
                    requested_browsers: options.playwright_browsers,
                    npm_program: None,
                    node_program: None,
                    container_program: None,
                },
            )?)
            .map_err(|source| SetupError::Json {
                path: PathBuf::from("<browser-runtime-report>"),
                source,
            })
        }
        "package-layout" => {
            let options = CliOptions::parse(&args[1..])?;
            let output_dir = options.output_dir.ok_or_else(|| {
                SetupError::InvalidArgument("--output-dir is required for package-layout".into())
            })?;
            let platforms = match options.platform.as_deref() {
                Some("all") | None => TargetPlatform::all().to_vec(),
                Some(value) => vec![TargetPlatform::parse(value)?],
            };
            let mut packages = Vec::new();
            for platform in platforms {
                packages.push(
                    write_package_layout(&output_dir, platform)?
                        .display()
                        .to_string(),
                );
            }
            Ok(serde_json::json!({
                "status": "ok",
                "version": VERSION,
                "packages": packages,
                "expectedReleaseArtifacts": release_artifact_file_names(VERSION)
            }))
        }
        "package-archive" => {
            let options = CliOptions::parse(&args[1..])?;
            let output_dir = options.output_dir.ok_or_else(|| {
                SetupError::InvalidArgument("--output-dir is required for package-archive".into())
            })?;
            let package_root = options.package_root.ok_or_else(|| {
                SetupError::InvalidArgument("--package-root is required for package-archive".into())
            })?;
            let platform =
                TargetPlatform::parse(options.platform.as_deref().ok_or_else(|| {
                    SetupError::InvalidArgument("--platform is required for package-archive".into())
                })?)?;
            let archive = archive_package_layout(&package_root, &output_dir, platform)?;
            Ok(serde_json::json!({
                "status": "ok",
                "version": VERSION,
                "archive": archive.display().to_string()
            }))
        }
        other => Err(SetupError::InvalidArgument(format!(
            "unknown command '{other}'\n{}",
            usage()
        ))),
    }
}

#[derive(Debug, Default)]
struct CliOptions {
    agent: Option<String>,
    package_root: Option<PathBuf>,
    output_dir: Option<PathBuf>,
    platform: Option<String>,
    all: bool,
    playwright_versions: Vec<String>,
    playwright_browsers: Vec<String>,
}

impl CliOptions {
    fn parse(args: &[String]) -> Result<Self, SetupError> {
        let mut options = CliOptions::default();
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--agent" => {
                    index += 1;
                    options.agent = Some(required_value(args, index, "--agent")?.to_string());
                }
                "--package-root" => {
                    index += 1;
                    options.package_root = Some(PathBuf::from(required_value(
                        args,
                        index,
                        "--package-root",
                    )?));
                }
                "--output-dir" => {
                    index += 1;
                    options.output_dir =
                        Some(PathBuf::from(required_value(args, index, "--output-dir")?));
                }
                "--platform" => {
                    index += 1;
                    options.platform = Some(required_value(args, index, "--platform")?.to_string());
                }
                "--all" => options.all = true,
                "--playwright-version" => {
                    index += 1;
                    options
                        .playwright_versions
                        .push(required_value(args, index, "--playwright-version")?.to_string());
                }
                "--browser" => {
                    index += 1;
                    options
                        .playwright_browsers
                        .push(required_value(args, index, "--browser")?.to_string());
                }
                unknown => {
                    return Err(SetupError::InvalidArgument(format!(
                        "unknown option '{unknown}'"
                    )))
                }
            }
            index += 1;
        }
        Ok(options)
    }
}

fn required_value<'a>(
    args: &'a [String],
    index: usize,
    option: &str,
) -> Result<&'a str, SetupError> {
    args.get(index)
        .map(String::as_str)
        .ok_or_else(|| SetupError::InvalidArgument(format!("{option} requires a value")))
}

fn usage() -> &'static str {
    "loom-setup install --agent codex|claude-code|opencode|all\n\
     loom-setup doctor [--agent codex|claude-code|opencode|all]\n\
     loom-setup uninstall --agent codex|claude-code|opencode|all\n\
     loom-setup uninstall --all\n\
     loom-setup purge\n\
     loom-setup browser-runtime prepare [--playwright-version <registry-version-or-range>] [--browser chromium|firefox|webkit]\n\
     loom-setup package-layout --output-dir <dir> [--platform all|darwin-arm64|darwin-x64|linux-x64|linux-arm64|windows-x64]\n\
     loom-setup package-archive --package-root <dir> --output-dir <dir> --platform <platform>"
}
