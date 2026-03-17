use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

// ---------------------------------------------------------------------------
// Congress
// ---------------------------------------------------------------------------

/// A validated Congress number (1..=120).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Congress(u32);

impl Congress {
    /// Create a new `Congress`, returning an error string if out of range.
    pub fn new(n: u32) -> Result<Self, String> {
        if (1..=120).contains(&n) {
            Ok(Self(n))
        } else {
            Err(format!("Congress number {n} out of valid range 1..=120"))
        }
    }

    /// Return the inner `u32`.
    pub fn number(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for Congress {
    type Error = String;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl fmt::Display for Congress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let n = self.0;
        let suffix = match n % 100 {
            11..=13 => "th",
            _ => match n % 10 {
                1 => "st",
                2 => "nd",
                3 => "rd",
                _ => "th",
            },
        };
        write!(f, "{n}{suffix}")
    }
}

impl Serialize for Congress {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Congress {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let n = u32::deserialize(deserializer)?;
        Congress::new(n).map_err(serde::de::Error::custom)
    }
}

// ---------------------------------------------------------------------------
// Chamber
// ---------------------------------------------------------------------------

/// Congressional chamber.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Chamber {
    House,
    Senate,
}

impl fmt::Display for Chamber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Chamber::House => write!(f, "House"),
            Chamber::Senate => write!(f, "Senate"),
        }
    }
}

// ---------------------------------------------------------------------------
// BillType
// ---------------------------------------------------------------------------

/// The type / form of a bill or resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BillType {
    Hr,
    S,
    Hjres,
    Sjres,
    Hconres,
    Sconres,
    Hres,
    Sres,
}

impl BillType {
    /// Whether this bill type can carry appropriations.
    pub fn can_appropriate(self) -> bool {
        matches!(self, Self::Hr | Self::S | Self::Hjres | Self::Sjres)
    }

    /// The chamber this bill type originates in.
    pub fn chamber(self) -> Chamber {
        match self {
            Self::Hr | Self::Hjres | Self::Hconres | Self::Hres => Chamber::House,
            Self::S | Self::Sjres | Self::Sconres | Self::Sres => Chamber::Senate,
        }
    }

    /// The slug used in Congress.gov API paths (lowercase).
    pub fn api_slug(self) -> &'static str {
        match self {
            Self::Hr => "hr",
            Self::S => "s",
            Self::Hjres => "hjres",
            Self::Sjres => "sjres",
            Self::Hconres => "hconres",
            Self::Sconres => "sconres",
            Self::Hres => "hres",
            Self::Sres => "sres",
        }
    }

    /// Human-readable short label (e.g. "H.R.", "S.", "H.J.Res.").
    pub fn label(self) -> &'static str {
        match self {
            Self::Hr => "H.R.",
            Self::S => "S.",
            Self::Hjres => "H.J.Res.",
            Self::Sjres => "S.J.Res.",
            Self::Hconres => "H.Con.Res.",
            Self::Sconres => "S.Con.Res.",
            Self::Hres => "H.Res.",
            Self::Sres => "S.Res.",
        }
    }

    /// Parse from a case-insensitive slug string.
    pub fn from_slug(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "hr" => Some(Self::Hr),
            "s" => Some(Self::S),
            "hjres" => Some(Self::Hjres),
            "sjres" => Some(Self::Sjres),
            "hconres" => Some(Self::Hconres),
            "sconres" => Some(Self::Sconres),
            "hres" => Some(Self::Hres),
            "sres" => Some(Self::Sres),
            _ => None,
        }
    }
}

impl FromStr for BillType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::from_slug(s).ok_or_else(|| format!("unknown bill type: {s}"))
    }
}

impl fmt::Display for BillType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.label())
    }
}

impl Serialize for BillType {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.api_slug())
    }
}

impl<'de> Deserialize<'de> for BillType {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        BillType::from_slug(&s)
            .ok_or_else(|| serde::de::Error::custom(format!("unknown bill type: {s}")))
    }
}

// ---------------------------------------------------------------------------
// BillId
// ---------------------------------------------------------------------------

/// Unique identifier for a bill: congress + type + number.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BillId {
    pub congress: Congress,
    pub bill_type: BillType,
    pub number: u32,
}

