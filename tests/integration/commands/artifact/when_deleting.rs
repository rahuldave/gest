use crate::support::helpers::GestCmd;

#[test]
fn it_deletes_an_artifact_with_dependents_and_writes_a_tombstone_file() {
  let g = GestCmd::new();
  let artifact_id = g.create_artifact("Doomed spec", "body");
  g.attach_tag("artifact", &artifact_id, "design");
  let task_id = g.create_task("relates");
  let link_output = g
    .cmd()
    .args([
      "task",
      "link",
      &task_id,
      &artifact_id,
      "--artifact",
      "--rel",
      "relates-to",
    ])
    .output()
    .expect("task link failed to run");
  assert!(
    link_output.status.success(),
    "task link failed: {}",
    String::from_utf8_lossy(&link_output.stderr)
  );
  let note_output = g
    .cmd()
    .args(["artifact", "note", "add", &artifact_id, "--body", "keep it"])
    .output()
    .expect("artifact note add failed to run");
  assert!(
    note_output.status.success(),
    "artifact note add failed: {}",
    String::from_utf8_lossy(&note_output.stderr)
  );

  let output = g
    .cmd()
    .args(["artifact", "delete", &artifact_id, "--yes"])
    .output()
    .expect("artifact delete failed to run");

  assert!(
    output.status.success(),
    "artifact delete exited non-zero: {}",
    String::from_utf8_lossy(&output.stderr)
  );
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(stdout.contains("deleted artifact"), "got: {stdout}");

  // Row is gone.
  let show = g
    .cmd()
    .args(["artifact", "show", &artifact_id])
    .output()
    .expect("artifact show failed to run");
  assert!(!show.status.success(), "expected show to fail after delete");

  // Tombstone file persists on disk with deleted_at in the frontmatter.
  let artifact_dir = g.temp_dir_path().join(".gest").join("artifact");
  let tombstone = std::fs::read_dir(&artifact_dir)
    .expect("read artifact dir")
    .flatten()
    .map(|e| e.path())
    .find(|p| {
      p.extension().is_some_and(|ext| ext == "md")
        && p
          .file_stem()
          .and_then(|s| s.to_str())
          .is_some_and(|s| s.starts_with(&artifact_id))
    })
    .expect("tombstone .md file should still exist after delete");
  let raw = std::fs::read_to_string(&tombstone).expect("read tombstone");
  assert!(raw.contains("deleted_at:"), "frontmatter missing deleted_at:\n{raw}");
}

#[test]
fn it_is_undoable_and_restores_dependents() {
  let g = GestCmd::new();
  let artifact_id = g.create_artifact("Restorable", "body");
  g.attach_tag("artifact", &artifact_id, "keep");

  let delete = g
    .cmd()
    .args(["artifact", "delete", &artifact_id, "--yes"])
    .output()
    .expect("artifact delete failed to run");
  assert!(delete.status.success());

  let undo = g.cmd().args(["undo"]).output().expect("undo failed to run");
  assert!(
    undo.status.success(),
    "undo exited non-zero: {}",
    String::from_utf8_lossy(&undo.stderr)
  );

  let show = g
    .cmd()
    .args(["artifact", "show", &artifact_id])
    .output()
    .expect("artifact show failed to run");
  assert!(
    show.status.success(),
    "expected show to succeed after undo: {}",
    String::from_utf8_lossy(&show.stderr)
  );
  let stdout = String::from_utf8_lossy(&show.stdout);
  assert!(stdout.contains("Restorable"), "got: {stdout}");
  assert!(stdout.contains("keep"), "expected restored tag in output: {stdout}");
}

#[test]
fn it_prints_the_id_on_quiet_delete() {
  let g = GestCmd::new();
  let id = g.create_artifact("Doomed", "body");
  let output = g
    .cmd()
    .args(["artifact", "delete", &id, "--yes", "-q"])
    .output()
    .unwrap();

  assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  let printed = String::from_utf8_lossy(&output.stdout).trim().to_string();
  assert!(
    !printed.is_empty() && id.starts_with(&printed),
    "printed {printed:?} for {id}"
  );
}

#[test]
fn it_does_not_claim_quiet_prints_nothing_on_delete() {
  let g = GestCmd::new();
  let help = g.cmd().args(["artifact", "delete", "--help"]).output().unwrap();
  let help = String::from_utf8_lossy(&help.stdout);

  assert!(help.contains("print at most a bare ID, label or value"), "{help}");
}
