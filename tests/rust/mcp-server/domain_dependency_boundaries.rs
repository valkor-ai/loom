use std::{fs, path::PathBuf};

#[test]
fn architecture_does_not_depend_on_execution() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let architecture_root = repo_root.join("src/rust/architecture");
    let cargo_toml = fs::read_to_string(architecture_root.join("Cargo.toml")).unwrap();
    let lib_rs = fs::read_to_string(architecture_root.join("lib.rs")).unwrap();

    assert!(
        !cargo_toml.contains("execution ="),
        "architecture is an upstream domain and must not depend on execution"
    );
    assert!(
        !lib_rs.contains("execution::"),
        "architecture routing must not call execution directly"
    );
}
