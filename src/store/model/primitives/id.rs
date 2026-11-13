//! The 128-bit identifier of every entity, written as 32 letters from `k` to `z`.

use std::{
  fmt::{self, Display, Formatter},
  str::FromStr,
};

use libsql::Value;
use rand::RngExt;
use serde::{Deserialize, Serialize};

/// A 128-bit identifier encoded as a 32-character string using the `k`-`z` alphabet.
///
/// Each byte is split into two nibbles and mapped through a reverse-hex alphabet,
/// producing a lowercase, URL-safe string that sorts in reverse byte order.
/// The encoding is bijective: every valid 32-character `[k-z]` string decodes
/// to exactly one `[u8; 16]` and vice-versa.
///
/// # Examples
///
/// ```
/// # use gest::store::model::primitives::Id;
/// let id = Id::new();
/// assert_eq!(id.to_string().len(), 32);
///
/// let parsed: Id = id.to_string().parse().unwrap();
/// assert_eq!(id, parsed);
/// ```
#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(from = "String", into = "String")]
pub struct Primitive([u8; 16]);

impl Primitive {
  /// Alphabet used to encode bytes into the `k`-`z` character range.
  const REVERSE_HEX_CHARS: &[u8; 16] = b"zyxwvutsrqponmlk";

  /// Generate a new random `Id` using the thread-local RNG.
  ///
  /// # Returns
  ///
  /// An id of 16 random bytes.
  pub fn new() -> Self {
    let mut rng = rand::rng();
    let mut bytes = [0u8; 16];
    for b in &mut bytes {
      *b = rng.random();
    }
    Self(bytes)
  }

  /// Validate that `s` is a valid ID prefix: 1-32 characters, all in `[k-z]`.
  ///
  /// # Arguments
  ///
  /// * `s` - the prefix to check
  ///
  /// # Returns
  ///
  /// Returns `Ok(s)` on success so callers can chain the result. Rejects empty
  /// strings, strings longer than 32 characters, and any character outside the
  /// ID alphabet. The error message says what is wrong.
  pub fn validate_prefix(s: &str) -> Result<&str, String> {
    if s.is_empty() {
      return Err("Id prefix must not be empty".to_string());
    }
    if s.len() > 32 {
      return Err(format!("Id prefix must be at most 32 characters, got {}", s.len()));
    }
    if let Some(c) = s.chars().find(|c| !('k'..='z').contains(c)) {
      return Err(format!("Id prefix must contain only characters k-z, found '{c}'"));
    }
    Ok(s)
  }

  /// Returns the first 8 characters of the encoded ID as a short prefix.
  ///
  /// # Returns
  ///
  /// The characters that encode the first 4 bytes.
  pub fn short(&self) -> String {
    Self::encode_prefix(&self.0)
  }

  /// Decode a 32-character `[k-z]` string back into 16 raw bytes.
  ///
  /// # Arguments
  ///
  /// * `s` - the encoded id
  ///
  /// # Returns
  ///
  /// The bytes, or an error message when `s` is not 32 bytes long or holds a character outside `k`-`z`.
  fn decode(s: &str) -> Result<[u8; 16], String> {
    let chars: Vec<u8> = s.bytes().collect();
    if chars.len() != 32 {
      return Err(format!("Id must be exactly 32 characters, got {}", chars.len()));
    }

    let mut bytes = [0u8; 16];
    for i in 0..16 {
      let high = Self::nibble_from_char(chars[i * 2])?;
      let low = Self::nibble_from_char(chars[i * 2 + 1])?;
      bytes[i] = (high << 4) | low;
    }
    Ok(bytes)
  }

