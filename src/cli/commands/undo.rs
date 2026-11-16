//! `gest undo`: undo the last command.

use clap::Args;

use crate::{AppContext, cli::Error, store::repo, ui::components::SuccessMessage};

/// Undo the last command.
#[derive(Args, Debug)]
pub struct Command {
  /// Number of commands to undo.
  #[arg(default_value = "1")]
  steps: u32,
}

impl Command {
  /// Roll back the most recent `steps` undoable transactions for the current project.
  ///
  /// Each transaction is undone atomically, newest first, and printed when it is done. When one fails, the ones
  /// before it stay undone and the failed one and every older one are left as they were, so running the command
  /// again resumes from the failed one.
  ///
  /// A transaction that gest recorded before join-table events carried their key can never be undone. It is
  /// skipped with a warning, marked so that undo does not offer it again, and does not count as a step.
  ///
  /// # Arguments
  ///
  /// * `context` - the application context: settings, store and current project
  ///
  /// # Returns
  ///
  /// `Ok(())` when the command succeeds, or the error that stopped it.
  pub async fn call(&self, context: &AppContext) -> Result<(), Error> {
    log::debug!("undo: entry");
    let project_id = context.project_id().as_ref().ok_or(Error::UninitializedProject)?;
    let conn = context.store().connect().await?;

    let mut undone = 0;
    while undone < self.steps {
      let Some(tx) = repo::transaction::latest_undoable(&conn, project_id).await? else {
        break;
      };
      if let Some(reason) = repo::transaction::missing_join_key(&conn, tx.id()).await? {
        repo::transaction::mark_skipped(&conn, tx.id()).await?;
        log::warn!(
          "skipped {} ({}): it cannot be undone, because gest recorded it in an older format ({reason}); \
            its changes stay",
          tx.id().short(),
          tx.command()
        );
        continue;
      }
      let command = repo::transaction::undo(&conn, tx.id()).await?;
      let message = SuccessMessage::new("undone").field("command", command);
      println!("{message}");
      undone += 1;
    }

    if undone == 0 {
      return Err(crate::store::Error::NothingToUndo.into());
    }

    Ok(())
  }
}
