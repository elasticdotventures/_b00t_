//! ISO standard type wrappers — LEI, ISO 4217 currency codes, and IFRS 9
//! financial instrument classification.
//!
//! Every ISO type validates on construction and supports Serde roundtrips
//! for JSONL evidence logging.

use serde::{Deserialize, Serialize};

// ── ISO 17442: Legal Entity Identifier (LEI) ────────────────────────────────

/// ISO 17442 Legal Entity Identifier (LEI) — a 20-character alphanumeric
/// code with an ISO 7064 Mod 97-10 check digit in the last two positions.
///
/// # Validation
/// - Must be exactly 20 alphanumeric characters (A-Z, 0-9)
/// - Uppercase only on construction
/// - Last two characters must satisfy the Luhn Mod N check
///
/// # Example
/// ```ignore
/// let lei = Lei::new("7LTWFZYICNSX8D621K86").unwrap();
/// assert_eq!(lei.as_str(), "7LTWFZYICNSX8D621K86");
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Lei(String);

impl Lei {
    /// Create a new LEI, validating the format and check digit.
    ///
    /// Returns `Err` if the input is the wrong length, contains invalid
    /// characters, or fails the check digit validation.
    pub fn new(raw: impl Into<String>) -> Result<Self, LeiError> {
        let raw = raw.into().to_uppercase();
        if raw.len() != 20 {
            return Err(LeiError::InvalidLength(raw.len()));
        }
        if !raw.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(LeiError::InvalidCharacters);
        }
        if !validate_lei_check_digit(&raw) {
            return Err(LeiError::CheckDigitFailed);
        }
        Ok(Self(raw))
    }

    /// Return the LEI as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Lei {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Errors that can occur when constructing an LEI.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LeiError {
    #[error("LEI must be 20 characters, got {0}")]
    InvalidLength(usize),

    #[error("LEI contains invalid characters (only A-Z, 0-9 allowed)")]
    InvalidCharacters,

    #[error("LEI check digit validation failed")]
    CheckDigitFailed,
}

/// Validate the ISO 17442 check digit per ISO 7064 Mod 97-10.
///
/// Algorithm:
/// 1. Convert each character of the 18-char body to its numeric value (0-9 for
///    digits, A=10 through Z=35 for letters).
/// 2. Process digit-by-digit: single-digit values (0-9) shift the accumulator
///    by 10; two-digit values (10-35) shift by 100. Each step applies mod 97.
/// 3. Append "00" to the body (ISO 7064 annex).
/// 4. Check digits = 98 - remainder (zero-padded to 2 digits).
fn validate_lei_check_digit(lei: &str) -> bool {
    let body = &lei[..18];
    let expected_check = &lei[18..20];

    /// Map alphanumeric char to its integer value (0-9, A=10, ..., Z=35).
    fn char_value(c: char) -> u32 {
        match c {
            '0'..='9' => c as u32 - '0' as u32,
            'A'..='Z' => c as u32 - 'A' as u32 + 10,
            _ => 0,
        }
    }

    // Process each character: letters (value >= 10) are two decimal digits
    // and require multiplying the accumulator by 100; digits need only 10.
    let mut remainder: u32 = 0;
    for c in body.chars() {
        let val = char_value(c);
        if val < 10 {
            remainder = (remainder * 10 + val) % 97;
        } else {
            remainder = (remainder * 100 + val) % 97;
        }
    }
    // Append "00" per ISO 7064 annex
    remainder = (remainder * 100) % 97;

    let check = 98u32.wrapping_sub(remainder) % 97;
    let check_str = format!("{:02}", check);

    check_str == expected_check
}

// ── ISO 4217: Currency codes ────────────────────────────────────────────────

/// ISO 4217 currency code — a 3-letter alphabetic code.
///
/// # Example
/// ```ignore
/// let aud = Iso4217::new("AUD").unwrap();
/// assert_eq!(aud.as_str(), "AUD");
/// assert!(Iso4217::new("BLARG").is_err());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Iso4217(String);

