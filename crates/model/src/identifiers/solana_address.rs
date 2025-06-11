// -------------------------------------------------------------------------------------------------
//  Copyright (C) 2015-2025 Nautech Systems Pty Ltd. All rights reserved.
//  https://nautechsystems.io
//
//  Licensed under the GNU Lesser General Public License Version 3.0 (the "License");
//  You may not use this file except in compliance with the License.
//  You may obtain a copy of the License at https://www.gnu.org/licenses/lgpl-3.0.en.html
//
//  Unless required by applicable law or agreed to in writing, software
//  distributed under the License is distributed on an "AS IS" BASIS,
//  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//  See the License for the specific language governing permissions and
//  limitations under the License.
// -------------------------------------------------------------------------------------------------

//! Represents a valid Solana address.

use std::{
    ffi::CStr,
    fmt::{Debug, Display, Formatter},
    // Hash is derived
};

use nautilus_core::correctness::{
    check_predicate_false, check_predicate_true, check_slice_not_empty, FAILED,
};
use serde::{de, Deserialize, Deserializer, Serialize, Serializer};

/// The maximum number of data characters for a `SolanaAddress` string value.
/// Solana addresses are typically 32-44 characters long.
pub const SOLANA_ADDRESS_MAX_CHARS: usize = 44;

/// The total buffer length for a `SolanaAddress` byte array (including null terminator).
pub const SOLANA_ADDRESS_BUFFER_LEN: usize = SOLANA_ADDRESS_MAX_CHARS + 1;

const BASE58_CHARS: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";

#[inline]
fn is_valid_base58_char(byte: u8) -> bool {
    BASE58_CHARS.contains(&byte)
}

/// Represents a valid Solana address.
/// A Solana address is typically a Base58 encoded public key of 32-44 characters.
#[repr(C)]
#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(
    feature = "python",
    pyo3::pyclass(module = "nautilus_trader.core.nautilus_pyo3.model")
)]
pub struct SolanaAddress {
    /// The Solana address value as a fixed-length C string byte array (includes null terminator).
    pub(crate) value: [u8; SOLANA_ADDRESS_BUFFER_LEN],
}

impl SolanaAddress {
    /// Creates a new [`SolanaAddress`] instance with correctness checking.
    ///
    /// Maximum data length is 44 characters.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - `value` is an invalid string (e.g., is empty, contains non-Base58 characters).
    /// - `value` data length exceeds 44 characters.
    /// - `value` length is otherwise incompatible with the internal buffer.
    ///
    /// # Notes
    ///
    /// PyO3 requires a `Result` type for proper error handling and stacktrace printing in Python.
    pub fn new_checked<T: AsRef<str>>(value: T) -> anyhow::Result<Self> {
        Self::from_bytes(value.as_ref().as_bytes())
    }

    /// Creates a new [`SolanaAddress`] instance.
    ///
    /// Maximum data length is 44 characters.
    ///
    /// # Panics
    ///
    /// This function panics if `value` does not represent a valid Solana address
    /// according to the criteria of [`SolanaAddress::new_checked`].
    pub fn new<T: AsRef<str>>(value: T) -> Self {
        Self::new_checked(value).expect(FAILED)
    }

    /// Creates a new [`SolanaAddress`] instance from a byte slice.
    ///
    /// The byte slice can be null-terminated or not. Validation is performed on characters
    /// before the first null byte (if any). The effective data length must not exceed 44 characters.
    ///
    /// # Errors
    ///
    /// Returns an error if `bytes` is empty, effectively empty (e.g. `b"\0"`),
    /// contains non-Base58 characters in its data part, or if its length is
    /// incompatible with storage requirements (e.g. data part too long, or overall
    /// length too long without proper null termination).
    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        check_slice_not_empty(bytes, "bytes")?;

