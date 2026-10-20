use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{BufReader, Read},
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
    let mut config_dirs = BTreeSet::new();
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
        for ancestor in Path::new(path).ancestors().skip(1) {
            config_dirs.insert(ancestor.to_path_buf());
        }
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
        for directory in config_dirs {
            roots.extend(
                CONFIG_FILES
                    .iter()
                    .map(|file| directory.join(file).to_string_lossy().replace('\\', "/")),
            );
        }
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
    ancestors: &mut BTreeSet<PathBuf>,
) -> Result<(), StateError> {
    if excluded(relative) || files.contains_key(relative) {
        return Ok(());
    }
    let path = from_project_relative(root, relative)?;
    let metadata = match fs::symlink_metadata(&path) {
        Ok(meta) => meta,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            files.insert(relative.to_string(), "missing".to_string());
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    let link = if metadata.file_type().is_symlink() {
        format!("link:{:?}:", fs::read_link(&path)?)
    } else {
        String::new()
    };
    // Canonicalize every input, including children reached through a linked
    // parent, so a scoped scan cannot read files outside the checkout.
    let destination = match fs::canonicalize(&path) {
        Ok(destination) => destination,
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
            ) =>
        {
            files.insert(relative.to_string(), format!("{link}missing"));
            return Ok(());
        }
        Err(error) => return Err(error.into()),
    };
    if !destination.starts_with(root) {
        return Err(StateError::InvalidArgument(format!(
            "Verification source resolves outside project root: {relative}"
        )));
    }
    let resolved_relative = destination
        .strip_prefix(root)
        .expect("destination is inside project root")
        .to_string_lossy()
        .replace('\\', "/");
    if excluded(&resolved_relative) {
        // Keep the link identity so retargeting it to real source invalidates
        // evidence, without tracking mutable control state or generated output.
        files.insert(relative.to_string(), format!("{link}excluded"));
        return Ok(());
    }
    let metadata = fs::metadata(&destination)?;
    if metadata.is_dir() {
        files.insert(relative.to_string(), format!("{link}directory"));
        // A directory link can point back to a parent. Record the link but do
        // not recurse into that directory again along the same traversal.
        if !ancestors.insert(destination.clone()) {
            return Ok(());
        }
        for entry in fs::read_dir(&destination)? {
            let entry = entry?;
            let child = format!("{relative}/{}", entry.file_name().to_string_lossy());
            visit(root, &child, files, ancestors)?;
        }
        ancestors.remove(&destination);
    } else if metadata.is_file() {
        files.insert(
            relative.to_string(),
            format!("{link}{}", file_digest(&destination)?),
        );
    } else {
        return Err(StateError::InvalidArgument(format!(
            "Verification source is not a regular file: {relative}"
        )));
    }
    Ok(())
}

