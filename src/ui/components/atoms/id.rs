//! Two-tone entity identifier display for terminal output.

use std::{
  collections::HashMap,
  fmt::{self, Display, Formatter},
};

use yansi::Paint;

/// Displays a truncated, two-tone entity identifier (highlighted prefix + dimmed suffix).
///
/// At most 8 characters are shown. The first `prefix_len` characters are rendered
/// in the [`id.prefix`](crate::ui::style::Theme::id_prefix) style and the remainder
/// in [`id.rest`](crate::ui::style::Theme::id_rest).
///
/// Use `min_unique_prefix` to compute the shortest prefix that uniquely
/// identifies each ID in a set.
pub struct Component<'a> {
  /// Number of leading characters in the prefix style; clamped to the shown length.
  prefix_len: usize,
  /// Full identifier; only its first 8 characters are shown.
  value: &'a str,
}

impl<'a> Component<'a> {
  /// Create an id display, showing at most 8 characters with a 2-char highlighted prefix.
  ///
  /// # Arguments
  ///
  /// * `value` - the full identifier
  ///
  /// # Returns
  ///
  /// The display, with a 2-character prefix highlighted.
  pub fn new(value: &'a str) -> Self {
    Self {
      prefix_len: 2,
      value,
    }
  }

  /// Set the number of highlighted prefix characters.
  ///
  /// # Arguments
  ///
  /// * `len` - the prefix length, usually from [`unique_prefix_lengths`]
  ///
  /// # Returns
  ///
  /// The display with the new prefix length.
  pub fn prefix_len(mut self, len: usize) -> Self {
    self.prefix_len = len;
    self
  }
}

impl Display for Component<'_> {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    let theme = crate::ui::style::global();
    let display: String = self.value.chars().take(8).collect();
    let prefix_len = self.prefix_len.min(display.len());
    let (prefix, rest) = display.split_at(prefix_len);

    write!(
      f,
      "{}{}",
      prefix.paint(*theme.id_prefix()),
      rest.paint(*theme.id_rest()),
    )
  }
}

/// Compute the minimum prefix length that uniquely identifies every short ID
/// in the set (each ID is first truncated to 8 chars).
///
/// Returns at least 1 and at most 8.
#[cfg(test)]
pub fn min_unique_prefix(ids: &[&str]) -> usize {
  use std::collections::HashSet;

  let shorts: Vec<String> = ids.iter().map(|id| id.chars().take(8).collect()).collect();

  for len in 1..=8 {
    let prefixes: Vec<&str> = shorts.iter().map(|s| &s[..len.min(s.len())]).collect();
    let unique: HashSet<&str> = prefixes.iter().copied().collect();
    if unique.len() == shorts.len() {
      return len;
    }
  }
  8
}

/// Compute two-tier prefix lengths: active IDs are resolved against the active
/// pool only, while archived IDs (those in `all_ids` but not in `active_ids`)
/// are resolved against the full pool.
///
/// # Arguments
///
/// * `active_ids` - the IDs of active entities; a subset of `all_ids`
/// * `all_ids` - every ID, active and archived, in display order
///
/// # Returns
///
/// A `Vec<usize>` aligned to `all_ids`.
pub fn prefix_lengths_two_tier(active_ids: &[&str], all_ids: &[&str]) -> Vec<usize> {
  let active_lengths = unique_prefix_lengths(active_ids);
  let all_lengths = unique_prefix_lengths(all_ids);

  let active_map: HashMap<&str, usize> = active_ids.iter().copied().zip(active_lengths).collect();

  all_ids
    .iter()
    .enumerate()
    .map(|(i, &id)| {
      if let Some(&len) = active_map.get(id) {
        len
      } else {
        all_lengths[i]
      }
    })
    .collect()
}

