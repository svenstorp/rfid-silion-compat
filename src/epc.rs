//! EPC helper types for common GS1 schemes.
//!
//! This module is intended for application developers who want to work with
//! RFID identifiers without memorizing the full GS1 EPC standard.
//!
//! # Quick Start
//!
//! 1. Create an [`EpcValue`] from raw bytes read from a tag.
//! 2. Decode to a typed struct when the header matches a supported scheme.
//! 3. Work with strongly typed fields and create URIs as needed.
//!
//! Supported schemes in this module:
//! - [`Sgtin96`] (header `0x30`)
//! - [`Giai96`] (header `0x34`)
//!
//! # Important Terminology
//!
//! - `EPC`: The binary identifier stored on tag memory (usually EPC bank).
//! - `Tag URI`: URI form that includes a filter value, for example
//!   `urn:epc:tag:sgtin-96:3.0614141.8123456.6789`.
//! - `Pure Identity URI`: URI form without filter value, for example
//!   `urn:epc:id:sgtin:0614141.8123456.6789`.
//!
//! # How To Use Filter Bits
//!
//! Both [`Sgtin96`] and [`Giai96`] contain a 3-bit `filter` field (`0..=7`).
//! The filter does not change the core identity itself; it is a hint that helps
//! readers or applications quickly distinguish "classes" of tagged objects.
//!
//! Practical guidance:
//! - Choose one filter value per operational category in your system.
//! - Keep it stable over time so downstream systems can rely on it.
//! - Document your mapping in your app (for example in config/docs).
//! - Use the same mapping in both writer and reader software.
//!
//! Example mapping for sports timing:
//! - `1`: race bibs
//! - `2`: staff badges
//! - `3`: equipment tags
//!
//! Notes:
//! - Filter values are scheme-defined as a 3-bit field, but business meaning
//!   depends on your application and operational profile.
//! - In Tag URI form, filter is the first numeric component after the scheme.
//! - In Pure Identity URI form (`urn:epc:id:*`), filter is intentionally omitted.
//!
//! # Example: Decode Depending on Header
//!
//! ```
//! use rfid_silion_compat::EpcValue;
//!
//! let epc = EpcValue::from_slice(&[0x30, 0x74, 0x4B, 0x5A, 0x1C, 0xA0, 0x10, 0x01, 0x00, 0x00, 0x1A, 0x85])?;
//!
//! if let Ok(sgtin) = epc.decode_sgtin96() {
//!     println!("SGTIN company prefix: {}", sgtin.company_prefix);
//! } else if let Ok(giai) = epc.decode_giai96() {
//!     println!("GIAI asset ref: {}", giai.individual_asset_reference);
//! } else {
//!     println!("Unsupported EPC header: {}", epc.to_hex());
//! }
//! # Ok::<(), rfid_silion_compat::ProtocolError>(())
//! ```
//!
//! # SGTIN-96 Field Widths
//!
//! SGTIN-96 is exactly 96 bits (12 bytes), laid out as:
//! - Header: 8 bits (`0x30`)
//! - Filter: 3 bits
//! - Partition: 3 bits
//! - Company Prefix: variable bits based on partition
//! - Item Reference: variable bits based on partition
//! - Serial: 38 bits
//!
//! Partition mapping used by [`Sgtin96`]:
//! - `0`: company 40 bits (12 digits), item 4 bits (1 digit)
//! - `1`: company 37 bits (11 digits), item 7 bits (2 digits)
//! - `2`: company 34 bits (10 digits), item 10 bits (3 digits)
//! - `3`: company 30 bits (9 digits), item 14 bits (4 digits)
//! - `4`: company 27 bits (8 digits), item 17 bits (5 digits)
//! - `5`: company 24 bits (7 digits), item 20 bits (6 digits)
//! - `6`: company 20 bits (6 digits), item 24 bits (7 digits)
//!
//! # GIAI-96 Field Widths
//!
//! GIAI-96 is exactly 96 bits (12 bytes), laid out as:
//! - Header: 8 bits (`0x34`)
//! - Filter: 3 bits
//! - Partition: 3 bits
//! - Company Prefix: variable bits based on partition
//! - Individual Asset Reference: remaining variable bits
//!
//! Partition mapping used by [`Giai96`]:
//! - `0`: company 40 bits (12 digits), asset 42 bits
//! - `1`: company 37 bits (11 digits), asset 45 bits
//! - `2`: company 34 bits (10 digits), asset 48 bits
//! - `3`: company 30 bits (9 digits), asset 52 bits
//! - `4`: company 27 bits (8 digits), asset 55 bits
//! - `5`: company 24 bits (7 digits), asset 58 bits
//! - `6`: company 20 bits (6 digits), asset 62 bits
//!
//! # Example: Build and Read Back SGTIN-96
//!
//! ```
//! use rfid_silion_compat::{EpcValue, Sgtin96};
//!
//! let fields = Sgtin96 {
//!     filter: 3,
//!     partition: 6,
//!     company_prefix: 614141,
//!     item_reference: 8123456,
//!     serial: 6789,
//! };
//!
//! let epc = EpcValue::from_schema(fields)?;
//! let decoded = epc.decode_sgtin96()?;
//! assert_eq!(decoded, fields);
//! assert_eq!(decoded.to_tag_uri()?, "urn:epc:tag:sgtin-96:3.614141.8123456.6789");
//! # Ok::<(), rfid_silion_compat::ProtocolError>(())
//! ```
//!
//! # Example: Build and Read Back GIAI-96
//!
//! ```
//! use rfid_silion_compat::{EpcValue, Giai96};
//!
//! let fields = Giai96 {
//!     filter: 1,
//!     partition: 6,
//!     company_prefix: 614141,
//!     individual_asset_reference: 123456789,
//! };
//!
//! let epc = EpcValue::from_schema(fields)?;
//! let decoded = epc.decode_giai96()?;
//! assert_eq!(decoded, fields);
//! assert_eq!(decoded.to_pure_identity_uri()?, "urn:epc:id:giai:614141.123456789");
//! # Ok::<(), rfid_silion_compat::ProtocolError>(())
//! ```
//!
//! # Generic URI Helpers
//!
//! [`EpcValue`] also includes generic URI helpers that are useful when you need
//! a custom URI representation:
//! - [`EpcValue::tag_uri_custom`]
//! - [`EpcValue::parse_tag_uri_custom`]
//! - [`EpcValue::tag_uri_to_pure_identity_uri`]
//! - [`EpcValue::tag_uri_with_epc`]
//!
//! These are not scheme-specific validators; they are convenience helpers.