fn file_digest(path: &Path) -> Result<String, StateError> {
    let mut reader = BufReader::new(fs::File::open(path)?);
    let mut hash = Sha256::new();
    let mut buffer = [0; 16384];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn scan(root: &Path, roots: &BTreeSet<String>) -> Result<BTreeMap<String, String>, StateError> {
    let root = fs::canonicalize(root)?;
    let mut files = BTreeMap::new();
    let mut ancestors = BTreeSet::new();
    for relative in roots {
        visit(&root, relative, &mut files, &mut ancestors)?;
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
    // Existing records keep their original scope. An upgrade may add required
    // inputs; never silently reuse evidence that did not snapshot those inputs.
    if !roots_for(result).is_subset(&before.roots) {
        return Ok(vec![
            "source snapshot scope expanded; verification must be refreshed".to_string(),
        ]);
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
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_ID: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "loom-source-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                FIXTURE_ID.fetch_add(1, Ordering::Relaxed)
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
        let initial_changes = changes(&f.0, &f.file(), &r).unwrap();
        assert!(initial_changes.is_empty(), "{initial_changes:?}");
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

    #[test]
    fn tracks_nested_project_configs_without_scanning_other_packages() {
        let f = Fixture::new();
        fs::create_dir_all(f.0.join("packages/api/src")).unwrap();
        fs::create_dir_all(f.0.join("packages/web")).unwrap();
        fs::write(f.0.join("packages/api/src/main.ts"), "source").unwrap();
        let mut r = f.result();
        r.changed_files = vec!["packages/api/src/main.ts".to_string()];
        capture(&f.0, &f.file(), &r).unwrap();
        fs::write(f.0.join("packages/web/package.json"), "unrelated").unwrap();
        assert!(changes(&f.0, &f.file(), &r).unwrap().is_empty());
        fs::write(f.0.join("packages/api/package.json"), "config").unwrap();
        fs::write(f.0.join("packages/tsconfig.json"), "shared config").unwrap();
        let changed = changes(&f.0, &f.file(), &r).unwrap();
        assert_eq!(
            changed,
            vec!["packages/api/package.json", "packages/tsconfig.json"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn follows_internal_directory_links_and_terminates_parent_cycles() {
        use std::os::unix::fs::symlink;
        let f = Fixture::new();
        fs::create_dir_all(f.0.join("shared")).unwrap();
        fs::write(f.0.join("shared/dependency.rs"), "dependency").unwrap();
        symlink("../shared", f.0.join("src/shared")).unwrap();
        symlink(".", f.0.join("src/back")).unwrap();
        let r = f.result();
        capture(&f.0, &f.file(), &r).unwrap();
        assert!(changes(&f.0, &f.file(), &r).unwrap().is_empty());
        fs::write(f.0.join("shared/dependency.rs"), "changed dependency").unwrap();
        assert_eq!(
            changes(&f.0, &f.file(), &r).unwrap(),
            vec!["src/shared/dependency.rs"]
        );
        capture(&f.0, &f.file(), &r).unwrap();
        fs::write(f.0.join("shared/new.rs"), "new dependency").unwrap();
        assert_eq!(
            changes(&f.0, &f.file(), &r).unwrap(),
            vec!["src/shared/new.rs"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn tracks_link_retargeting_and_broken_link_recovery() {
        use std::os::unix::fs::symlink;
        let f = Fixture::new();
        fs::write(f.0.join("one.rs"), "same content").unwrap();
        fs::write(f.0.join("two.rs"), "same content").unwrap();
        symlink("../one.rs", f.0.join("src/linked.rs")).unwrap();
        symlink("../missing.rs", f.0.join("src/broken.rs")).unwrap();
        let r = f.result();
        capture(&f.0, &f.file(), &r).unwrap();
        fs::remove_file(f.0.join("src/linked.rs")).unwrap();
        symlink("../two.rs", f.0.join("src/linked.rs")).unwrap();
        fs::write(f.0.join("missing.rs"), "restored target").unwrap();
        assert_eq!(
            changes(&f.0, &f.file(), &r).unwrap(),
            vec!["src/broken.rs", "src/linked.rs"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_external_sources_even_through_a_linked_parent() {
        use std::os::unix::fs::symlink;
        let f = Fixture::new();
        let external = Fixture::new();
        symlink(&external.0, f.0.join("linked")).unwrap();
        let mut r = f.result();
        r.changed_files = vec!["linked/src/main.rs".to_string()];
        assert!(capture(&f.0, &f.file(), &r)
            .unwrap_err()
            .to_string()
            .contains("outside project root"));
        assert!(!f.file().with_extension("source-snapshot.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn linked_control_and_generated_files_do_not_invalidate_source_evidence() {
        use std::os::unix::fs::symlink;
        let f = Fixture::new();
        for directory in [".loom", ".cache", "target"] {
            fs::create_dir_all(f.0.join(directory)).unwrap();
            fs::write(f.0.join(directory).join("state.json"), "initial state").unwrap();
        }
        symlink("../.loom", f.0.join("src/control")).unwrap();
        symlink("../.cache", f.0.join("src/runtime")).unwrap();
        symlink("../target", f.0.join("src/output")).unwrap();
        symlink("../.loom/state.json", f.0.join("src/state.json")).unwrap();
        let r = f.result();
        capture(&f.0, &f.file(), &r).unwrap();
        let changed = changes(&f.0, &f.file(), &r).unwrap();
        assert!(
            changed.is_empty(),
            "snapshot must not invalidate itself: {changed:?}"
        );
        for directory in [".loom", ".cache", "target"] {
            fs::write(f.0.join(directory).join("state.json"), "updated state").unwrap();
            fs::write(f.0.join(directory).join("new.json"), "new output").unwrap();
        }
        assert!(changes(&f.0, &f.file(), &r).unwrap().is_empty());
        fs::write(f.0.join("src/main.rs"), "changed source").unwrap();
        assert_eq!(changes(&f.0, &f.file(), &r).unwrap(), vec!["src/main.rs"]);
    }

    #[cfg(unix)]
    #[test]
    fn retargeting_an_excluded_link_to_source_requires_reverification() {
        use std::os::unix::fs::symlink;
        let f = Fixture::new();
        fs::create_dir_all(f.0.join(".cache")).unwrap();
        fs::create_dir_all(f.0.join("shared")).unwrap();
        fs::write(f.0.join("shared/module.rs"), "source").unwrap();
        symlink("../.cache", f.0.join("src/dependency")).unwrap();
        let r = f.result();
        capture(&f.0, &f.file(), &r).unwrap();
        fs::remove_file(f.0.join("src/dependency")).unwrap();
        symlink("../shared", f.0.join("src/dependency")).unwrap();
        let changed = changes(&f.0, &f.file(), &r).unwrap();
        assert!(changed.contains(&"src/dependency".to_string()));
        assert!(changed.contains(&"src/dependency/module.rs".to_string()));
        capture(&f.0, &f.file(), &r).unwrap();
        fs::write(f.0.join("shared/module.rs"), "changed dependency").unwrap();
        assert_eq!(
            changes(&f.0, &f.file(), &r).unwrap(),
            vec!["src/dependency/module.rs"]
        );
    }

    #[test]
    fn shared_source_changes_still_invalidate_both_tasks() {
        let f = Fixture::new();
        let first = f.result();
        let mut second = f.result();
        second.task_id = "second-task".to_string();
        second.task_result_id = "second-result".to_string();
        let second_file = f.0.join(".loom/results/second-result.json");
        capture(&f.0, &f.file(), &first).unwrap();
        capture(&f.0, &second_file, &second).unwrap();
        fs::write(f.0.join("src/main.rs"), "shared source changed").unwrap();
        assert_eq!(
            changes(&f.0, &f.file(), &first).unwrap(),
            vec!["src/main.rs"]
        );
        assert_eq!(
            changes(&f.0, &second_file, &second).unwrap(),
            vec!["src/main.rs"]
        );
        capture(&f.0, &f.file(), &first).unwrap();
        assert!(changes(&f.0, &f.file(), &first).unwrap().is_empty());
        assert!(!changes(&f.0, &second_file, &second).unwrap().is_empty());
    }

    #[test]
    fn replacing_a_source_directory_with_a_file_requires_reverification() {
        let f = Fixture::new();
        let r = f.result();
        capture(&f.0, &f.file(), &r).unwrap();
        fs::remove_dir_all(f.0.join("src")).unwrap();
        fs::write(f.0.join("src"), "directory replaced by file").unwrap();
        let changed = changes(&f.0, &f.file(), &r).expect("route changed inputs to repair");
        assert!(changed.contains(&"src".to_string()));
        assert!(changed.contains(&"src/main.rs".to_string()));
    }

    #[test]
    fn restoring_an_older_snapshot_requires_refresh_for_expanded_config_scope() {
        let f = Fixture::new();
        fs::create_dir_all(f.0.join("packages/api/src")).unwrap();
        fs::write(f.0.join("packages/api/src/main.ts"), "source").unwrap();
        let mut r = f.result();
        r.changed_files = vec!["packages/api/src/main.ts".to_string()];
        let roots = BTreeSet::from([
            "packages/api/src".to_string(),
            "packages/api/src/main.ts".to_string(),
            "package.json".to_string(),
        ]);
        let old_snapshot = SourceSnapshot {
            schema_version: 1,
            task_result_id: r.task_result_id.clone(),
            files: scan(&f.0, &roots).unwrap(),
            roots,
        };
        state::store::write_json_atomic(&sidecar(&f.file()), &old_snapshot).unwrap();
        fs::write(f.0.join("packages/api/package.json"), "new config").unwrap();
        let changed = changes(&f.0, &f.file(), &r).unwrap();
        assert!(
            !changed.is_empty(),
            "older scope must not silently approve new config"
        );
        capture(&f.0, &f.file(), &r).unwrap();
        assert!(changes(&f.0, &f.file(), &r).unwrap().is_empty());
    }
}
