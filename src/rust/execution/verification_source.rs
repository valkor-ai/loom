use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use contracts::{TaskResult, TaskResultStatus};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use state::{paths::from_project_relative, store::StateError};

// Kept beside the accepted result, never in the agent-authored result schema.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct SourceSnapshot {
    schema_version: u32,
    task_result_id: String,
    roots: BTreeSet<String>,
    files: BTreeMap<String, String>,
}

const CONFIG_FILES: &[&str] = &[
    "pyproject.toml",
    "setup.cfg",
    "setup.py",
    "tox.ini",
    "pytest.ini",
    "requirements.txt",
    "uv.lock",
    "poetry.lock",
    "Pipfile",
    "Pipfile.lock",
    "package.json",
    "package-lock.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "tsconfig.json",
    "Cargo.toml",
    "Cargo.lock",
    "go.mod",
    "go.sum",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "settings.gradle",
    "Gemfile",
    "Gemfile.lock",
    "composer.json",
    "composer.lock",
    "global.json",
    "NuGet.Config",
    "project.godot",
    "CMakeLists.txt",
    "Makefile",
];

fn excluded(path: &str) -> bool {
    crate::task_result::is_generated_output_path(path)
        || path.split('/').any(|part| {
            matches!(
                part,
                ".git" | ".loom" | ".venv" | "venv" | "__pycache__" | ".mypy_cache" | ".ruff_cache"
            )
        })
}

fn roots_for(result: &TaskResult) -> BTreeSet<String> {
    let mut roots = BTreeSet::new();
    let refs = result.changed_files.iter().chain(
        result
            .verification_results
            .iter()
            .filter_map(|v| v.provenance.as_ref())
            .flat_map(|p| p.changed_files.iter().chain(p.test_case_refs.iter())),
    );
    for reference in refs {
        let path = reference.split("::").next().unwrap_or(reference);
        if excluded(path) || path.is_empty() {
            continue;
        }
        roots.insert(path.to_string());
        // Watch local additions/deletions, without expanding a root-level file
        // into an unbounded scan of the whole checkout.
        if let Some(parent) = Path::new(path)
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
        {
            roots.insert(parent.to_string_lossy().replace('\\', "/"));
        }
    }
    if !roots.is_empty() {
        roots.extend(CONFIG_FILES.iter().map(|p| p.to_string()));
    }
    roots
}

fn sidecar(result_file: &Path) -> PathBuf {
    result_file.with_extension("source-snapshot.json")
}

fn visit(
    root: &Path,
    relative: &str,
    files: &mut BTreeMap<String, String>,
) -> Result<(), StateError> {
    if excluded(relative) {
        return Ok(());
    }
    let path = from_project_relative(root, relative)?;
    let metadata = match fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            files.insert(relative.to_string(), "missing".to_string());
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() {
        // Do not follow directory links into caches or outside the project.
        let destination = fs::canonicalize(&path)?;
        if !destination.starts_with(fs::canonicalize(root)?) || destination.is_dir() {
            return Err(StateError::InvalidArgument(format!(
                "Verification source cannot follow symlink: {relative}"
            )));
        }
        files.insert(
            relative.to_string(),
            format!(
                "link:{:?}:{:x}",
                fs::read_link(&path)?,
                Sha256::digest(fs::read(destination)?)
            ),
        );
    } else if metadata.is_dir() {
        files.insert(relative.to_string(), "directory".to_string());
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let child = format!("{relative}/{}", entry.file_name().to_string_lossy());
            visit(root, &child, files)?;
        }
    } else if metadata.is_file() {
        files.insert(
            relative.to_string(),
            format!("{:x}", Sha256::digest(fs::read(path)?)),
        );
    } else {
        return Err(StateError::InvalidArgument(format!(
            "Verification source is not a regular file: {relative}"
        )));
    }
    Ok(())
}

fn scan(root: &Path, roots: &BTreeSet<String>) -> Result<BTreeMap<String, String>, StateError> {
    let mut files = BTreeMap::new();
    for relative in roots {
        visit(root, relative, &mut files)?;
    }
    Ok(files)
}