use crate::error::ProtocolError;
use crate::parsers::TagEpcAndMetaData;

/// High-level EPC value wrapper used by reader APIs and application logic.
///
/// This type stores raw EPC bytes and provides convenience helpers for
/// formatting, URI construction, and scheme-specific encoding/decoding.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "web-serial", derive(serde::Serialize, serde::Deserialize))]
pub struct EpcValue {
    bytes: Vec<u8>,
}

/// Trait implemented by typed EPC schema structs that can be encoded to bytes.
///
/// Implemented by [`Sgtin96`] and [`Giai96`].
pub trait EpcSchema {
    /// Encode this schema value into an [`EpcValue`].
    fn encode(self) -> Result<EpcValue, ProtocolError>;
}

/// Decoded field values for an SGTIN-96 EPC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sgtin96 {
    /// Filter value (3 bits, range `0..=7`).
    ///
    /// In EPC Tag URI this is the first component after the scheme.
    ///
    /// Use this as an application-level grouping hint (for example, `1` for
    /// race bibs, `2` for staff badges). It should be stable and documented.
    pub filter: u8,
    /// Partition value (3 bits, range `0..=6`).
    ///
    /// Selects how many bits/digits belong to `company_prefix` vs
    /// `item_reference`.
    pub partition: u8,
    /// Company prefix value.
    ///
    /// Width depends on `partition` (see module-level partition table).
    pub company_prefix: u64,
    /// Item reference value (includes indicator digit).
    ///
    /// Width depends on `partition` (see module-level partition table).
    pub item_reference: u64,
    /// Serial number value (38 bits).
    pub serial: u64,
}