  /// Encode 16 raw bytes into a 32-character `[k-z]` string.
  ///
  /// # Arguments
  ///
  /// * `bytes` - the raw id
  ///
  /// # Returns
  ///
  /// The encoded id, two characters per byte, high nibble first.
  fn encode(bytes: &[u8; 16]) -> String {
    let mut s = String::with_capacity(32);
    for &b in bytes {
      let high = (b >> 4) as usize;
      let low = (b & 0x0F) as usize;
      s.push(Self::REVERSE_HEX_CHARS[high] as char);
      s.push(Self::REVERSE_HEX_CHARS[low] as char);
    }
    s
  }

  /// Encode only the first 4 bytes into an 8-character string for [`Primitive::short`].
  ///
  /// # Arguments
  ///
  /// * `bytes` - the raw id
  ///
  /// # Returns
  ///
  /// The 8-character prefix.
  fn encode_prefix(bytes: &[u8; 16]) -> String {
    let mut s = String::with_capacity(8);
    for &b in &bytes[..4] {
      let high = (b >> 4) as usize;
      let low = (b & 0x0F) as usize;
      s.push(Self::REVERSE_HEX_CHARS[high] as char);
      s.push(Self::REVERSE_HEX_CHARS[low] as char);
    }
    s
  }

  /// Convert a single `[k-z]` character byte to its nibble value (`0..=15`).
  ///
  /// # Arguments
  ///
  /// * `c` - the character, as a byte
  ///
  /// # Returns
  ///
  /// The nibble, where `z` is 0 and `k` is 15, or an error message for any other byte.
  fn nibble_from_char(c: u8) -> Result<u8, String> {
    match c {
      b'k'..=b'z' => Ok(b'z' - c),
      _ => Err(format!("Id must contain only characters k-z, found '{}'", c as char)),
    }
  }
}

/// Delegates to [`Primitive::new`], generating a fresh random identifier.
impl Default for Primitive {
  fn default() -> Self {
    Self::new()
  }
}

/// Formats the ID as a 32-character `[k-z]` encoded string.
impl Display for Primitive {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    f.write_str(&Self::encode(&self.0))
  }
}

/// Converts to the 32-character encoded string representation.
impl From<Primitive> for String {
  fn from(id: Primitive) -> Self {
    id.to_string()
  }
}

/// Converts to a [`Value::Text`] for storage in libsql.
impl From<Primitive> for Value {
  fn from(id: Primitive) -> Self {
    Value::from(id.to_string())
  }
}

/// Parses a 32-character `[k-z]` string into an `Id`.
///
/// Returns an error if the string is not exactly 32 characters or contains
/// characters outside the `[k-z]` range.
impl FromStr for Primitive {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    if s.len() != 32 {
      return Err(format!("Id must be exactly 32 characters, got {}", s.len()));
    }
    let bytes = Self::decode(s)?;
    Ok(Self(bytes))
  }
}

/// Parses the string into an `Id`, panicking on invalid input.
///
/// Prefer [`FromStr`] / [`TryFrom<String>`] when handling untrusted input.
impl From<String> for Primitive {
  fn from(s: String) -> Self {
    s.parse().unwrap_or_else(|e| panic!("invalid Id: {e}"))
  }
}

