use std::{fs, path::PathBuf};

#[test]
fn stage_crates_do_not_depend_on_later_delivery_stages() {
    let root = repo_root();

    assert_crate_omits_dependency(&root, "brainstorm", "planning");
    assert_crate_omits_dependency(&root, "brainstorm", "architecture");
    assert_crate_omits_dependency(&root, "brainstorm", "execution");
    assert_crate_omits_dependency(&root, "planning", "architecture");
    assert_crate_omits_dependency(&root, "planning", "execution");
    assert_crate_omits_dependency(&root, "architecture", "execution");
}

#[test]
fn workflow_owns_cross_stage_delivery_routing() {
    let root = repo_root();
    let workflow_root = root.join("src/rust/workflow");
    let cargo_toml = fs::read_to_string(workflow_root.join("Cargo.toml")).unwrap();
    let lib_rs = fs::read_to_string(workflow_root.join("lib.rs")).unwrap();

    for dependency in ["brainstorm", "planning", "architecture", "execution"] {
        assert!(
            cargo_toml.contains(&format!("{dependency} =")),
            "workflow must depend on {dependency} to own cross-stage routing"
        );
    }
    assert!(
        lib_rs.contains("WorkflowDomainDispatcher"),
        "workflow must expose the root delivery dispatcher"
    );
    assert!(
        lib_rs.contains("RouteActionKind::PlanningContractCreate")
            && lib_rs.contains("RouteActionKind::TaskplanGeneration"),
        "workflow must route both planning and execution handoffs"
    );
}

fn assert_crate_omits_dependency(root: &PathBuf, crate_name: &str, forbidden: &str) {
    let crate_root = root.join("src/rust").join(crate_name);
    let cargo_toml = fs::read_to_string(crate_root.join("Cargo.toml")).unwrap();
    let source = read_rust_sources(&crate_root);

    assert!(
        !cargo_toml.contains(&format!("{forbidden} =")),
        "{crate_name} must not depend on later stage {forbidden}"
    );
    assert!(
        !source.contains(&format!("{forbidden}::")),
        "{crate_name} must not call later stage {forbidden} directly"
    );
}

fn read_rust_sources(root: &PathBuf) -> String {
    let mut out = String::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            out.push_str(&read_rust_sources(&path));
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            out.push_str(&fs::read_to_string(path).unwrap());
            out.push('\n');
        }
    }
    out
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
