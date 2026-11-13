//! Integration tests for `gest project delete`.

use std::process;

use crate::support::helpers::{GestCmd, strip_ansi};

/// Extract the full project ID from `project list --json` output whose root
/// path ends with the given `suffix`.
fn project_id_by_root_suffix(g: &GestCmd, suffix: &str) -> String {
  let output = g
    .cmd()
    .args(["project", "list", "--all", "--json"])
    .output()
    .expect("project list --json failed");
  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("valid JSON");
  let arr = parsed.as_array().expect("JSON array");
  for entry in arr {
    if let Some(r) = entry["root"].as_str()
      && r.ends_with(suffix)
    {
      return entry["id"].as_str().unwrap().to_string();
    }
  }
  panic!("no project found with root ending in {suffix} in: {stdout}");
}

/// Run a SQL query against the local database and return the integer result.
fn sql_count(g: &GestCmd, sql: &str) -> i64 {
  let db = g.db_path();
  let output = process::Command::new("sqlite3")
    .arg(db)
    .arg(sql)
    .output()
    .expect("sqlite3 should be available");
  assert!(
    output.status.success(),
    "sqlite3 query failed: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  let stdout = String::from_utf8_lossy(&output.stdout);
  stdout.trim().parse().unwrap_or(0)
}

#[test]
fn it_deletes_a_project_with_owned_entities_and_children() {
  let g = GestCmd::new();

  // Create some entities under the default project.
  let task_id = g.create_task("Delete me task");
  let artifact_id = g.create_artifact("Delete me artifact", "body");
  let iter_id = g.create_iteration("Delete me iteration");

  // Add children: tag the task, add a note, link task to artifact, add task to iteration.
  g.attach_tag("task", &task_id, "urgent");

  let note_output = g
    .cmd()
    .args(["task", "note", "add", &task_id, "--body", "a note"])
    .output()
    .expect("task note add failed");
  assert!(note_output.status.success());

  g.link_task(&task_id, &artifact_id, "artifact", "relates-to");

  let add_output = g
    .cmd()
    .args(["iteration", "add", &iter_id, &task_id, "--phase", "1"])
    .output()
    .expect("iteration add failed");
  assert!(add_output.status.success());

  // Get the project ID.
  let project_id = project_id_by_root_suffix(&g, g.temp_dir_path().file_name().unwrap().to_str().unwrap());

  // Delete with --yes.
  let delete_output = g
    .cmd()
    .args(["project", "delete", &project_id, "--yes"])
    .output()
    .expect("project delete failed");
  assert!(
    delete_output.status.success(),
    "project delete should succeed: {}",
    String::from_utf8_lossy(&delete_output.stderr)
  );

  let stdout = strip_ansi(&String::from_utf8_lossy(&delete_output.stdout));
  assert!(
    stdout.contains("deleted project"),
    "output should confirm deletion: {stdout}"
  );

  // Verify all rows are gone.
  assert_eq!(
    sql_count(&g, &format!("SELECT COUNT(*) FROM projects WHERE id = '{project_id}';")),
    0,
    "project row should be deleted"
  );
  assert_eq!(
    sql_count(
      &g,
      &format!("SELECT COUNT(*) FROM tasks WHERE project_id = '{project_id}';")
    ),
    0,
    "task rows should be deleted"
  );
  assert_eq!(
    sql_count(
      &g,
      &format!("SELECT COUNT(*) FROM artifacts WHERE project_id = '{project_id}';")
    ),
    0,
    "artifact rows should be deleted"
  );
  assert_eq!(
    sql_count(
      &g,
      &format!("SELECT COUNT(*) FROM iterations WHERE project_id = '{project_id}';")
    ),
    0,
    "iteration rows should be deleted"
  );
  assert_eq!(
    sql_count(
      &g,
      &format!("SELECT COUNT(*) FROM transactions WHERE project_id = '{project_id}';")
    ),
    0,
    "transaction rows should be deleted"
  );

  // Verify tombstone files exist on disk.
  let gest_dir = g.temp_dir_path().join(".gest");
  let project_yaml = gest_dir.join("project.yaml");
  if project_yaml.exists() {
    let contents = std::fs::read_to_string(&project_yaml).unwrap();
    assert!(
      contents.contains("deleted_at"),
      "project.yaml should be tombstoned: {contents}"
    );
  }
}

#[test]
fn it_is_not_reversible_via_undo() {
  let g = GestCmd::new();
  let task_id = g.create_task("Undo test task");

  let project_id = project_id_by_root_suffix(&g, g.temp_dir_path().file_name().unwrap().to_str().unwrap());

  // Delete the project.
  let delete_output = g
    .cmd()
    .args(["project", "delete", &project_id, "--yes"])
    .output()
    .expect("project delete failed");
  assert!(delete_output.status.success());

  // Attempt undo -- should fail or have no effect since project delete is non-undoable.
  let undo_output = g.cmd().args(["undo"]).output().expect("undo failed to run");

  // The undo should either fail (no transactions) or succeed without restoring the project.
  // Either way, the project and its tasks should remain deleted.
  assert_eq!(
    sql_count(&g, &format!("SELECT COUNT(*) FROM projects WHERE id = '{project_id}';")),
    0,
    "project should not be restored by undo"
  );
  assert_eq!(
    sql_count(&g, &format!("SELECT COUNT(*) FROM tasks WHERE id = '{task_id}';")),
    0,
    "task should not be restored by undo"
  );

  let _ = undo_output;
}

#[test]
fn it_requires_confirmation_without_yes_flag() {
  let g = GestCmd::new();

  let project_id = project_id_by_root_suffix(&g, g.temp_dir_path().file_name().unwrap().to_str().unwrap());

  // Without --yes and with no tty (piped stdin with no input), the prompt
  // defaults to No and the command exits successfully without deleting.
  let output = g
    .cmd()
    .args(["project", "delete", &project_id])
    .output()
    .expect("project delete failed to run");

  assert!(
    output.status.success(),
    "project delete should succeed (prompt defaults to No): {}",
    String::from_utf8_lossy(&output.stderr)
  );

  // The project should still exist because confirmation was not given.
  assert_eq!(
    sql_count(&g, &format!("SELECT COUNT(*) FROM projects WHERE id = '{project_id}';")),
    1,
    "project should not be deleted when confirmation is not provided"
  );
}
