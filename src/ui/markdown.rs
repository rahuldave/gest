//! Markdown-to-styled-terminal renderer.
//!
//! Two-pass pipeline: [`parse`] converts markdown into a typed [`Block`](block::Block) IR,
//! then [`render`] walks that IR to produce styled terminal output. The IR
//! split keeps each pass small and independently testable.

mod block;
mod parse;
mod render;

/// Render a markdown string to styled terminal output, fitted to `width` columns.
///
/// # Arguments
///
/// * `input` - the markdown source, parsed with every pulldown-cmark extension enabled
/// * `width` - the column budget for wrapping; `0` turns wrapping off
///
/// # Returns
///
/// The styled text, with ANSI escapes when color is enabled.
pub fn render(input: &str, width: usize) -> String {
  render::render(&parse::parse(input), width)
}