        let mut effective_len = 0;
        // let mut first_null_pos: Option<usize> = None; // Removed
        for (i, &byte) in bytes.iter().enumerate() {
            if byte == 0 {
                // first_null_pos = Some(i); // Removed
                break;
            }
            if !is_valid_base58_char(byte) {
                return Err(anyhow::anyhow!(
                    "'bytes' contains non-Base58 characters (at index {})",
                    i
                ));
            }
            effective_len += 1;
        }

        check_predicate_false(
            effective_len == 0 && bytes.len() > 0,
            "'bytes' is effectively empty or contains only null bytes",
        )?;

        check_predicate_true(
            effective_len <= SOLANA_ADDRESS_MAX_CHARS,
            format!(
                "'bytes' effective data length ({}) exceeds max chars ({})",
                effective_len, SOLANA_ADDRESS_MAX_CHARS
            )
            .as_str(),
        )?;

        let is_length_compatible_with_buffer = bytes.len() <= SOLANA_ADDRESS_MAX_CHARS
            || (bytes.len() == SOLANA_ADDRESS_BUFFER_LEN
                && bytes.last().map_or(false, |&b| b == 0));

        check_predicate_true(
            is_length_compatible_with_buffer,
            format!(
                "'bytes' length ({}) is incompatible with storage (max data {} chars, buffer {})",
                bytes.len(),
                SOLANA_ADDRESS_MAX_CHARS,
                SOLANA_ADDRESS_BUFFER_LEN
            )
            .as_str(),
        )?;

        let mut buf = [0; SOLANA_ADDRESS_BUFFER_LEN];
        let len_to_copy = std::cmp::min(bytes.len(), SOLANA_ADDRESS_BUFFER_LEN);
        buf[..len_to_copy].copy_from_slice(&bytes[..len_to_copy]);

        Ok(Self { value: buf })
    }

    /// Returns a C string slice from the Solana address value.
    ///
    /// # Panics
    ///
    /// Panics if the stored byte array is not a valid C string up to the first NUL.
    /// This should not happen if instances are created via provided constructors.
    #[must_use]
    pub fn as_cstr(&self) -> &CStr {
        // SAFETY: Unwrap safe as constructors ensure valid C strings are stored.
        // Uses `from_bytes_until_nul` because the value array may be padded with NULs
        // if the original string was shorter than SOLANA_ADDRESS_MAX_CHARS.
        CStr::from_bytes_until_nul(&self.value).unwrap()
    }

    /// Returns the Solana address as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        // SAFETY: Unwrap safe as Base58 characters are valid UTF-8, and constructors ensure this.
        self.as_cstr().to_str().unwrap()
    }
}

impl Debug for SolanaAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "SolanaAddress('{}')", self.as_str())
    }
}

impl Display for SolanaAddress {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Serialize for SolanaAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for SolanaAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SolanaAddressVisitor;

        impl<'de> de::Visitor<'de> for SolanaAddressVisitor {
            type Value = SolanaAddress;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                formatter.write_str(
                    "a Base58 encoded Solana address string, max 44 characters",
                )
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                SolanaAddress::new_checked(value).map_err(de::Error::custom)
            }
        }
        deserializer.deserialize_str(SolanaAddressVisitor)
    }
}