/// Parses the string into an `Id`, panicking on invalid input.
///
/// Prefer [`FromStr`] / [`TryFrom<String>`] when handling untrusted input.
impl From<&str> for Primitive {
  fn from(s: &str) -> Self {
    s.parse().unwrap_or_else(|e| panic!("invalid Id: {e}"))
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod encode_decode {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_encodes_ff_bytes_as_k() {
      let bytes = [0xFF; 16];
      assert_eq!(Primitive::encode(&bytes), "kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk");
    }

    #[test]
    fn it_encodes_zero_bytes_as_z() {
      let bytes = [0u8; 16];
      assert_eq!(Primitive::encode(&bytes), "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz");
    }

    #[test]
    fn it_roundtrips_encode_decode() {
      let bytes = [
        0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF,
      ];
      let encoded = Primitive::encode(&bytes);
      let decoded = Primitive::decode(&encoded).unwrap();
      assert_eq!(bytes, decoded);
    }
  }

  mod from_id_for_value {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_converts_to_text_value() {
      let id: Primitive = "zyxwvutsrqponmlkzyxwvutsrqponmlk".parse().unwrap();
      let value = Value::from(id);
      assert_eq!(value, Value::from("zyxwvutsrqponmlkzyxwvutsrqponmlk".to_string()));
    }
  }

  mod from_str {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_parses_valid_id() {
      let id: Primitive = "zyxwvutsrqponmlkzyxwvutsrqponmlk".parse().unwrap();
      assert_eq!(id.to_string(), "zyxwvutsrqponmlkzyxwvutsrqponmlk");
    }

    #[test]
    fn it_rejects_digits() {
      let result = "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz1".parse::<Primitive>();
      assert!(result.is_err());
    }

    #[test]
    fn it_rejects_out_of_range_chars() {
      let result = "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzja".parse::<Primitive>();
      assert!(result.is_err());
    }

    #[test]
    fn it_rejects_too_long() {
      let result = "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz".parse::<Primitive>();
      assert!(result.is_err());
    }

    #[test]
    fn it_rejects_too_short() {
      let result = "zzz".parse::<Primitive>();
      assert!(result.is_err());
    }
  }

  mod new {
    use super::*;

    #[test]
    fn it_generates_valid_ids() {
      let id = Primitive::new();
      let s = id.to_string();
      assert_eq!(s.len(), 32);
      assert!(s.chars().all(|c| ('k'..='z').contains(&c)));
    }
  }

  mod validate_prefix {
    use super::*;

    #[test]
    fn it_accepts_full_id() {
      assert!(Primitive::validate_prefix("zyxwvutsrqponmlkzyxwvutsrqponmlk").is_ok());
    }

    #[test]
    fn it_accepts_short_prefix() {
      assert_eq!(Primitive::validate_prefix("zyxw").unwrap(), "zyxw");
    }

    #[test]
    fn it_accepts_single_char() {
      assert!(Primitive::validate_prefix("k").is_ok());
    }

    #[test]
    fn it_rejects_chars_below_range() {
      assert!(Primitive::validate_prefix("kkkj").is_err());
    }

    #[test]
    fn it_rejects_digits() {
      assert!(Primitive::validate_prefix("kkkk1").is_err());
    }

    #[test]
    fn it_rejects_dots() {
      assert!(Primitive::validate_prefix("..").is_err());
    }

    #[test]
    fn it_rejects_empty() {
      assert!(Primitive::validate_prefix("").is_err());
    }

    #[test]
    fn it_rejects_path_separator() {
      assert!(Primitive::validate_prefix("../etc").is_err());
    }

    #[test]
    fn it_rejects_too_long() {
      let long = "k".repeat(33);
      assert!(Primitive::validate_prefix(&long).is_err());
    }

    #[test]
    fn it_rejects_uppercase() {
      assert!(Primitive::validate_prefix("kkkK").is_err());
    }
  }

  mod display {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_encodes_all_ff_bytes_as_all_k() {
      let id = Primitive([0xFF; 16]);
      assert_eq!(id.to_string(), "kkkkkkkkkkkkkkkkkkkkkkkkkkkkkkkk");
    }

    #[test]
    fn it_formats_as_the_encoded_string() {
      let id: Primitive = "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz".parse().unwrap();
      assert_eq!(id.to_string(), "zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz");
    }
  }

  mod roundtrip {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_roundtrips_through_string() {
      let id = Primitive::new();
      let s = id.to_string();
      let parsed: Primitive = s.parse().unwrap();
      assert_eq!(id, parsed);
    }
  }

  mod short {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_first_eight_characters() {
      let id: Primitive = "zyxwvutsrqponmlkzyxwvutsrqponmlk".parse().unwrap();
      assert_eq!(id.short(), "zyxwvuts");
    }
  }
}
