use machiavelli_core::{ContinentiaMeta, ContinentiaNode, NodeId, PhaseVector};
use std::process::Command;

fn cli(dir: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_machiavelli"))
        .arg(dir)
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn cli_persists_and_simulates_without_mutating_state() {
    let temp = tempfile::tempdir().unwrap();
    let dir = temp.path().join("db");
    assert!(cli(&dir, &["init"]).status.success());
    let node = ContinentiaNode::new(
        ContinentiaMeta::new(NodeId::derive("cli", "one"), 1),
        PhaseVector::with_constitution(vec![0.3], vec![0.5], vec![7.0]).unwrap(),
    );
    let path = temp.path().join("node.json");
    std::fs::write(&path, serde_json::to_vec(&node).unwrap()).unwrap();
    assert!(cli(&dir, &["put", path.to_str().unwrap()]).status.success());
    let id = node.meta.id.to_string();
    let original = cli(&dir, &["get", &id]);
    assert!(original.status.success());
    let sim = cli(&dir, &["simulate", &id, "10000", "0.001"]);
    assert!(
        sim.status.success(),
        "{}",
        String::from_utf8_lossy(&sim.stderr)
    );
    let response: serde_json::Value = serde_json::from_slice(&sim.stdout).unwrap();
    assert_eq!(response["persisted"], false);
    assert_eq!(response["phase"]["c"][0], 7.0);
    assert_eq!(cli(&dir, &["get", &id]).stdout, original.stdout);
    assert!(
        !cli(&dir, &["simulate", &id, "1000001", "0.001"])
            .status
            .success()
    );
    assert!(!cli(&dir, &["delete", &id]).status.success());
}
#[test]
fn cli_errors_return_nonzero_exit_code() {
    let temp = tempfile::tempdir().unwrap();
    assert!(!cli(temp.path(), &["bogus"]).status.success());
    assert!(!cli(temp.path(), &["get", "invalid"]).status.success());
}

#[test]
fn cli_solver_emits_measured_best_response_gap() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("game.json");
    let matrix = machiavelli_core::PayoffMatrix::zero_sum(2, 2, vec![3.0, 0.0, 0.0, 1.0]).unwrap();
    std::fs::write(&path, serde_json::to_vec(&matrix).unwrap()).unwrap();
    let result = cli(
        temp.path(),
        &["solve", path.to_str().unwrap(), "50000", "1000"],
    );
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let response: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert!(response["exploitability"].as_f64().unwrap() < 0.02);
    assert!(response["iterations"].as_u64().unwrap() > 0);
}

#[test]
fn cli_backup_restore_is_portable_and_does_not_overwrite() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let backup = temp.path().join("backup");
    let destination = temp.path().join("restored");
    let node = ContinentiaNode::new(
        ContinentiaMeta::new(NodeId::new(), 1),
        PhaseVector::zeros(2),
    );
    let path = temp.path().join("node.json");
    std::fs::write(&path, serde_json::to_vec(&node).unwrap()).unwrap();
    assert!(
        cli(&source, &["put", path.to_str().unwrap()])
            .status
            .success()
    );
    assert!(
        cli(&source, &["backup", backup.to_str().unwrap()])
            .status
            .success()
    );
    assert!(
        cli(&destination, &["restore", backup.to_str().unwrap()])
            .status
            .success()
    );
    assert!(
        !cli(&destination, &["restore", backup.to_str().unwrap()])
            .status
            .success()
    );
    assert_eq!(
        cli(&source, &["get", &node.meta.id.to_string()]).stdout,
        cli(&destination, &["get", &node.meta.id.to_string()]).stdout
    );
}
