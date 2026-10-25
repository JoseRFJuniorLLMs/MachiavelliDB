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
#[test]
fn cli_atomic_batch_roundtrips_strategic_state_and_rejects_stale_sequence() {
    use machiavelli_core::{EdgeId, EdgeKind, StrategicEdge};
    use machiavelli_storage::{Mutation, StrategicRecord};
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("db");
    let file = temp.path().join("batch.json");
    let a = ContinentiaNode::new(
        ContinentiaMeta::new(NodeId::new(), 1),
        PhaseVector::zeros(1),
    );
    let b = ContinentiaNode::new(
        ContinentiaMeta::new(NodeId::new(), 1),
        PhaseVector::zeros(1),
    );
    let edge = StrategicRecord::Edge {
        edge: StrategicEdge {
            id: EdgeId::new(),
            from: a.meta.id,
            to: b.meta.id,
            kind: EdgeKind::Threat,
            weight: 0.75,
        },
    };
    let ops = vec![
        Mutation::Put(a),
        Mutation::Put(b),
        Mutation::PutStrategic(edge.clone()),
    ];
    std::fs::write(&file, serde_json::to_vec(&ops).unwrap()).unwrap();
    let result = cli(&db, &["commit", file.to_str().unwrap(), "0"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        !cli(&db, &["commit", file.to_str().unwrap(), "0"])
            .status
            .success()
    );
    let output = cli(&db, &["records"]);
    let records: Vec<StrategicRecord> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(records, vec![edge]);
    let stats: serde_json::Value = serde_json::from_slice(&cli(&db, &["stats"]).stdout).unwrap();
    assert_eq!(stats["sequence"], 1);
    assert_eq!(stats["strategic_records"], 1);
}
#[test]
fn cli_solver_resumes_durable_cfr_state_across_processes_and_backup() {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("db");
    let first = cli(&db, &["train-kuhn", "500", "10000"]);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let second = cli(&db, &["train-kuhn", "500", "10000"]);
    assert!(
        second.status.success(),
        "{}",
        String::from_utf8_lossy(&second.stderr)
    );
    let response: serde_json::Value = serde_json::from_slice(&second.stdout).unwrap();
    assert_eq!(response["cumulative_iterations"], 1000);
    assert_eq!(response["sequence"], 2);
    let backup = temp.path().join("backup");
    let restored = temp.path().join("restored");
    assert!(
        cli(&db, &["backup", backup.to_str().unwrap()])
            .status
            .success()
    );
    assert!(
        cli(&restored, &["restore", backup.to_str().unwrap()])
            .status
            .success()
    );
    let result = cli(&restored, &["train-kuhn", "1000", "10000"]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let response: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(response["cumulative_iterations"], 2000);
    assert!(response["exploitability"].as_f64().unwrap() < 0.001);
}
#[test]
fn cli_mcts_plans_are_reproducible_and_do_not_commit_live_state() {
    let temp = tempfile::tempdir().unwrap();
    let db = temp.path().join("db");
    let file = temp.path().join("node.json");
    let id = NodeId::new();
    let node = ContinentiaNode::new(
        ContinentiaMeta::new(id, 1),
        PhaseVector::with_constitution(vec![0.0], vec![4.0], vec![7.0]).unwrap(),
    );
    std::fs::write(&file, serde_json::to_vec(&node).unwrap()).unwrap();
    assert!(cli(&db, &["put", file.to_str().unwrap()]).status.success());
    let before = cli(&db, &["get", &id.to_string()]);
    let args = ["plan", &id.to_string(), "3000", "1", "42", "10000"];
    let a = cli(&db, &args);
    let b = cli(&db, &args);
    assert!(a.status.success(), "{}", String::from_utf8_lossy(&a.stderr));
    assert_eq!(a.stdout, b.stdout);
    let result: serde_json::Value = serde_json::from_slice(&a.stdout).unwrap();
    assert_eq!(result["persisted"], false);
    assert_eq!(result["source_sequence"], 1);
    assert_eq!(result["search"]["selected_action"], "damp");
    assert_eq!(result["search"]["iterations"], 3000);
    assert_eq!(cli(&db, &["get", &id.to_string()]).stdout, before.stdout);
    assert!(
        !cli(&db, &["plan", &id.to_string(), "0", "1", "42", "1000"])
            .status
            .success()
    );
}