/// Decoded field values for a GIAI-96 EPC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Giai96 {
    /// Filter value (3 bits, range `0..=7`).
    ///
    /// Use this as an application-level grouping hint (for example, `1` for
    /// race bibs, `2` for staff badges). It should be stable and documented.
    pub filter: u8,
    /// Partition value (3 bits, range `0..=6`).
    ///
    /// Selects how many bits/digits belong to `company_prefix` vs
    /// `individual_asset_reference`.
    pub partition: u8,
    /// Company prefix value.
    ///
    /// Width depends on `partition` (see module-level partition table).
    pub company_prefix: u64,
    /// Individual asset reference value (numeric for GIAI-96).
    ///
    /// Width depends on `partition` (see module-level partition table).
    pub individual_asset_reference: u64,
}

impl Sgtin96 {
    fn partition_info(partition: u8) -> Result<(u8, u8, u8, u8), ProtocolError> {
        // (company_bits, company_digits, item_bits, item_digits)
        match partition {
            0 => Ok((40, 12, 4, 1)),
            1 => Ok((37, 11, 7, 2)),
            2 => Ok((34, 10, 10, 3)),
            3 => Ok((30, 9, 14, 4)),
            4 => Ok((27, 8, 17, 5)),
            5 => Ok((24, 7, 20, 6)),
            6 => Ok((20, 6, 24, 7)),
            _ => Err(ProtocolError::InvalidArgument(
                "sgtin-96 partition must be in 0..=6",
            )),
        }
    }

    fn max_value_for_digits(digits: u8) -> u64 {
        let mut acc = 1u64;
        for _ in 0..digits {
            acc *= 10;
        }
        acc - 1
    }

    /// Build an EPC Tag URI (`urn:epc:tag:sgtin-96`) for this decoded SGTIN-96 value.
    pub fn to_tag_uri(self) -> Result<String, ProtocolError> {
        let (_, company_digits, _, item_digits) = Self::partition_info(self.partition)?;
        let company = format!("{:0width$}", self.company_prefix, width = company_digits as usize);
        let item = format!("{:0width$}", self.item_reference, width = item_digits as usize);
        Ok(format!(
            "urn:epc:tag:sgtin-96:{}.{}.{}.{}",
            self.filter, company, item, self.serial
        ))
    }

    /// Build an EPC Pure Identity URI (`urn:epc:id:sgtin`) for this SGTIN-96 value.
    pub fn to_pure_identity_uri(self) -> Result<String, ProtocolError> {
        let (_, company_digits, _, item_digits) = Self::partition_info(self.partition)?;
        let company = format!("{:0width$}", self.company_prefix, width = company_digits as usize);
        let item = format!("{:0width$}", self.item_reference, width = item_digits as usize);
        Ok(format!("urn:epc:id:sgtin:{company}.{item}.{}", self.serial))
    }
}

impl Giai96 {
    fn partition_info(partition: u8) -> Result<(u8, u8, u8), ProtocolError> {
        // (company_bits, company_digits, asset_bits)
        match partition {
            0 => Ok((40, 12, 42)),
            1 => Ok((37, 11, 45)),
            2 => Ok((34, 10, 48)),
            3 => Ok((30, 9, 52)),
            4 => Ok((27, 8, 55)),
            5 => Ok((24, 7, 58)),
            6 => Ok((20, 6, 62)),
            _ => Err(ProtocolError::InvalidArgument(
                "giai-96 partition must be in 0..=6",
            )),
        }
    }

