
use crate::error::ProtocolError;

pub(crate) const ASYNC_MARKER: &[u8; 10] = b"Moduletech";
pub(crate) const ASYNC_TERMINATOR: u8 = 0xBB;

/// Parsed asynchronous inventory payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsyncPayload<'a> {
    /// Subcommand code.
    pub subcommand: u16,
    /// Subcommand data body.
    pub subcommand_data: &'a [u8],
}

/// Parsed asynchronous inventory payload with owned data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsyncPayloadOwned {
    /// Subcommand code.
    pub subcommand: u16,
    /// Subcommand data body.
    pub subcommand_data: Vec<u8>,
}

/// Parse `0xAA` asynchronous inventory payload data.
///
/// The function validates vendor framing inside the response data field:
/// - fixed marker `Moduletech`,
/// - 2-byte subcommand,
/// - trailing sub-CRC,
/// - trailing terminator `0xBB`.
///
/// On success it returns the parsed subcommand and subcommand data bytes.
pub fn parse_async_payload(data: &[u8]) -> Result<AsyncPayload<'_>, ProtocolError> {
    if data.len() < 13 {
        return Err(ProtocolError::InvalidResponse(
            "async payload too short for marker+code+subcrc+terminator",
        ));
    }
    if &data[..10] != ASYNC_MARKER {
        return Err(ProtocolError::InvalidResponse("invalid async marker"));
    }
    let subcommand = u16::from_be_bytes([data[10], data[11]]);
    let terminator = data[data.len() - 1];
    if terminator != ASYNC_TERMINATOR {
        return Err(ProtocolError::InvalidResponse("invalid async terminator"));
    }
    let sub_crc_actual = data[data.len() - 2];
    let subcommand_data = &data[12..data.len() - 2];
    let sub_crc_expected = subcommand_crc(subcommand, subcommand_data);
    if sub_crc_actual != sub_crc_expected {
        return Err(ProtocolError::InvalidResponse("invalid async sub-CRC"));
    }
    Ok(AsyncPayload {
        subcommand,
        subcommand_data,
    })
}

/// Parse `0xAA` asynchronous inventory payload data and return owned bytes.
pub fn parse_async_payload_owned(data: &[u8]) -> Result<AsyncPayloadOwned, ProtocolError> {
    let parsed = parse_async_payload(data)?;
    Ok(AsyncPayloadOwned {
        subcommand: parsed.subcommand,
        subcommand_data: parsed.subcommand_data.to_vec(),
    })
}

/// Compute asynchronous subcommand checksum (`SubCRC`).
///
/// The algorithm is the lower 8 bits of the sum of:
/// - subcommand high byte,
/// - subcommand low byte,
/// - each byte of subcommand data.
pub fn subcommand_crc(subcommand: u16, subcommand_data: &[u8]) -> u8 {
    let [hi, lo] = subcommand.to_be_bytes();
    let mut sum = hi.wrapping_add(lo);
    for b in subcommand_data {
        sum = sum.wrapping_add(*b);
    }
    sum
}
