use crate::support::helpers::GestCmd;

#[test]
fn it_undoes_a_task_creation() {
  let g = GestCmd::new();
  let id = g.create_task("Undoable task");

  // Verify the task exists
  g.cmd().args(["task", "show", &id]).assert().success();

  // Undo it
  let undo_out = g.cmd().args(["undo"]).output().expect("undo failed to run");
  assert!(undo_out.status.success(), "undo exited non-zero");
  let stdout = String::from_utf8_lossy(&undo_out.stdout);
  assert!(stdout.contains("undone"), "got: {stdout}");

  // Task should no longer be visible
  let show_out = g.cmd().args(["task", "show", &id]).output().expect("task show failed");
  assert!(!show_out.status.success(), "task show should fail after undo");
}

#[test]
fn it_errors_when_nothing_to_undo() {
  let g = GestCmd::new();
  let output = g.cmd().args(["undo"]).output().expect("undo failed to run");

  assert!(!output.status.success(), "undo should fail when there is no history");
  let stderr = String::from_utf8_lossy(&output.stderr);
  assert!(stderr.to_lowercase().contains("nothing to undo"), "stderr: {stderr}");
}

/// Runs `gest undo` and asserts that it succeeds.
fn undo(g: &GestCmd) {
  let out = g.cmd().args(["undo"]).output().expect("undo failed to run");
  assert!(
    out.status.success(),
    "undo failed: {}",
    String::from_utf8_lossy(&out.stderr)
  );
}