    /// Build an EPC Tag URI (`urn:epc:tag:giai-96`) for this decoded GIAI-96 value.
    pub fn to_tag_uri(self) -> Result<String, ProtocolError> {
        let (_, company_digits, _) = Self::partition_info(self.partition)?;
        let company = format!("{:0width$}", self.company_prefix, width = company_digits as usize);
        Ok(format!(
            "urn:epc:tag:giai-96:{}.{}.{}",
            self.filter, company, self.individual_asset_reference
        ))
    }

    /// Build an EPC Pure Identity URI (`urn:epc:id:giai`) for this GIAI-96 value.
    pub fn to_pure_identity_uri(self) -> Result<String, ProtocolError> {
        let (_, company_digits, _) = Self::partition_info(self.partition)?;
        let company = format!("{:0width$}", self.company_prefix, width = company_digits as usize);
        Ok(format!(
            "urn:epc:id:giai:{company}.{}",
            self.individual_asset_reference
        ))
    }
}

impl EpcValue {
        /// Create an EPC value from a typed schema struct.
        ///
        /// This is the single generic constructor for supported schema types,
        /// such as [`Sgtin96`] and [`Giai96`].
        pub fn from_schema<S: EpcSchema>(schema: S) -> Result<Self, ProtocolError> {
            schema.encode()
        }

    /// Create an EPC value from raw bytes.
    pub fn new(bytes: Vec<u8>) -> Result<Self, ProtocolError> {
        if bytes.is_empty() {
            return Err(ProtocolError::InvalidArgument("epc bytes cannot be empty"));
        }
        Ok(Self { bytes })
    }