impl Iso4217 {
    /// Create a new ISO 4217 currency code. Must be exactly 3 uppercase
    /// ASCII letters.
    pub fn new(code: impl Into<String>) -> Result<Self, Iso4217Error> {
        let code = code.into().to_uppercase();
        if code.len() != 3 {
            return Err(Iso4217Error::InvalidLength(code.len()));
        }
        if !code.chars().all(|c| c.is_ascii_uppercase()) {
            return Err(Iso4217Error::InvalidCharacters);
        }
        Ok(Self(code))
    }

    /// Return the currency code as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Iso4217 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Errors constructing an Iso4217 code.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Iso4217Error {
    #[error("ISO 4217 code must be 3 characters, got {0}")]
    InvalidLength(usize),

    #[error("ISO 4217 code must be all uppercase ASCII letters (A-Z)")]
    InvalidCharacters,
}

// ── IFRS 9: Financial Instrument Classification ──────────────────────────────

/// IFRS 9 classification — how a financial instrument is measured for
/// accounting purposes (International Financial Reporting Standard 9).
///
/// | Variant          | IFRS 9 para | Measurement basis               |
/// |------------------|-------------|---------------------------------|
/// | `Fvpl`           | §4.1.4      | Fair Value through P&L          |
/// | `Fvoci`          | §4.1.2A     | Fair Value through OCI          |
/// | `AmortizedCost`  | §4.1.2      | Amortised cost (SPPI test pass) |
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum Ifrs9Classification {
    /// Fair Value Through Profit and Loss (§4.1.4)
    Fvpl,

    /// Fair Value Through Other Comprehensive Income (§4.1.2A)
    Fvoci,

    /// Amortised Cost — only if the SPPI (Solely Payments of Principal and
    /// Interest) test passes and the business model is "hold to collect"
    /// (§4.1.2)
    AmortizedCost,
}

impl Ifrs9Classification {
    /// Return the IFRS 9 section reference for this classification.
    pub fn ifrs_section(&self) -> &'static str {
        match self {
            Ifrs9Classification::Fvpl => "IFRS 9 §4.1.4",
            Ifrs9Classification::Fvoci => "IFRS 9 §4.1.2A",
            Ifrs9Classification::AmortizedCost => "IFRS 9 §4.1.2",
        }
    }

    /// Return a short label for serialization/logging.
    pub fn label(&self) -> &'static str {
        match self {
            Ifrs9Classification::Fvpl => "FVPL",
            Ifrs9Classification::Fvoci => "FVOCI",
            Ifrs9Classification::AmortizedCost => "AmortizedCost",
        }
    }
}

