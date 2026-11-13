//! Launching the user's external editor on a temporary file.

use std::{io::Error as IoError, path::Path, process::Command};

/// Errors that can occur when launching an external editor.
#[derive(Debug, thiserror::Error)]
pub enum Error {
  /// The editor process exited with a non-zero status or was terminated by a signal.
  #[error("Editor '{0}' exited with {1}")]
  EditorFailed(String, String),
  /// The editor command is empty after shell splitting, such as `EDITOR="  "`.
  #[error("No editor configured: set $EDITOR or $VISUAL")]
  EditorNotFound,
  /// An underlying I/O error occurred while reading or writing the temp file.
  #[error(transparent)]
  Io(#[from] IoError),
}

/// Open the user's preferred editor with `initial` text pre-filled in a temporary file using
/// the given `suffix` (e.g. `".md"`) for syntax highlighting, and return the final contents
/// after the editor exits.
///
/// # Arguments
///
/// * `initial` - the text to start with; an empty string leaves the file empty
/// * `suffix` - the temporary file's suffix, such as `".md"`
///
/// # Returns
///
/// The file's contents after the editor exits, or an error when the file cannot be written or read, or the editor
/// fails.
pub fn edit_text_with_suffix(initial: &str, suffix: &str) -> Result<String, Error> {
  let tmp = tempfile::Builder::new().suffix(suffix).tempfile()?;

  if !initial.is_empty() {
    std::fs::write(tmp.path(), initial)?;
  }

  open_editor(tmp.path())?;
  let content = std::fs::read_to_string(tmp.path())?;
  Ok(content)
}

/// Return the editor command from `$EDITOR`, falling back to `$VISUAL`, then `vi`.
///
/// # Returns
///
/// The editor command line, which may hold arguments. An empty `$EDITOR` gives `vi` without trying `$VISUAL`.
pub fn resolve_editor() -> String {
  std::env::var("EDITOR")
    .ok()
    .or_else(|| std::env::var("VISUAL").ok())
    .filter(|s| !s.is_empty())
    .unwrap_or_else(|| "vi".to_string())
}

/// Run the editor on `path` and wait for it to exit.
///
/// The editor command is split with shell quoting rules, so it may carry arguments such as `code --wait`.
///
/// # Arguments
///
/// * `path` - the file to edit
///
/// # Returns
///
/// `Ok(())` when the editor exits successfully; [`Error::EditorFailed`] for a non-zero exit or a signal,
/// [`Error::EditorNotFound`] for an empty command, or [`Error::Io`] when the command cannot be parsed or started.
fn open_editor(path: &Path) -> Result<(), Error> {
  let editor = resolve_editor();
  log::debug!("editor: launching {editor} on {}", path.display());
  let parts =
    shell_words::split(&editor).map_err(|e| Error::Io(std::io::Error::other(format!("bad editor command: {e}"))))?;
  let (program, args) = parts.split_first().ok_or(Error::EditorNotFound)?;
  let status = Command::new(program).args(args).arg(path).status()?;

  if !status.success() {
    let code = status
      .code()
      .map(|c| c.to_string())
      .unwrap_or_else(|| "signal".to_string());
    log::error!("editor: {editor} exited with {code}");
    return Err(Error::EditorFailed(editor, code));
  }

  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;

  mod edit_text_with_suffix {
    use temp_env::with_vars;

    use super::*;

    #[test]
    fn it_creates_temp_file_with_suffix() {
      with_vars([("EDITOR", Some("true")), ("VISUAL", None::<&str>)], || {
        let content = edit_text_with_suffix("markdown content", ".md").unwrap();

        assert_eq!(content, "markdown content");
      });
    }

    #[test]
    fn it_returns_empty_for_empty_initial() {
      with_vars([("EDITOR", Some("true")), ("VISUAL", None::<&str>)], || {
        let content = edit_text_with_suffix("", ".rs").unwrap();

        assert_eq!(content, "");
      });
    }
  }

  mod open_editor {
    use temp_env::with_vars;

    use super::*;

    #[test]
    fn it_errors_on_nonzero_exit() {
      with_vars([("EDITOR", Some("false")), ("VISUAL", None::<&str>)], || {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let result = open_editor(tmp.path());

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, Error::EditorFailed(..)));
      });
    }

    #[test]
    fn it_succeeds_with_true_editor() {
      with_vars([("EDITOR", Some("true")), ("VISUAL", None::<&str>)], || {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let result = open_editor(tmp.path());

        assert!(result.is_ok());
      });
    }
  }

  mod resolve_editor {
    use temp_env::with_vars;

    use super::*;

    #[test]
    fn it_falls_back_to_vi() {
      with_vars([("EDITOR", None::<&str>), ("VISUAL", None::<&str>)], || {
        assert_eq!(resolve_editor(), "vi");
      });
    }

    #[test]
    fn it_falls_back_to_visual() {
      with_vars([("EDITOR", None::<&str>), ("VISUAL", Some("code"))], || {
        assert_eq!(resolve_editor(), "code");
      });
    }

    #[test]
    fn it_prefers_editor_over_visual() {
      with_vars([("EDITOR", Some("nano")), ("VISUAL", Some("code"))], || {
        assert_eq!(resolve_editor(), "nano");
      });
    }
  }
}