    /// Create an EPC value from a byte slice.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, ProtocolError> {
        Self::new(bytes.to_vec())
    }

    /// Create an EPC value from parsed tag data.
    pub fn from_tag(tag: &TagEpcAndMetaData) -> Result<Self, ProtocolError> {
        Self::new(tag.epc_id.clone())
    }

    fn encode_sgtin96(fields: Sgtin96) -> Result<Self, ProtocolError> {
        let (company_bits, company_digits, item_bits, item_digits) =
            Sgtin96::partition_info(fields.partition)?;

        if fields.filter > 0b111 {
            return Err(ProtocolError::InvalidArgument(
                "sgtin-96 filter must be in 0..=7",
            ));
        }
        if fields.serial >= (1u64 << 38) {
            return Err(ProtocolError::InvalidArgument(
                "sgtin-96 serial must fit in 38 bits",
            ));
        }

        let max_company_digits = Sgtin96::max_value_for_digits(company_digits);
        let max_item_digits = Sgtin96::max_value_for_digits(item_digits);
        if fields.company_prefix > max_company_digits {
            return Err(ProtocolError::InvalidArgument(
                "sgtin-96 company prefix exceeds partition digit capacity",
            ));
        }
        if fields.item_reference > max_item_digits {
            return Err(ProtocolError::InvalidArgument(
                "sgtin-96 item reference exceeds partition digit capacity",
            ));
        }

        let max_company_bits = (1u64 << company_bits) - 1;
        let max_item_bits = (1u64 << item_bits) - 1;
        if fields.company_prefix > max_company_bits {
            return Err(ProtocolError::InvalidArgument(
                "sgtin-96 company prefix exceeds partition bit capacity",
            ));
        }
        if fields.item_reference > max_item_bits {
            return Err(ProtocolError::InvalidArgument(
                "sgtin-96 item reference exceeds partition bit capacity",
            ));
        }

        let mut bits = 0u128;
        bits |= 0x30u128 << 88;
        bits |= (fields.filter as u128) << 85;
        bits |= (fields.partition as u128) << 82;
        bits |= (fields.company_prefix as u128) << (38 + item_bits);
        bits |= (fields.item_reference as u128) << 38;
        bits |= fields.serial as u128;

        let mut out = vec![0u8; 12];
        for i in 0..12 {
            let shift = (11 - i) * 8;
            out[i] = ((bits >> shift) & 0xFF) as u8;
        }

        Self::new(out)
    }

    fn encode_giai96(fields: Giai96) -> Result<Self, ProtocolError> {
        let (company_bits, company_digits, asset_bits) = Giai96::partition_info(fields.partition)?;

        if fields.filter > 0b111 {
            return Err(ProtocolError::InvalidArgument(
                "giai-96 filter must be in 0..=7",
            ));
        }

        let max_company_digits = Sgtin96::max_value_for_digits(company_digits);
        if fields.company_prefix > max_company_digits {
            return Err(ProtocolError::InvalidArgument(
                "giai-96 company prefix exceeds partition digit capacity",
            ));
        }

        let max_company_bits = (1u64 << company_bits) - 1;
        if fields.company_prefix > max_company_bits {
            return Err(ProtocolError::InvalidArgument(
                "giai-96 company prefix exceeds partition bit capacity",
            ));
        }

        let max_asset_bits = if asset_bits == 64 {
            u64::MAX
        } else {
            (1u64 << asset_bits) - 1
        };
        if fields.individual_asset_reference > max_asset_bits {
            return Err(ProtocolError::InvalidArgument(
                "giai-96 individual asset reference exceeds partition bit capacity",
            ));
        }

        let mut bits = 0u128;
        bits |= 0x34u128 << 88;
        bits |= (fields.filter as u128) << 85;
        bits |= (fields.partition as u128) << 82;
        bits |= (fields.company_prefix as u128) << asset_bits;
        bits |= fields.individual_asset_reference as u128;

        let mut out = vec![0u8; 12];
        for i in 0..12 {
            let shift = (11 - i) * 8;
            out[i] = ((bits >> shift) & 0xFF) as u8;
        }

        Self::new(out)
    }

    /// Decode this EPC value as SGTIN-96 fields.
    ///
    /// Returns an error when EPC length is not 12 bytes or header is not `0x30`.
    pub fn decode_sgtin96(&self) -> Result<Sgtin96, ProtocolError> {
        if self.bytes.len() != 12 {
            return Err(ProtocolError::InvalidArgument(
                "sgtin-96 requires exactly 12 EPC bytes",
            ));
        }

        let mut bits = 0u128;
        for &b in &self.bytes {
            bits = (bits << 8) | (b as u128);
        }

        let header = ((bits >> 88) & 0xFF) as u8;
        if header != 0x30 {
            return Err(ProtocolError::InvalidArgument(
                "epc header is not sgtin-96 (expected 0x30)",
            ));
        }

        let filter = ((bits >> 85) & 0x07) as u8;
        let partition = ((bits >> 82) & 0x07) as u8;
        let (company_bits, _company_digits, item_bits, _item_digits) =
            Sgtin96::partition_info(partition)?;

        let company_mask = (1u128 << company_bits) - 1;
        let item_mask = (1u128 << item_bits) - 1;

        let company_prefix = ((bits >> (38 + item_bits)) & company_mask) as u64;
        let item_reference = ((bits >> 38) & item_mask) as u64;
        let serial = (bits & ((1u128 << 38) - 1)) as u64;

        Ok(Sgtin96 {
            filter,
            partition,
            company_prefix,
            item_reference,
            serial,
        })
    }

    /// Decode this EPC value as GIAI-96 fields.
    ///
    /// Returns an error when EPC length is not 12 bytes or header is not `0x34`.
    pub fn decode_giai96(&self) -> Result<Giai96, ProtocolError> {
        if self.bytes.len() != 12 {
            return Err(ProtocolError::InvalidArgument(
                "giai-96 requires exactly 12 EPC bytes",
            ));
        }

        let mut bits = 0u128;
        for &b in &self.bytes {
            bits = (bits << 8) | (b as u128);
        }

        let header = ((bits >> 88) & 0xFF) as u8;
        if header != 0x34 {
            return Err(ProtocolError::InvalidArgument(
                "epc header is not giai-96 (expected 0x34)",
            ));
        }

        let filter = ((bits >> 85) & 0x07) as u8;
        let partition = ((bits >> 82) & 0x07) as u8;
        let (company_bits, _company_digits, asset_bits) = Giai96::partition_info(partition)?;

        let company_mask = (1u128 << company_bits) - 1;
        let asset_mask = (1u128 << asset_bits) - 1;

        let company_prefix = ((bits >> asset_bits) & company_mask) as u64;
        let individual_asset_reference = (bits & asset_mask) as u64;

        Ok(Giai96 {
            filter,
            partition,
            company_prefix,
            individual_asset_reference,
        })
    }

    /// Borrow raw EPC bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consume and return raw EPC bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Render EPC bytes as uppercase hexadecimal.
    pub fn to_hex(&self) -> String {
        let mut out = String::with_capacity(self.bytes.len() * 2);
        for b in &self.bytes {
            out.push_str(&format!("{b:02X}"));
        }
        out
    }

    fn is_unreserved_uri_byte(b: u8) -> bool {
        b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~')
    }

    fn percent_encode_component(component: &str) -> String {
        let mut out = String::with_capacity(component.len());
        for &b in component.as_bytes() {
            if Self::is_unreserved_uri_byte(b) {
                out.push(char::from(b));
            } else {
                out.push_str(&format!("%{b:02X}"));
            }
        }
        out
    }

    fn hex_nibble(byte: u8) -> Option<u8> {
        match byte {
            b'0'..=b'9' => Some(byte - b'0'),
            b'a'..=b'f' => Some(byte - b'a' + 10),
            b'A'..=b'F' => Some(byte - b'A' + 10),
            _ => None,
        }
    }

    fn percent_decode_component(component: &str) -> Result<String, ProtocolError> {
        let bytes = component.as_bytes();
        let mut out = Vec::with_capacity(bytes.len());
        let mut i = 0;

        while i < bytes.len() {
            if bytes[i] == b'%' {
                if i + 2 >= bytes.len() {
                    return Err(ProtocolError::InvalidArgument(
                        "invalid percent encoding in EPC URI component",
                    ));
                }
                let hi = Self::hex_nibble(bytes[i + 1]).ok_or(ProtocolError::InvalidArgument(
                    "invalid percent encoding in EPC URI component",
                ))?;
                let lo = Self::hex_nibble(bytes[i + 2]).ok_or(ProtocolError::InvalidArgument(
                    "invalid percent encoding in EPC URI component",
                ))?;
                out.push((hi << 4) | lo);
                i += 3;
            } else {
                out.push(bytes[i]);
                i += 1;
            }
        }

        String::from_utf8(out)
            .map_err(|_| ProtocolError::InvalidArgument("EPC URI component is not UTF-8"))
    }

    fn validate_tag_scheme(tag_scheme: &str) -> Result<(), ProtocolError> {
        if tag_scheme.is_empty() {
            return Err(ProtocolError::InvalidArgument("tag_scheme cannot be empty"));
        }
        if !tag_scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
        {
            return Err(ProtocolError::InvalidArgument(
                "tag_scheme must contain only [A-Za-z0-9-]",
            ));
        }
        Ok(())
    }

    fn to_identity_scheme(tag_scheme: &str) -> &str {
        if let Some((base, suffix)) = tag_scheme.rsplit_once('-') {
            if !base.is_empty() && suffix.chars().all(|c| c.is_ascii_digit()) {
                return base;
            }
        }
        tag_scheme
    }

    /// Build a custom EPC Tag URI.
    ///
    /// Result format:
    /// `urn:epc:tag:{tag_scheme}:{value1}.{value2}...`
    pub fn tag_uri_custom(tag_scheme: &str, values: &[&str]) -> Result<String, ProtocolError> {
        Self::validate_tag_scheme(tag_scheme)?;
        if values.is_empty() {
            return Err(ProtocolError::InvalidArgument(
                "custom EPC Tag URI must include at least one value",
            ));
        }
        if values.iter().any(|v| v.is_empty()) {
            return Err(ProtocolError::InvalidArgument(
                "custom EPC Tag URI values cannot be empty",
            ));
        }

        let encoded_values: Vec<String> = values
            .iter()
            .map(|v| Self::percent_encode_component(v))
            .collect();

        Ok(format!(
            "urn:epc:tag:{tag_scheme}:{}",
            encoded_values.join(".")
        ))
    }

    /// Decode a custom EPC Tag URI into `(tag_scheme, values)`.
    ///
    /// Input format:
    /// `urn:epc:tag:{tag_scheme}:{value1}.{value2}...`
    pub fn parse_tag_uri_custom(uri: &str) -> Result<(String, Vec<String>), ProtocolError> {
        let body = uri
            .strip_prefix("urn:epc:tag:")
            .ok_or(ProtocolError::InvalidArgument(
                "EPC Tag URI must start with 'urn:epc:tag:'",
            ))?;

        let (tag_scheme, value_part) = body
            .split_once(':')
            .ok_or(ProtocolError::InvalidArgument("EPC Tag URI is missing value section"))?;

        Self::validate_tag_scheme(tag_scheme)?;

        if value_part.is_empty() {
            return Err(ProtocolError::InvalidArgument(
                "EPC Tag URI values cannot be empty",
            ));
        }

        let mut values = Vec::new();
        for raw in value_part.split('.') {
            if raw.is_empty() {
                return Err(ProtocolError::InvalidArgument(
                    "EPC Tag URI values cannot contain empty components",
                ));
            }
            values.push(Self::percent_decode_component(raw)?);
        }

        Ok((tag_scheme.to_string(), values))
    }

    /// Convert an EPC Tag URI (`urn:epc:tag`) into a Pure Identity URI (`urn:epc:id`).
    ///
    /// This follows GS1 URI conventions where tag URIs include a filter value
    /// as the first value component and pure identity URIs omit that filter.
    pub fn tag_uri_to_pure_identity_uri(tag_uri: &str) -> Result<String, ProtocolError> {
        let (tag_scheme, values) = Self::parse_tag_uri_custom(tag_uri)?;
        if values.len() < 2 {
            return Err(ProtocolError::InvalidArgument(
                "EPC Tag URI must include filter and identity values",
            ));
        }

        let identity_scheme = Self::to_identity_scheme(&tag_scheme);
        let identity_values = &values[1..];

        Ok(format!(
            "urn:epc:id:{identity_scheme}:{}",
            identity_values.join(".")
        ))
    }

    /// Build a custom EPC Tag URI and append this EPC value as the last URI component.
    ///
    /// Example result:
    /// `urn:epc:tag:raw:myCompany.01.300011223344`
    pub fn tag_uri_with_epc(
        &self,
        tag_scheme: &str,
        values: &[&str],
    ) -> Result<String, ProtocolError> {
        let mut all_values: Vec<&str> = Vec::with_capacity(values.len() + 1);
        all_values.extend(values.iter().copied());

        let epc_hex = self.to_hex();
        let epc_ref = epc_hex.as_str();
        all_values.push(epc_ref);

        Self::tag_uri_custom(tag_scheme, &all_values)
    }
}

