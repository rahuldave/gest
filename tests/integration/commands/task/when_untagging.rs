use crate::support::helpers::GestCmd;

#[test]
fn it_removes_single_tag() {
  let g = GestCmd::new();
  let task_id = g.create_task("tagged task");
  g.attach_tag("task", &task_id, "keep");
  g.attach_tag("task", &task_id, "drop");

  let output = g
    .cmd()
    .args(["task", "untag", &task_id, "drop"])
    .output()
    .expect("task untag failed");
  assert!(
    output.status.success(),
    "task untag should succeed: {}",
    String::from_utf8_lossy(&output.stderr)
  );

  let list = g
    .cmd()
    .args(["tag", "list", "--task"])
    .output()
    .expect("tag list failed");
  let stdout = String::from_utf8_lossy(&list.stdout);
  assert!(stdout.contains("keep"), "keep tag should remain, got: {stdout}");
  assert!(!stdout.contains("drop"), "drop tag should be gone, got: {stdout}");
}

#[test]
fn it_removes_nonexistent_tag_gracefully() {
  let g = GestCmd::new();
  let task_id = g.create_task("no tags");

  let output = g
    .cmd()
    .args(["task", "untag", &task_id, "never-existed"])
    .output()
    .expect("task untag failed");

  // The command should either succeed as a no-op or fail explicitly; either way
  // it should not panic or leave the task in a broken state.
  let show = g
    .cmd()
    .args(["task", "show", &task_id])
    .output()
    .expect("task show failed");
  assert!(
    show.status.success(),
    "task should remain accessible regardless of untag outcome; status={:?}",
    output.status.code()
  );
}

#[test]
fn it_says_when_the_task_did_not_have_the_tag() {
  let g = GestCmd::new();
  let task_id = g.create_task("untagged already");

  let output = g
    .cmd()
    .args(["task", "untag", &task_id, "never-existed"])
    .output()
    .expect("task untag failed");

  assert!(output.status.success());
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("task was not tagged"), "got: {stdout}");
}