////////////////////////////////////////////////////////////////////////////////
// Tests
////////////////////////////////////////////////////////////////////////////////
#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // Valid examples
    const VALID_ADDR_S1: &str = "So11111111111111111111111111111111111111112"; // 44 chars
    const VALID_ADDR_S2: &str = "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s"; // 44 chars
    const VALID_ADDR_S3: &str = "Vote111111111111111111111111111111111111111"; // 43 chars
    const VALID_ADDR_SHORT: &str = "12345"; // 5 chars

    #[rstest]
    #[case(VALID_ADDR_S1)]
    #[case(VALID_ADDR_S2)]
    #[case(VALID_ADDR_S3)]
    #[case(VALID_ADDR_SHORT)]
    fn test_new_checked_valid(#[case] val: &str) {
        let addr = SolanaAddress::new_checked(val);
        assert!(addr.is_ok());
        assert_eq!(addr.unwrap().as_str(), val);
    }

    #[rstest]
    #[case(VALID_ADDR_S1)]
    #[case(VALID_ADDR_S2)]
    #[case(VALID_ADDR_S3)]
    #[case(VALID_ADDR_SHORT)]
    fn test_new_valid(#[case] val: &str) {
        let addr = SolanaAddress::new(val);
        assert_eq!(addr.as_str(), val);
    }

    #[test]
    fn test_new_checked_empty_string() {
        assert!(SolanaAddress::new_checked("").is_err());
    }

    #[test]
    #[should_panic(expected = "Condition failed: the 'bytes' slice `&[u8]` was empty")]
    fn test_new_panics_empty_string() {
        SolanaAddress::new("");
    }

    #[test]
    fn test_new_checked_effectively_empty_null_byte() {
        let err = SolanaAddress::new_checked("\0").unwrap_err();
        assert!(format!("{:?}", err)
            .contains("'bytes' is effectively empty or contains only null bytes"));
    }

    #[test]
    #[should_panic(expected = "'bytes' is effectively empty or contains only null bytes")]
    fn test_new_panics_effectively_empty_null_byte() {
        SolanaAddress::new("\0");
    }

    #[rstest]
    #[case("12345!", "non-Base58 characters (at index 5)")] // Invalid char '!'
    #[case("123\0ABC", "non-Base58 characters (at index 3)")] // Null in middle, effectively non-base58 for that part
    #[case("Test_Addr", "non-Base58 characters (at index 4)")] // Underscore
    fn test_new_checked_invalid_chars(#[case] val: &str, #[case] expected_err_msg: &str) {
        let result = SolanaAddress::new_checked(val);
        assert!(result.is_err());
        assert!(
            format!("{:?}", result.unwrap_err()).contains(expected_err_msg),
            "Error message mismatch for input: {}",
            val
        );
    }

    #[rstest]
    #[case("12345!")]
    #[case("Test_Addr")]
    fn test_new_panics_invalid_chars(#[case] val: &str) {
        // The exact panic message can vary based on which check fails first (content or length related to buffer)
        // For `from_bytes`, it's usually the content check.
        let result = std::panic::catch_unwind(|| SolanaAddress::new(val));
        assert!(result.is_err());
    }

    #[test]
    fn test_new_checked_too_long() {
        let too_long_str = "1".repeat(SOLANA_ADDRESS_MAX_CHARS + 1); // 45 chars
        let result = SolanaAddress::new_checked(&too_long_str);
        assert!(result.is_err());
        assert!(format!("{:?}", result.unwrap_err()).contains(
            &format!(
                "'bytes' effective data length ({}) exceeds max chars ({})",
                SOLANA_ADDRESS_MAX_CHARS + 1,
                SOLANA_ADDRESS_MAX_CHARS
            )
        ));
    }
    
    #[test]
    #[should_panic] // Expected panic message is complex due to formatting, just check for panic
    fn test_new_panics_too_long() {
        let too_long_str = "1".repeat(SOLANA_ADDRESS_MAX_CHARS + 1);
        SolanaAddress::new(&too_long_str);
    }

    #[test]
    fn test_from_bytes_valid_with_null_terminator() {
        let bytes_with_null = format!("{}\0", VALID_ADDR_SHORT).into_bytes();
        let addr = SolanaAddress::from_bytes(&bytes_with_null).unwrap();
        assert_eq!(addr.as_str(), VALID_ADDR_SHORT);
    }

    #[test]
    fn test_from_bytes_max_len_data_with_null_terminator() {
        let mut val = "1".repeat(SOLANA_ADDRESS_MAX_CHARS);
        let bytes_val = val.clone().into_bytes();
        let addr1 = SolanaAddress::from_bytes(&bytes_val).unwrap();
        assert_eq!(addr1.as_str(), val);
        
        val.push('\0'); // Now 45 bytes total
        let bytes_with_null = val.into_bytes();
        assert_eq!(bytes_with_null.len(), SOLANA_ADDRESS_BUFFER_LEN);
        let addr2 = SolanaAddress::from_bytes(&bytes_with_null).unwrap();
        assert_eq!(addr2.as_str(), "1".repeat(SOLANA_ADDRESS_MAX_CHARS));
    }

    #[test]
    fn test_from_bytes_too_long_no_null_terminator_at_buffer_len() {
        // 45 chars, all data. Should fail length compatibility for buffer.
        let bytes = "1".repeat(SOLANA_ADDRESS_BUFFER_LEN).into_bytes();
        let result = SolanaAddress::from_bytes(&bytes);
        assert!(result.is_err());
        assert!(format!("{:?}", result.unwrap_err()).contains(
            &format!(
                "'bytes' length ({}) is incompatible with storage",
                SOLANA_ADDRESS_BUFFER_LEN
            )
        ));
    }
    
    #[test]
    fn test_from_bytes_too_long_overall() {
        // 46 chars. Should fail length compatibility for buffer.
        let bytes = "1".repeat(SOLANA_ADDRESS_BUFFER_LEN + 1).into_bytes();
        let result = SolanaAddress::from_bytes(&bytes);
        assert!(result.is_err());
         assert!(format!("{:?}", result.unwrap_err()).contains(
            &format!(
                "'bytes' length ({}) is incompatible with storage",
                SOLANA_ADDRESS_BUFFER_LEN + 1
            )
        ));
    }


    #[rstest]
    fn test_string_reprs() {
        let addr = SolanaAddress::new(VALID_ADDR_S1);
        assert_eq!(addr.as_str(), VALID_ADDR_S1);
        assert_eq!(addr.to_string(), VALID_ADDR_S1); // Display trait
        assert_eq!(
            format!("{:?}", addr),
            format!("SolanaAddress('{}')", VALID_ADDR_S1)
        ); // Debug trait
    }

    #[test]
    fn test_as_cstr() {
        let addr = SolanaAddress::new(VALID_ADDR_SHORT);
        let cstr = addr.as_cstr();
        assert_eq!(cstr.to_str().unwrap(), VALID_ADDR_SHORT);
    }

    #[test]
    fn test_equality_and_ordering() {
        let addr1 = SolanaAddress::new(VALID_ADDR_S3); // Vote...
        let addr2 = SolanaAddress::new(VALID_ADDR_S3);
        let addr3 = SolanaAddress::new(VALID_ADDR_S1); // So11...

        assert_eq!(addr1, addr2);
        assert_ne!(addr1, addr3);
        assert!(addr3 < addr1); // "So11..." < "Vote..." lexicographically
    }

    #[test]
    fn test_serialization_deserialization() {
        let addr = SolanaAddress::new(VALID_ADDR_S2);
        let serialized = serde_json::to_string(&addr).unwrap();
        assert_eq!(serialized, format!("\"{}\"", VALID_ADDR_S2));

        let deserialized: SolanaAddress = serde_json::from_str(&serialized).unwrap();
        assert_eq!(addr, deserialized);
    }

    #[test]
    fn test_deserialize_invalid_string() {
        let invalid_json = "\"123!\""; // Contains non-Base58
        let result: Result<SolanaAddress, _> = serde_json::from_str(invalid_json);
        assert!(result.is_err());
        assert!(format!("{:?}", result.unwrap_err()).contains("non-Base58 characters"));
    }

    #[test]
    fn test_deserialize_too_long_string() {
        let too_long_addr = "1".repeat(SOLANA_ADDRESS_MAX_CHARS + 1);
        let invalid_json = format!("\"{}\"", too_long_addr);
        let result: Result<SolanaAddress, _> = serde_json::from_str(&invalid_json);
        assert!(result.is_err());
        assert!(format!("{:?}", result.unwrap_err()).contains("exceeds max chars"));
    }
}