impl EpcSchema for Sgtin96 {
    /// Encode an SGTIN-96 payload to raw EPC bytes.
    ///
    /// Validation performed:
    /// - `filter` must be `0..=7`
    /// - `partition` must be `0..=6`
    /// - `serial` must fit in 38 bits
    /// - `company_prefix` and `item_reference` must fit both the partition
    ///   digit constraints and bit constraints
    fn encode(self) -> Result<EpcValue, ProtocolError> {
        EpcValue::encode_sgtin96(self)
    }
}

impl EpcSchema for Giai96 {
    /// Encode a GIAI-96 payload to raw EPC bytes.
    ///
    /// Validation performed:
    /// - `filter` must be `0..=7`
    /// - `partition` must be `0..=6`
    /// - `company_prefix` must fit partition digit/bit constraints
    /// - `individual_asset_reference` must fit partition bit constraints
    fn encode(self) -> Result<EpcValue, ProtocolError> {
        EpcValue::encode_giai96(self)
    }
}

#[cfg(test)]
mod tests {
    use super::{EpcValue, Giai96, Sgtin96};

    #[test]
    fn epc_value_hex_and_tag_uri_helpers() {
        let epc = EpcValue::from_slice(&[0x30, 0x00, 0x11, 0x22]).expect("valid EPC");
        assert_eq!(epc.to_hex(), "30001122");

        let uri =
            EpcValue::tag_uri_custom("gid-96", &["1234", "5678", "90"]).expect("valid URI");
        assert_eq!(uri, "urn:epc:tag:gid-96:1234.5678.90");

        let uri_encoded = EpcValue::tag_uri_custom("sgtin-96", &["3", "0614141", "A/B C"]) 
            .expect("valid URI with escaping");
        assert_eq!(uri_encoded, "urn:epc:tag:sgtin-96:3.0614141.A%2FB%20C");

        let (scheme, values) = EpcValue::parse_tag_uri_custom(&uri_encoded)
            .expect("valid URI decode");
        assert_eq!(scheme, "sgtin-96");
        assert_eq!(values, vec!["3", "0614141", "A/B C"]);

        let pure = EpcValue::tag_uri_to_pure_identity_uri("urn:epc:tag:sgtin-96:3.0614141.812345.6789")
            .expect("valid pure identity conversion");
        assert_eq!(pure, "urn:epc:id:sgtin:0614141.812345.6789");

        let uri_with_epc = epc
            .tag_uri_with_epc("raw", &["custom", "1"])
            .expect("valid URI with EPC");
        assert_eq!(uri_with_epc, "urn:epc:tag:raw:custom.1.30001122");
    }