impl BillId {
    /// Create a new `BillId`.
    pub fn new(congress: Congress, bill_type: BillType, number: u32) -> Self {
        Self {
            congress,
            bill_type,
            number,
        }
    }

    /// Build the API path segment, e.g. `"119/hr/1"`.
    pub fn api_path(&self) -> String {
        format!(
            "{}/{}/{}",
            self.congress.number(),
            self.bill_type.api_slug(),
            self.number
        )
    }
}

impl fmt::Display for BillId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {}, {} Congress",
            self.bill_type, self.number, self.congress
        )
    }
}

// ---------------------------------------------------------------------------
// VersionCode
// ---------------------------------------------------------------------------

/// A bill-text version code (e.g. "enr", "ih", "eh", "rfs", etc.).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VersionCode(pub String);

impl VersionCode {
    pub fn new(code: impl Into<String>) -> Self {
        Self(code.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for VersionCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn congress_valid_range() {
        assert!(Congress::new(1).is_ok());
        assert!(Congress::new(119).is_ok());
        assert!(Congress::new(120).is_ok());
        assert!(Congress::new(0).is_err());
        assert!(Congress::new(121).is_err());
    }

    #[test]
    fn congress_display() {
        assert_eq!(Congress::new(1).unwrap().to_string(), "1st");
        assert_eq!(Congress::new(2).unwrap().to_string(), "2nd");
        assert_eq!(Congress::new(3).unwrap().to_string(), "3rd");
        assert_eq!(Congress::new(4).unwrap().to_string(), "4th");
        assert_eq!(Congress::new(11).unwrap().to_string(), "11th");
        assert_eq!(Congress::new(12).unwrap().to_string(), "12th");
        assert_eq!(Congress::new(13).unwrap().to_string(), "13th");
        assert_eq!(Congress::new(21).unwrap().to_string(), "21st");
        assert_eq!(Congress::new(119).unwrap().to_string(), "119th");
    }

    #[test]
    fn congress_serde_roundtrip() {
        let c = Congress::new(119).unwrap();
        let json = serde_json::to_string(&c).unwrap();
        assert_eq!(json, "119");
        let back: Congress = serde_json::from_str(&json).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn bill_type_slug_roundtrip() {
        for bt in [
            BillType::Hr,
            BillType::S,
            BillType::Hjres,
            BillType::Sjres,
            BillType::Hconres,
            BillType::Sconres,
            BillType::Hres,
            BillType::Sres,
        ] {
            assert_eq!(BillType::from_slug(bt.api_slug()), Some(bt));
        }
    }

    #[test]
    fn bill_type_deserialize_case_insensitive() {
        let bt: BillType = serde_json::from_str(r#""HR""#).unwrap();
        assert_eq!(bt, BillType::Hr);
        let bt: BillType = serde_json::from_str(r#""hr""#).unwrap();
        assert_eq!(bt, BillType::Hr);
        let bt: BillType = serde_json::from_str(r#""HJRES""#).unwrap();
        assert_eq!(bt, BillType::Hjres);
    }

    #[test]
    fn bill_type_can_appropriate() {
        assert!(BillType::Hr.can_appropriate());
        assert!(BillType::S.can_appropriate());
        assert!(BillType::Hjres.can_appropriate());
        assert!(BillType::Sjres.can_appropriate());
        assert!(!BillType::Hconres.can_appropriate());
        assert!(!BillType::Sconres.can_appropriate());
        assert!(!BillType::Hres.can_appropriate());
        assert!(!BillType::Sres.can_appropriate());
    }

    #[test]
    fn bill_id_display_and_api_path() {
        let id = BillId {
            congress: Congress::new(119).unwrap(),
            bill_type: BillType::Hr,
            number: 1,
        };
        assert_eq!(id.to_string(), "H.R. 1, 119th Congress");
        assert_eq!(id.api_path(), "119/hr/1");
    }

    #[test]
    fn version_code_basics() {
        let vc = VersionCode::new("enr");
        assert_eq!(vc.as_str(), "enr");
        assert_eq!(vc.to_string(), "enr");
    }
}