/// Compute the shortest prefix that distinguishes a single target ID from
/// every other ID in the pool (each ID is first truncated to 8 chars).
///
/// The pool may optionally contain `target`; entries whose full string equals
/// `target` are skipped once so callers can pass pools assembled via a
/// narrow `LIKE` query without special-casing the target's own row. Pool
/// entries whose 8-char prefix collides with `target`'s but whose full IDs
/// differ are treated like any other colliding entry and drive the result
/// up to the 8-char cap.
///
/// # Arguments
///
/// * `target` - the ID to shorten
/// * `pool` - the IDs it must be told apart from; it may contain `target` itself
///
/// # Returns
///
/// The prefix length, at least 1 and at most 8.
pub fn unique_prefix_length_for_id(target: &str, pool: &[&str]) -> usize {
  let target_short: String = target.chars().take(8).collect();
  let mut len = 1usize;
  let mut skipped_self = false;
  for other in pool {
    if !skipped_self && *other == target {
      skipped_self = true;
      continue;
    }
    let other_short: String = other.chars().take(8).collect();
    let common = target_short
      .chars()
      .zip(other_short.chars())
      .take_while(|(a, b)| a == b)
      .count();
    len = len.max((common + 1).min(8));
  }
  len
}

/// Compute the shortest prefix that distinguishes each ID from every other ID
/// in the input slice (each ID is first truncated to 8 chars).
///
/// # Arguments
///
/// * `ids` - the IDs to shorten together
///
/// # Returns
///
/// An index-aligned `Vec<usize>` — one prefix length per input ID.
/// Each length is at least 1 and at most 8.
pub fn unique_prefix_lengths(ids: &[&str]) -> Vec<usize> {
  let shorts: Vec<String> = ids.iter().map(|id| id.chars().take(8).collect()).collect();

  shorts
    .iter()
    .enumerate()
    .map(|(i, s)| {
      let mut len = 1usize;
      for (j, other) in shorts.iter().enumerate() {
        if i == j {
          continue;
        }
        let common = s.chars().zip(other.chars()).take_while(|(a, b)| a == b).count();
        len = len.max((common + 1).min(8));
      }
      len
    })
    .collect()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod min_unique_prefix_fn {
    use super::*;

    #[test]
    fn it_handles_identical_ids() {
      let ids = vec!["aaaaaaaa", "aaaaaaaa"];

      assert_eq!(min_unique_prefix(&ids), 8);
    }

    #[test]
    fn it_increases_when_ids_share_a_prefix() {
      let ids = vec!["aaxyz111", "aaxyz222", "bbcde333"];

      // "a"/"b" collide at len=1, "aa" collides at len=2..5, unique at len=6 (aaxyz1 vs aaxyz2)
      assert_eq!(min_unique_prefix(&ids), 6);
    }

    #[test]
    fn it_returns_1_for_a_single_id() {
      let ids = vec!["abcdefgh"];

      assert_eq!(min_unique_prefix(&ids), 1);
    }

    #[test]
    fn it_returns_1_for_empty_list() {
      let ids: Vec<&str> = vec![];

      assert_eq!(min_unique_prefix(&ids), 1);
    }

    #[test]
    fn it_returns_1_when_all_ids_differ_at_first_char() {
      let ids = vec!["abcd1234", "bcde2345", "cdef3456"];

      assert_eq!(min_unique_prefix(&ids), 1);
    }
  }

  mod prefix_lengths_two_tier_fn {
    use super::*;

    #[test]
    fn it_gives_active_ids_shorter_prefixes_than_archived() {
      // Active IDs all differ at first char → prefix 1
      let active = vec!["aaa11111", "bbb22222", "ccc33333"];
      // Full pool adds an archived ID sharing prefix with "aaa11111"
      let all = vec!["aaa11111", "bbb22222", "ccc33333", "aaa44444"];

      let lengths = prefix_lengths_two_tier(&active, &all);

      // Active IDs keep their short prefixes (computed against active pool only)
      assert_eq!(lengths[0], 1); // "aaa11111" — unique among active
      assert_eq!(lengths[1], 1); // "bbb22222"
      assert_eq!(lengths[2], 1); // "ccc33333"
      // Archived ID needs longer prefix (computed against full pool)
      assert_eq!(lengths[3], 4); // "aaa44444" vs "aaa11111" — need 4 chars
    }
  }

  mod unique_prefix_length_for_id_fn {
    use super::*;

    #[test]
    fn it_caps_at_eight_when_full_short_id_collides() {
      // The target's 8-char prefix matches another entry's 8-char prefix but
      // the full IDs differ past character 8 (pool values longer than 8).
      let pool = vec!["aaaaaaaab", "aaaaaaaaz"];

      assert_eq!(unique_prefix_length_for_id("aaaaaaaax", &pool), 8);
    }

    #[test]
    fn it_skips_target_entry_in_pool() {
      // Narrow-query pools include the target row itself; skip it so the
      // target's own ID doesn't force the length to 8 spuriously.
      let pool = vec!["aaxyz111", "aaxyz222"];

      assert_eq!(unique_prefix_length_for_id("aaxyz111", &pool), 6);
    }

    #[test]
    fn it_returns_1_for_empty_pool() {
      let pool: Vec<&str> = vec![];

      assert_eq!(unique_prefix_length_for_id("abcdefgh", &pool), 1);
    }

    #[test]
    fn it_returns_1_when_pool_shares_no_first_char() {
      let pool = vec!["bcde1234", "cdef5678"];

      assert_eq!(unique_prefix_length_for_id("abcd0000", &pool), 1);
    }

    #[test]
    fn it_returns_common_plus_one_for_sibling_in_pool() {
      let pool = vec!["aaxyz111", "aaxyz222", "bbcde333"];

      // "aaxyz1" vs "aaxyz2" → 5 common chars → prefix 6
      assert_eq!(unique_prefix_length_for_id("aaxyz111", &pool), 6);
    }

    #[test]
    fn it_matches_unique_prefix_lengths_for_same_pool() {
      // Single-ID narrow path must agree with the full-pool per-ID computation.
      let pool = vec!["xmaaaaaa", "wbbbbbbb", "pccccccc", "xtdddddd", "oeeeeeee"];
      let full = unique_prefix_lengths(&pool);

      for (i, id) in pool.iter().enumerate() {
        assert_eq!(unique_prefix_length_for_id(id, &pool), full[i]);
      }
    }
  }

  mod unique_prefix_lengths_fn {
    use super::*;

    #[test]
    fn it_produces_per_id_variable_lengths() {
      let ids = vec!["xmaaaaaa", "wbbbbbbb", "pccccccc", "xtdddddd", "oeeeeeee"];

      let lengths = unique_prefix_lengths(&ids);

      assert_eq!(lengths, vec![2, 1, 1, 2, 1]);
    }

    #[test]
    fn it_returns_1_for_a_single_id() {
      let ids = vec!["abcdefgh"];

      assert_eq!(unique_prefix_lengths(&ids), vec![1]);
    }

    #[test]
    fn it_returns_empty_vec_for_empty_list() {
      let ids: Vec<&str> = vec![];

      assert_eq!(unique_prefix_lengths(&ids), Vec::<usize>::new());
    }

    #[test]
    fn it_returns_8_for_all_identical_ids() {
      let ids = vec!["aaaaaaaa", "aaaaaaaa", "aaaaaaaa"];

      assert_eq!(unique_prefix_lengths(&ids), vec![8, 8, 8]);
    }
  }

  mod fmt {
    use super::*;

    #[test]
    fn it_renders_short_id_without_padding() {
      let id = Component::new("abc");
      let rendered = id.to_string();

      assert!(rendered.contains("ab"));
      assert!(rendered.contains("c"));
      assert!(!rendered.contains(' '));
    }

    #[test]
    fn it_respects_custom_prefix_len() {
      yansi::disable();
      let id = Component::new("abcdefgh").prefix_len(4);
      let rendered = id.to_string();
      yansi::enable();

      // All 8 chars should be present
      assert_eq!(rendered, "abcdefgh");
    }

    #[test]
    fn it_truncates_to_8_characters() {
      let id = Component::new("abcdefghijklmnop");
      let rendered = id.to_string();

      assert!(rendered.contains("ab"));
      assert!(rendered.contains("cdefgh"));
      assert!(!rendered.contains("ijklmnop"));
    }
  }
}
