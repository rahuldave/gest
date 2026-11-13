//! The priority of a task.

use std::{
  fmt::{self, Display, Formatter},
  str::FromStr,
};

use serde::{Deserialize, Serialize};

/// Relative importance assigned to a task.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "lowercase")]
#[repr(u8)]
pub enum Primitive {
  /// Must be addressed immediately; blocks other work.
  Critical = 0,
  /// Important but not a blocker.
  High = 1,
  /// Should be done eventually.
  Low = 3,
  /// Nice to have; defer if needed.
  Lowest = 4,
  /// Default importance for most work.
  Medium = 2,
}

impl Primitive {
  /// All priority variants in rank order, from highest to lowest.
  pub const ALL: &'static [Primitive] = &[
    Primitive::Critical,
    Primitive::High,
    Primitive::Medium,
    Primitive::Low,
    Primitive::Lowest,
  ];
}

impl Display for Primitive {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      Self::Critical => f.write_str("critical"),
      Self::High => f.write_str("high"),
      Self::Low => f.write_str("low"),
      Self::Lowest => f.write_str("lowest"),
      Self::Medium => f.write_str("medium"),
    }
  }
}

impl From<Primitive> for u8 {
  fn from(value: Primitive) -> Self {
    value as u8
  }
}

impl FromStr for Primitive {
  type Err = String;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    match s.to_lowercase().as_str() {
      "critical" => Ok(Self::Critical),
      "high" => Ok(Self::High),
      "low" => Ok(Self::Low),
      "lowest" => Ok(Self::Lowest),
      "medium" => Ok(Self::Medium),
      other => Err(format!("invalid priority: {other}")),
    }
  }
}

impl TryFrom<u8> for Primitive {
  type Error = String;

  fn try_from(value: u8) -> Result<Self, Self::Error> {
    match value {
      0 => Ok(Self::Critical),
      1 => Ok(Self::High),
      2 => Ok(Self::Medium),
      3 => Ok(Self::Low),
      4 => Ok(Self::Lowest),
      other => Err(format!("invalid priority value: {other}")),
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  mod all {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_contains_every_variant_in_rank_order() {
      assert_eq!(
        Primitive::ALL,
        &[
          Primitive::Critical,
          Primitive::High,
          Primitive::Medium,
          Primitive::Low,
          Primitive::Lowest,
        ]
      );
    }
  }

  mod display {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_critical() {
      assert_eq!(Primitive::Critical.to_string(), "critical");
    }

    #[test]
    fn it_formats_high() {
      assert_eq!(Primitive::High.to_string(), "high");
    }

    #[test]
    fn it_formats_low() {
      assert_eq!(Primitive::Low.to_string(), "low");
    }

    #[test]
    fn it_formats_lowest() {
      assert_eq!(Primitive::Lowest.to_string(), "lowest");
    }

    #[test]
    fn it_formats_medium() {
      assert_eq!(Primitive::Medium.to_string(), "medium");
    }
  }

  mod from_str {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_parses_each_label() {
      assert_eq!("critical".parse::<Primitive>().unwrap(), Primitive::Critical);
      assert_eq!("high".parse::<Primitive>().unwrap(), Primitive::High);
      assert_eq!("medium".parse::<Primitive>().unwrap(), Primitive::Medium);
      assert_eq!("low".parse::<Primitive>().unwrap(), Primitive::Low);
      assert_eq!("lowest".parse::<Primitive>().unwrap(), Primitive::Lowest);
    }

    #[test]
    fn it_parses_case_insensitively() {
      assert_eq!("CRITICAL".parse::<Primitive>().unwrap(), Primitive::Critical);
      assert_eq!("Critical".parse::<Primitive>().unwrap(), Primitive::Critical);
      assert_eq!("critical".parse::<Primitive>().unwrap(), Primitive::Critical);
    }

    #[test]
    fn it_rejects_an_empty_string() {
      assert!("".parse::<Primitive>().is_err());
    }

    #[test]
    fn it_rejects_unknown_labels() {
      let err = "urgent".parse::<Primitive>().unwrap_err();
      assert!(err.contains("urgent"));
    }

    #[test]
    fn it_round_trips_through_string() {
      for priority in Primitive::ALL {
        let rendered = priority.to_string();
        assert_eq!(rendered.parse::<Primitive>().unwrap(), *priority);
      }
    }
  }

  mod try_from_u8 {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_accepts_values_in_range() {
      assert_eq!(Primitive::try_from(0u8).unwrap(), Primitive::Critical);
      assert_eq!(Primitive::try_from(1u8).unwrap(), Primitive::High);
      assert_eq!(Primitive::try_from(2u8).unwrap(), Primitive::Medium);
      assert_eq!(Primitive::try_from(3u8).unwrap(), Primitive::Low);
      assert_eq!(Primitive::try_from(4u8).unwrap(), Primitive::Lowest);
    }

    #[test]
    fn it_rejects_the_max_u8() {
      assert!(Primitive::try_from(255u8).is_err());
    }

    #[test]
    fn it_rejects_values_just_out_of_range() {
      assert!(Primitive::try_from(5u8).is_err());
    }

    #[test]
    fn it_round_trips_through_u8() {
      for priority in Primitive::ALL {
        let byte: u8 = (*priority).into();
        assert_eq!(Primitive::try_from(byte).unwrap(), *priority);
      }
    }
  }
}