pub(crate) fn capture(
    root: &Path,
    result_file: &Path,
    result: &TaskResult,
) -> Result<(), StateError> {
    if !matches!(
        result.status,
        TaskResultStatus::Completed | TaskResultStatus::CompletedWithNotes
    ) {
        return Ok(());
    }
    let roots = roots_for(result);
    let snapshot = SourceSnapshot {
        schema_version: 1,
        task_result_id: result.task_result_id.clone(),
        files: scan(root, &roots)?,
        roots,
    };
    state::store::write_json_atomic(&sidecar(result_file), &snapshot)
}

pub(crate) fn changes(
    root: &Path,
    result_file: &Path,
    result: &TaskResult,
) -> Result<Vec<String>, StateError> {
    if !matches!(
        result.status,
        TaskResultStatus::Completed | TaskResultStatus::CompletedWithNotes
    ) {
        return Ok(vec![]);
    }
    let path = sidecar(result_file);
    if !path.exists() {
        return Ok(if roots_for(result).is_empty() {
            vec![]
        } else {
            vec!["source snapshot unavailable; verification must be refreshed".to_string()]
        });
    }
    let before: SourceSnapshot = state::store::read_json(&path)?;
    if before.schema_version != 1 || before.task_result_id != result.task_result_id {
        return Err(StateError::StateCorrupted(
            "Verification source snapshot identity mismatch".to_string(),
        ));
    }
    let now = scan(root, &before.roots)?;
    Ok(before
        .files
        .keys()
        .chain(now.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|path| before.files.get(*path) != now.get(*path))
        .cloned()
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "loom-source-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir_all(root.join("src")).unwrap();
            fs::create_dir_all(root.join("tests")).unwrap();
            fs::write(root.join("src/main.rs"), "source").unwrap();
            fs::write(root.join("tests/check.rs"), "test").unwrap();
            Self(root)
        }
        fn result(&self) -> TaskResult {
            serde_json::from_value(json!({
                "schemaVersion":"1.0", "taskResultId":"result", "taskId":"task", "taskPlanId":"plan",
                "status":"completed", "changedFiles":["src/main.rs", "tests/check.rs"],
                "executionContinuity":{"taskResultSubmittedAfterVerification":true,"agentOwnedLongRunningWork":"none","notes":[]},
                "createdAt":"1", "updatedAt":"1"
            })).unwrap()
        }
        fn file(&self) -> PathBuf {
            self.0.join(".loom/results/result.json")
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn detects_source_test_config_additions_and_deletions_without_git() {
        let f = Fixture::new();
        let r = f.result();
        capture(&f.0, &f.file(), &r).unwrap();
        assert!(changes(&f.0, &f.file(), &r).unwrap().is_empty());
        fs::write(f.0.join("src/main.rs"), "changed").unwrap();
        fs::remove_file(f.0.join("tests/check.rs")).unwrap();
        fs::write(f.0.join("src/new.rs"), "new").unwrap();
        fs::write(f.0.join("Cargo.toml"), "config").unwrap();
        let changed = changes(&f.0, &f.file(), &r).unwrap();
        for path in ["src/main.rs", "tests/check.rs", "src/new.rs", "Cargo.toml"] {
            assert!(changed.contains(&path.to_string()), "{changed:?}");
        }
        capture(&f.0, &f.file(), &r).unwrap();
        assert!(changes(&f.0, &f.file(), &r).unwrap().is_empty());
    }

    #[test]
    fn ignores_control_files_caches_and_unrelated_directories() {
        let f = Fixture::new();
        let r = f.result();
        capture(&f.0, &f.file(), &r).unwrap();
        for path in [
            ".loom/state.json",
            "src/__pycache__/cached.pyc",
            "src/target/debug/app",
            "docs/notes.md",
        ] {
            let file = f.0.join(path);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, "generated or unrelated").unwrap();
        }
        assert!(changes(&f.0, &f.file(), &r).unwrap().is_empty());
    }

    #[test]
    fn legacy_results_require_refresh_only_when_they_have_source_inputs() {
        let f = Fixture::new();
        let mut r = f.result();
        assert!(!changes(&f.0, &f.file(), &r).unwrap().is_empty());
        r.changed_files.clear();
        assert!(changes(&f.0, &f.file(), &r).unwrap().is_empty());
        r.status = TaskResultStatus::Blocked;
        r.changed_files.push("src/main.rs".to_string());
        assert!(changes(&f.0, &f.file(), &r).unwrap().is_empty());
    }
}