/// Returns the stdout of a command that must succeed.
fn stdout_of(g: &GestCmd, args: &[&str]) -> String {
  let out = g.cmd().args(args).output().expect("command failed to run");
  assert!(
    out.status.success(),
    "{args:?} failed: {}",
    String::from_utf8_lossy(&out.stderr)
  );
  String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn it_undoes_tagging_a_task() {
  let g = GestCmd::new();
  let id = g.create_task("Tagged task");
  g.add_tag_direct(&id, "undo-me");

  undo(&g);

  let show = stdout_of(&g, &["task", "show", &id, "--json"]);
  assert!(!show.contains("undo-me"), "tag still attached: {show}");
}

#[test]
fn it_undoes_adding_a_task_to_an_iteration() {
  let g = GestCmd::new();
  let iteration = g.create_iteration("Sprint");
  let task = g.create_task("Planned task");
  stdout_of(&g, &["iteration", "add", &iteration, &task]);

  undo(&g);

  let show = stdout_of(&g, &["iteration", "show", &iteration, "--json"]);
  assert!(!show.contains(&task), "task still in the iteration: {show}");
}

#[test]
fn it_undoes_creating_a_task_into_an_iteration() {
  let g = GestCmd::new();
  let iteration = g.create_iteration("Sprint");
  let out = stdout_of(
    &g,
    &["task", "create", "Born planned", "--iteration", &iteration, "--quiet"],
  );
  let task = out.trim().to_string();

  undo(&g);

  let show = stdout_of(&g, &["iteration", "show", &iteration, "--json"]);
  assert!(!show.contains(&task), "task still in the iteration: {show}");
}

/// Returns the labels attached to a task, sorted, from `task show --json`.
fn task_tags(g: &GestCmd, id: &str) -> Vec<String> {
  let show = stdout_of(g, &["task", "show", id, "--json"]);
  let json: serde_json::Value = serde_json::from_str(&show).expect("task show --json is JSON");
  let mut tags: Vec<String> = json["tags"]
    .as_array()
    .unwrap_or_else(|| panic!("no tags array: {show}"))
    .iter()
    .map(|t| t.as_str().unwrap().to_string())
    .collect();
  tags.sort();
  tags
}

/// Counts the `entity_tags` rows of an entity with sqlite3. `entity_id` may be a short id, a prefix of the full one.
fn link_count(g: &GestCmd, entity_id: &str) -> usize {
  let out = std::process::Command::new("sqlite3")
    .arg(g.db_path())
    .arg(format!(
      "SELECT COUNT(*) FROM entity_tags WHERE entity_id LIKE '{entity_id}%';"
    ))
    .output()
    .expect("sqlite3 should be available");
  assert!(out.status.success());
  String::from_utf8_lossy(&out.stdout).trim().parse().unwrap()
}

#[test]
fn it_restores_the_tags_a_task_update_replaced() {
  let g = GestCmd::new();
  let out = stdout_of(&g, &["task", "create", "Tagged", "--tag", "a", "--tag", "b", "--quiet"]);
  let id = out.trim().to_string();
  stdout_of(&g, &["task", "update", &id, "--tag", "a", "--tag", "c"]);
  assert_eq!(task_tags(&g, &id), ["a", "c"]);

  undo(&g);

  assert_eq!(task_tags(&g, &id), ["a", "b"]);
}

#[test]
fn it_restores_the_tags_an_artifact_update_replaced() {
  let g = GestCmd::new();
  let id = g.create_artifact("Spec", "body");
  g.add_tag_direct(&id, "a");
  g.add_tag_direct(&id, "b");
  stdout_of(&g, &["artifact", "update", &id, "--tag", "a", "--tag", "c"]);

  undo(&g);

  let show = stdout_of(&g, &["artifact", "show", &id, "--json"]);
  let json: serde_json::Value = serde_json::from_str(&show).unwrap();
  let mut tags: Vec<&str> = json["tags"]
    .as_array()
    .unwrap()
    .iter()
    .map(|t| t.as_str().unwrap())
    .collect();
  tags.sort();
  assert_eq!(tags, ["a", "b"]);
}

#[test]
fn it_undoes_a_task_tag_command() {
  let g = GestCmd::new();
  let id = g.create_task("Tag me");
  g.attach_tag("task", &id, "keep");
  g.attach_tag("task", &id, "drop");

  undo(&g);

  assert_eq!(task_tags(&g, &id), ["keep"]);
}

#[test]
fn it_leaves_no_tag_link_after_undoing_a_tagged_iteration_create() {
  let g = GestCmd::new();
  let out = stdout_of(&g, &["iteration", "create", "Sprint", "--tag", "orphan", "--quiet"]);
  let id = out.trim().to_string();
  assert_eq!(link_count(&g, &id), 1);

  undo(&g);

  assert_eq!(link_count(&g, &id), 0);
}

#[test]
fn it_leaves_no_tag_link_after_undoing_a_tagged_artifact_create() {
  let g = GestCmd::new();
  let out = stdout_of(
    &g,
    &["artifact", "create", "Spec", "--body", "x", "--tag", "t", "--quiet"],
  );
  let id = out.trim().to_string();
  assert_eq!(link_count(&g, &id), 1);

  undo(&g);

  assert_eq!(link_count(&g, &id), 0);
}

#[test]
fn it_restores_the_body_of_an_updated_artifact() {
  let g = GestCmd::new();
  let id = g.create_artifact("Spec", "the old body");
  stdout_of(&g, &["artifact", "update", &id, "--body", "the new body"]);

  undo(&g);

  assert_eq!(artifact_body(&g, &id), "the old body");
}

/// Reads an artifact's body with sqlite3. `id` may be a short id.
fn artifact_body(g: &GestCmd, id: &str) -> String {
  let out = std::process::Command::new("sqlite3")
    .arg(g.db_path())
    .arg(format!("SELECT body FROM artifacts WHERE id LIKE '{id}%';"))
    .output()
    .expect("sqlite3 should be available");
  assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
  String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn it_keeps_the_new_body_when_undoing_a_later_metadata_change() {
  let g = GestCmd::new();
  let id = g.create_artifact("Spec", "the old body");
  stdout_of(&g, &["artifact", "update", &id, "--body", "the new body"]);
  stdout_of(&g, &["artifact", "meta", "set", &id, "owner", "ana"]);

  undo(&g);

  assert_eq!(artifact_body(&g, &id), "the new body");
}

/// Runs one SQL query with sqlite3 and returns its trimmed output.
fn sql(g: &GestCmd, query: &str) -> String {
  let out = std::process::Command::new("sqlite3")
    .arg(g.db_path())
    .arg(query)
    .output()
    .expect("sqlite3 should be available");
  assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
  String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn it_adds_no_tag_when_undoing_the_removal_of_a_tag_the_task_never_had() {
  let g = GestCmd::new();
  let id = g.create_task("Tagged");
  g.add_tag_direct(&id, "a");
  let other = g.create_task("Other");
  g.add_tag_direct(&other, "zzz");
  stdout_of(&g, &["tag", "remove", &id, "zzz"]);

  undo(&g);

  assert_eq!(task_tags(&g, &id), ["a"]);
}

#[test]
fn it_adds_no_tag_when_undoing_a_task_untag_of_a_tag_the_task_never_had() {
  let g = GestCmd::new();
  let id = g.create_task("Tagged");
  g.add_tag_direct(&id, "a");
  let other = g.create_task("Other");
  g.add_tag_direct(&other, "zzz");
  stdout_of(&g, &["task", "untag", &id, "zzz"]);

  undo(&g);

  assert_eq!(task_tags(&g, &id), ["a"]);
}

#[test]
fn it_puts_a_removed_task_back_in_its_phase() {
  let g = GestCmd::new();
  let iteration = g.create_iteration("Sprint");
  let task = g.create_task("Planned");
  stdout_of(&g, &["iteration", "add", &iteration, &task, "--phase", "3"]);
  stdout_of(&g, &["iteration", "remove", &iteration, &task]);

  undo(&g);

  let phase = sql(
    &g,
    &format!(
      "SELECT phase, typeof(phase) FROM iteration_tasks WHERE iteration_id LIKE '{iteration}%' AND task_id LIKE \
        '{task}%';"
    ),
  );
  assert_eq!(phase, "3|integer");
}

#[test]
fn it_adds_no_task_when_undoing_the_removal_of_a_task_that_was_not_in_the_iteration() {
  let g = GestCmd::new();
  let iteration = g.create_iteration("Sprint");
  let task = g.create_task("Never planned");
  stdout_of(&g, &["iteration", "remove", &iteration, &task]);

  undo(&g);

  let count = sql(
    &g,
    &format!("SELECT COUNT(*) FROM iteration_tasks WHERE task_id LIKE '{task}%';"),
  );
  assert_eq!(count, "0");
}

#[test]
fn it_restores_only_the_tags_a_multi_label_remove_really_removed() {
  let g = GestCmd::new();
  let id = g.create_task("Tagged");
  g.add_tag_direct(&id, "a");
  g.add_tag_direct(&id, "b");
  let other = g.create_task("Other");
  g.add_tag_direct(&other, "zzz");
  stdout_of(&g, &["tag", "remove", &id, "a", "zzz", "b", "nope"]);
  assert!(task_tags(&g, &id).is_empty());

  undo(&g);

  assert_eq!(task_tags(&g, &id), ["a", "b"]);
}

#[test]
fn it_undoes_the_add_and_not_a_remove_from_the_wrong_iteration() {
  let g = GestCmd::new();
  let planned = g.create_iteration("Planned");
  let wrong = g.create_iteration("Wrong");
  let task = g.create_task("Task");
  stdout_of(&g, &["iteration", "add", &planned, &task, "--phase", "3"]);
  stdout_of(&g, &["iteration", "remove", &wrong, &task]);
  let query =
    format!("SELECT iteration_id LIKE '{planned}%', phase FROM iteration_tasks WHERE task_id LIKE '{task}%';");
  assert_eq!(
    sql(&g, &query),
    "1|3",
    "the remove from the wrong iteration touched the planned one"
  );

  // The remove changed nothing, so it is not an undo step: undo takes back the add.
  undo(&g);

  assert_eq!(sql(&g, &query), "");
}

#[test]
fn it_restores_the_phase_a_task_update_changed_in_every_iteration() {
  let g = GestCmd::new();
  let first = g.create_iteration("First");
  let second = g.create_iteration("Second");
  let task = g.create_task("Planned");
  stdout_of(&g, &["iteration", "add", &first, &task, "--phase", "2"]);
  stdout_of(&g, &["iteration", "add", &second, &task, "--phase", "4"]);
  stdout_of(&g, &["task", "update", &task, "--phase", "7"]);

  undo(&g);

  let phases = std::process::Command::new("sqlite3")
    .arg(g.db_path())
    .arg(format!(
      "SELECT i.title, it.phase FROM iteration_tasks it JOIN iterations i ON i.id = it.iteration_id \
        WHERE it.task_id LIKE '{task}%' ORDER BY i.title;"
    ))
    .output()
    .expect("sqlite3 should be available");
  assert!(phases.status.success());
  assert_eq!(String::from_utf8_lossy(&phases.stdout).trim(), "First|2\nSecond|4");
}

#[test]
fn it_skips_removals_that_removed_nothing() {
  let g = GestCmd::new();
  let task_id = g.create_task("noop removals");
  let iter_id = g.create_iteration("noop sprint");
  g.attach_tag("task", &task_id, "keep");

  for args in [
    vec!["tag", "remove", &task_id, "never-added"],
    vec!["task", "untag", &task_id, "never-added"],
    vec!["iteration", "remove", &iter_id, &task_id],
  ] {
    g.cmd().args(&args).assert().success();
  }

  undo(&g);

  assert_eq!(
    task_tags(&g, &task_id),
    Vec::<String>::new(),
    "undo should undo the tag, the last real change"
  );
}
