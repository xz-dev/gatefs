use sandboxfs::path::SandboxPath;
use sandboxfs::state::{MetadataOperation, Sandbox};
use tempfile::TempDir;

#[test]
fn hidden_paths_cannot_receive_metadata_overrides() {
    let temp = TempDir::new().unwrap();
    let mut sandbox = Sandbox::new("demo_fs_errors", temp.path().join("demo.log"));
    sandbox.add_layer(temp.path(), SandboxPath::new("/data").unwrap());
    sandbox.add_hide(SandboxPath::new("/data").unwrap());

    let error = sandbox
        .apply_metadata_override(&MetadataOperation::Chmod {
            path: SandboxPath::new("/data").unwrap(),
            mode: 0o777,
        })
        .unwrap_err()
        .to_string();
    assert!(error.contains("path not found or not real"));
}

#[test]
fn missing_paths_cannot_receive_metadata_overrides() {
    let temp = TempDir::new().unwrap();
    let mut sandbox = Sandbox::new("demo_missing", temp.path().join("demo.log"));

    let error = sandbox
        .apply_metadata_override(&MetadataOperation::Chown {
            path: SandboxPath::new("/missing").unwrap(),
            uid: Some(1000),
            gid: None,
        })
        .unwrap_err()
        .to_string();
    assert!(error.contains("path not found or not real"));
}

#[test]
fn virtual_parent_directories_are_not_real_mount_targets() {
    let temp = TempDir::new().unwrap();
    let mut sandbox = Sandbox::new("demo_virtual", temp.path().join("demo.log"));
    sandbox.add_layer(temp.path(), SandboxPath::new("/a/b/c").unwrap());

    assert!(sandbox.resolve(&SandboxPath::new("/a").unwrap()).is_some());
    assert!(sandbox.children(&SandboxPath::new("/z").unwrap()).is_err());
}