    #[test]
    fn sgtin96_encode_decode_roundtrip() {
        let fields = Sgtin96 {
            filter: 3,
            partition: 6,
            company_prefix: 614141,
            item_reference: 8123456,
            serial: 6789,
        };

        let epc = EpcValue::from_schema(fields).expect("encode sgtin-96");
        let decoded = epc.decode_sgtin96().expect("decode sgtin-96");
        assert_eq!(decoded, fields);

        let tag_uri = decoded.to_tag_uri().expect("tag uri");
        assert_eq!(tag_uri, "urn:epc:tag:sgtin-96:3.614141.8123456.6789");

        let pure_uri = decoded.to_pure_identity_uri().expect("pure uri");
        assert_eq!(pure_uri, "urn:epc:id:sgtin:614141.8123456.6789");
    }

    #[test]
    fn giai96_encode_decode_roundtrip() {
        let fields = Giai96 {
            filter: 1,
            partition: 6,
            company_prefix: 614141,
            individual_asset_reference: 123456789,
        };

        let epc = EpcValue::from_schema(fields).expect("encode giai-96");
        let decoded = epc.decode_giai96().expect("decode giai-96");
        assert_eq!(decoded, fields);

        let tag_uri = decoded.to_tag_uri().expect("tag uri");
        assert_eq!(tag_uri, "urn:epc:tag:giai-96:1.614141.123456789");

        let pure_uri = decoded.to_pure_identity_uri().expect("pure uri");
        assert_eq!(pure_uri, "urn:epc:id:giai:614141.123456789");
    }
}