impl std::fmt::Display for Ifrs9Classification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label())
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── LEI tests ─────────────────────────────────────────────────────────

    #[test]
    fn lei_valid_real_world() {
        let lei = Lei::new("7LTWFZYICNSX8D621K86");
        assert!(lei.is_ok(), "Valid LEI should parse: {:?}", lei.err());
        let lei = lei.unwrap();
        assert_eq!(lei.as_str(), "7LTWFZYICNSX8D621K86");
    }

    #[test]
    fn lei_rejects_wrong_length() {
        let err = Lei::new("TOOSHORT").unwrap_err();
        assert!(matches!(err, LeiError::InvalidLength(8)));
    }

    #[test]
    fn lei_rejects_special_chars() {
        let err = Lei::new("5493001KJTIIG5Y0Y---").unwrap_err();
        assert!(matches!(err, LeiError::InvalidCharacters));
    }

    #[test]
    fn lei_lowercase_is_uppercased() {
        let lei = Lei::new("7ltwfzyicnsx8d621k86").unwrap();
        assert_eq!(lei.as_str(), "7LTWFZYICNSX8D621K86");
    }

    #[test]
    fn lei_display() {
        let lei = Lei::new("7LTWFZYICNSX8D621K86").unwrap();
        assert_eq!(lei.to_string(), "7LTWFZYICNSX8D621K86");
    }

    #[test]
    fn lei_serializes_as_string() {
        let lei = Lei::new("7LTWFZYICNSX8D621K86").unwrap();
        let json = serde_json::to_string(&lei).unwrap();
        assert_eq!(json, "\"7LTWFZYICNSX8D621K86\"");
        let back: Lei = serde_json::from_str(&json).unwrap();
        assert_eq!(lei, back);
    }

    #[test]
    fn lei_check_digit_rejects_bad() {
        // Change the last character to break check digit
        let err = Lei::new("7LTWFZYICNSX8D621K87").unwrap_err();
        assert!(matches!(err, LeiError::CheckDigitFailed));
    }

    // ── Iso4217 tests ─────────────────────────────────────────────────────

    #[test]
    fn iso4217_valid_codes() {
        for code in &["AUD", "USD", "EUR", "GBP", "SGD", "NZD", "JPY"] {
            let iso = Iso4217::new(*code).unwrap();
            assert_eq!(iso.as_str(), *code);
        }
    }

    #[test]
    fn iso4217_rejects_wrong_length() {
        assert!(Iso4217::new("AU").is_err());
        assert!(Iso4217::new("AUDD").is_err());
    }

    #[test]
    fn iso4217_rejects_digits() {
        assert!(Iso4217::new("A12").is_err());
    }

    #[test]
    fn iso4217_lowercase_uppercased() {
        let iso = Iso4217::new("aud").unwrap();
        assert_eq!(iso.as_str(), "AUD");
    }

    #[test]
    fn iso4217_serializes_as_string() {
        let iso = Iso4217::new("EUR").unwrap();
        let json = serde_json::to_string(&iso).unwrap();
        assert_eq!(json, "\"EUR\"");
        let back: Iso4217 = serde_json::from_str(&json).unwrap();
        assert_eq!(iso, back);
    }

    #[test]
    fn iso4217_display() {
        let iso = Iso4217::new("BTC").unwrap();
        assert_eq!(iso.to_string(), "BTC");
    }

    // ── Ifrs9Classification tests ─────────────────────────────────────────

    #[test]
    fn ifrs9_sections_are_correct() {
        assert_eq!(
            Ifrs9Classification::Fvpl.ifrs_section(),
            "IFRS 9 §4.1.4"
        );
        assert_eq!(
            Ifrs9Classification::Fvoci.ifrs_section(),
            "IFRS 9 §4.1.2A"
        );
        assert_eq!(
            Ifrs9Classification::AmortizedCost.ifrs_section(),
            "IFRS 9 §4.1.2"
        );
    }

    #[test]
    fn ifrs9_labels() {
        assert_eq!(Ifrs9Classification::Fvpl.label(), "FVPL");
        assert_eq!(Ifrs9Classification::Fvoci.label(), "FVOCI");
        assert_eq!(Ifrs9Classification::AmortizedCost.label(), "AmortizedCost");
    }

    #[test]
    fn ifrs9_all_three_variants_exist() {
        let all = vec![
            Ifrs9Classification::Fvpl,
            Ifrs9Classification::Fvoci,
            Ifrs9Classification::AmortizedCost,
        ];
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn ifrs9_roundtrips_json() {
        for variant in &[
            Ifrs9Classification::Fvpl,
            Ifrs9Classification::Fvoci,
            Ifrs9Classification::AmortizedCost,
        ] {
            let json = serde_json::to_string(variant).unwrap();
            let back: Ifrs9Classification = serde_json::from_str(&json).unwrap();
            assert_eq!(variant, &back);
        }
    }

    #[test]
    fn ifrs9_display() {
        assert_eq!(Ifrs9Classification::Fvpl.to_string(), "FVPL");
        assert_eq!(Ifrs9Classification::Fvoci.to_string(), "FVOCI");
        assert_eq!(Ifrs9Classification::AmortizedCost.to_string(), "AmortizedCost");
    }

    #[test]
    fn lei_another_real_world() {
        // Deutsche Bank London LEI
        let lei = Lei::new("7LTWFZYICNSX8D621K86").unwrap();
        assert_eq!(lei.as_str(), "7LTWFZYICNSX8D621K86");
    }

    #[test]
    fn iso4217_crypto_codes() {
        // ISO 4217 doesn't include crypto codes officially, but we accept any
        // 3-uppercase-letter code per the validation logic
        let btc = Iso4217::new("BTC").unwrap();
        assert_eq!(btc.as_str(), "BTC");
        let eth = Iso4217::new("ETH").unwrap();
        assert_eq!(eth.as_str(), "ETH");
    }
}
