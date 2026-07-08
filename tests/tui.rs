mod common;

use assert_cmd::cargo::CommandCargoExt;
use assert_cmd::prelude::*;
use gatefs::path::SandboxPath;
use gatefs::state::{
    MetadataOperation, PendingMetadataRequest, PendingReadWriteRequest, PendingRequest,
    ReadWriteOperation,
};
use gatefs::tui::PendingAction;
use predicates::prelude::*;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tempfile::TempDir;

use common::RunningSession;

fn buffer_lines(buffer: &ratatui::buffer::Buffer) -> Vec<String> {
    let area = *buffer.area();
    let mut lines = Vec::new();
    for y in area.y..area.y + area.height {
        let mut line = String::new();
        for x in area.x..area.x + area.width {
            line.push_str(buffer[(x, y)].symbol());
        }
        lines.push(line);
    }
    lines
}

#[test]
fn tui_renders_pending_request_and_controls() {
    let pending = vec![PendingRequest::Metadata(PendingMetadataRequest {
        id: 42,
        sandbox: "demo_tui".to_string(),
        attach_id: Some(7),
        operation: MetadataOperation::Chmod {
            path: SandboxPath::new("/data/file").unwrap(),
            mode: 0o444,
        },
        object: gatefs::state::MetadataObjectKey {
            layer_id: 1,
            relative_path: std::path::PathBuf::from("file"),
        },
        kinds: vec![gatefs::state::PendingOperationKind::Mode],
        pid: 123,
        uid: 1000,
        gid: 1000,
        description: "path=/data/file SETATTR mode=0444".to_string(),
    })];
    let backend = TestBackend::new(80, 12);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| gatefs::tui::draw_pending(frame, &pending, 0, "ok"))
        .unwrap();
    let lines = buffer_lines(terminal.backend().buffer());
    assert!(lines[0].contains("Operation"));
    assert!(lines[1].contains("id=42 attach=7"));
    assert!(lines[2].contains("path=/data/file"));
    assert!(lines[3].contains("path=/data/file SETATTR mode=0444"));
    let rendered = lines.join("\n");
    assert!(rendered.contains("a=allow d=deny n=do-nothing e=edit q=quit ok"));
}

#[test]
fn tui_renders_read_write_request_without_edit_control() {
    let pending = vec![PendingRequest::ReadWrite(
        PendingReadWriteRequest::new_with_attach_path(
            43,
            "demo_tui".to_string(),
            Some(8),
            ReadWriteOperation::ReadFile {
                path: SandboxPath::new("/secret/file").unwrap(),
            },
            SandboxPath::new("/secret/file").unwrap(),
            gatefs::state::RequesterIdentity {
                pid: 321,
                uid: 1001,
                gid: 1002,
            },
        ),
    )];
    let backend = TestBackend::new(100, 12);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| gatefs::tui::draw_pending(frame, &pending, 0, "ok"))
        .unwrap();
    let rendered = buffer_lines(terminal.backend().buffer()).join("\n");
    assert!(rendered.contains("id=43 attach=8"));
    assert!(rendered.contains("kind=READ"));
    assert!(rendered.contains("path=/secret/file"));
    assert!(rendered.contains("pid=321 uid=1001 gid=1002"));
    assert!(rendered.contains("path=/secret/file READ file"));
    assert!(rendered.contains("a=allow d=deny n=do-nothing q=quit ok"));
    assert!(!rendered.contains("e=edit"));
}

#[test]
fn edit_pending_command_uses_configured_binary_and_releases_original_request() {
    let session = RunningSession::start("demo_tui_edit");
    let gatefs_bin = std::process::Command::cargo_bin("gatefs").unwrap();
    let gatefs_bin = gatefs_bin.get_program().to_owned();

    session
        .sandbox_cmd()
        .args(["mount", session.temp.path().to_str().unwrap(), "/data"])
        .assert()
        .success();

    let runtime = gatefs::runtime::RuntimePaths::for_tests_with_log_dir(
        session.runtime(),
        session.log_dir(),
        None,
    );
    let message = gatefs::tui::edit_pending_command_with_options(
        &session.name,
        9999,
        "chmod 444 /data",
        Some(gatefs_bin),
        Some(&runtime),
    )
    .unwrap();
    assert!(message.contains("original request was not released"));

    session
        .sandbox_cmd()
        .arg("metadata")
        .assert()
        .success()
        .stdout(predicate::str::contains("/data"));
}

#[test]
fn pending_actions_report_session_errors() {
    let temp = TempDir::new().unwrap();
    let runtime = gatefs::runtime::RuntimePaths::for_tests_with_log_dir(
        temp.path().join("run"),
        temp.path().join("logs"),
        None,
    );
    let error = gatefs::tui::resolve_pending_action(
        &runtime,
        "missing_tui_action",
        1,
        PendingAction::Allow,
    )
    .unwrap_err()
    .to_string();
    assert!(error.contains("gatefs run missing_tui_action"));
}

#[test]
fn access_tui_reports_missing_foreground_session() {
    let temp = TempDir::new().unwrap();
    std::process::Command::cargo_bin("gatefs-access-tui")
        .unwrap()
        .env("GATEFS_RUNTIME_DIR", temp.path().join("run"))
        .env("GATEFS_LOG_DIR", temp.path().join("logs"))
        .env_remove("GATEFS_SOCKET")
        .arg("missing_tui")
        .assert()
        .failure()
        .stderr(predicate::str::contains("gatefs run missing_tui"));
}
