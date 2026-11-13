//! UI rendering and style infrastructure.

/// Reusable UI components for terminal output.
pub mod components;
/// JSON envelope wrapping domain entities with sidecar data.
pub mod envelope;
/// Shared output-mode (`--json`/`--quiet`/`--raw`) flags for command structs.
pub mod json;
/// Markdown-to-styled-terminal renderer.
pub mod markdown;
/// Semantic style tokens and theme resolution.
pub mod style;

use yansi::Condition;

/// Initialize the UI subsystem by enabling colored output when stdout is a TTY
/// or when `CLICOLOR_FORCE` is set (a common convention honored by many CLIs).
pub fn init() {
  // `CLICOLOR_FORCE=1` forces color output on even when stdout is not a TTY,
  // which is how integration tests observe styled prefixes.
  if std::env::var_os("CLICOLOR_FORCE").map(|v| v != "0").unwrap_or(false) {
    yansi::enable();
    return;
  }
  yansi::whenever(Condition::TTY_AND_COLOR);
}

/// Test helpers for rendering while yansi's color switch is global.
///
/// yansi keeps one process-wide switch, and the test harness runs tests on
/// many threads. A test that turns color off, renders, and turns it back on
/// can see another thread turn color on in between, and then its "plain"
/// output carries ANSI escapes. Every test that touches the switch goes
/// through these helpers, which hold one shared lock while they do.
#[cfg(test)]
pub mod testing {
  use std::{
    fmt::Display,
    sync::{Mutex, MutexGuard},
  };

  static COLOR_LOCK: Mutex<()> = Mutex::new(());

  fn lock() -> MutexGuard<'static, ()> {
    COLOR_LOCK.lock().unwrap_or_else(|e| e.into_inner())
  }

  /// Turns color back on when dropped, so a render that panics does not leave it off.
  struct ReEnable;

  impl Drop for ReEnable {
    fn drop(&mut self) {
      yansi::enable();
    }
  }

  /// Render `value` with color turned off, and turn color back on afterwards.
  ///
  /// A unit test that drives `--no-color` through `cli` turns color off without this lock; use these helpers instead.
  pub fn plain(value: impl Display) -> String {
    let _guard = lock();
    let _re_enable = ReEnable;
    yansi::disable();
    value.to_string()
  }

  /// Run `f` with color turned on. `f` must not call [`plain`] or
  /// [`with_color`], because the lock is not re-entrant.
  pub fn with_color<R>(f: impl FnOnce() -> R) -> R {
    let _guard = lock();
    yansi::enable();
    f()
  }

  mod tests {
    use std::{
      sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
      },
      thread,
    };

    use yansi::Paint;

    use super::*;

    #[test]
    fn it_keeps_plain_output_free_of_ansi_while_other_threads_toggle_color() {
      let stop = Arc::new(AtomicBool::new(false));
      let toggler = {
        let stop = Arc::clone(&stop);
        thread::spawn(move || {
          while !stop.load(Ordering::Relaxed) {
            with_color(|| "x".red().to_string());
            plain("y".blue());
          }
        })
      };

      let leaked = (0..20_000)
        .map(|_| plain("text".red()))
        .find(|out| out.contains('\x1b'));
      stop.store(true, Ordering::Relaxed);
      toggler.join().unwrap();

      assert_eq!(leaked, None, "plain output must never carry ANSI escapes");
    }
  }
}
