//! Wire-frame encoding and decoding helpers.
//!
//! This module provides low-level packet parsing and building utilities for the
//! Silion protocol frame format (`0xFF | len | command | ... | crc16`).

use crate::codes::StatusCode;
use crate::error::ProtocolError;

const HEADER: u8 = 0xFF;
const MAX_DATA_LEN: usize = 255;

/// Decoded reader response frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReaderFrame {
    /// Command code echoed by reader.
    pub command: u8,
    /// Raw 16-bit status code.
    pub status_raw: u16,
    /// Parsed status code when known.
    pub status: Option<StatusCode>,
    /// Data field following status code.
    pub data: Vec<u8>,
}

/// Parse one reader-to-host protocol frame.
///
/// Expected wire format:
/// `Header(0xFF) | DataLen | Command | Status(2B) | Data(N) | CRC16(2B)`.
///
/// `DataLen` is interpreted as the number of bytes in the `Data` field only
/// (not including command, status, or CRC). The function validates:
/// - minimum packet size,
/// - header byte,
/// - total length consistency,
/// - CRC using the vendor CRC routine from the protocol documentation.
///
/// On success, this returns a [`ReaderFrame`] with decoded command/status and
/// the raw `Data` payload bytes.
///
/// Note that this parser consumes a full wire packet (including leading header
/// and trailing CRC), while the returned [`ReaderFrame::data`] contains only
/// the response data field after command+status.
///
/// # Examples
/// ```rust
/// use rfid_silion_compat::frame::parse_reader_frame;
///
/// // Response for Get Run Phase (0x0C), status=0x0000, data=[0x12]
/// let packet = [0xFF, 0x01, 0x0C, 0x00, 0x00, 0x12, 0x63, 0x43];
/// let frame = parse_reader_frame(&packet).unwrap();
///
/// assert_eq!(frame.command, 0x0C);
/// assert_eq!(frame.status_raw, 0x0000);
/// assert_eq!(frame.data, vec![0x12]);
/// ```
pub fn parse_reader_frame(packet: &[u8]) -> Result<ReaderFrame, ProtocolError> {
    if packet.len() < 7 {
        return Err(ProtocolError::PacketTooShort);
    }
    if packet[0] != HEADER {
        return Err(ProtocolError::InvalidHeader(packet[0]));
    }

    let declared_len = packet[1] as usize;
    let expected_total = 1 + 1 + 1 + 2 + declared_len + 2;
    if packet.len() != expected_total {
        return Err(ProtocolError::LengthMismatch {
            declared: declared_len,
            actual: packet.len().saturating_sub(7),
        });
    }

    let crc_actual = u16::from_be_bytes([packet[packet.len() - 2], packet[packet.len() - 1]]);
    let crc_expected = protocol_crc16(&packet[..packet.len() - 2]);
    if crc_actual != crc_expected {
        return Err(ProtocolError::InvalidCrc {
            expected: crc_expected,
            actual: crc_actual,
        });
    }

    let command = packet[2];
    let status_raw = u16::from_be_bytes([packet[3], packet[4]]);
    let data_start = 5;
    let data_end = data_start + declared_len;
    let data = packet[data_start..data_end].to_vec();

    Ok(ReaderFrame {
        command,
        status_raw,
        status: StatusCode::from_u16(status_raw),
        data,
    })
}

/// Build one host-to-reader protocol frame.
///
/// Output wire format:
/// `Header(0xFF) | DataLen | Command | Data(N) | CRC16(2B)`.
///
/// This helper is command-agnostic and is used by all command builder methods
/// in [`HostCommand`]. The frame uses the CRC routine defined by
/// [`protocol_crc16`].
///
/// This function returns the full wire packet, including leading header `0xFF`
/// and trailing CRC bytes.
///
/// # Examples
/// ```rust
/// use rfid_silion_compat::frame::build_host_frame;
///
/// let packet = build_host_frame(0x03, &[]).unwrap();
/// assert_eq!(packet, vec![0xFF, 0x00, 0x03, 0x1D, 0x0C]);
/// ```
pub fn build_host_frame(command: u8, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    if data.len() > MAX_DATA_LEN {
        return Err(ProtocolError::DataTooLong(data.len()));
    }

    let mut out = Vec::with_capacity(1 + 1 + 1 + data.len() + 2);
    out.push(HEADER);
    out.push(data.len() as u8);
    out.push(command);
    out.extend_from_slice(data);
    let crc = protocol_crc16(&out);
    out.extend_from_slice(&crc.to_be_bytes());
    Ok(out)
}

/// Compute protocol CRC-16 for a frame without trailing CRC bytes.
///
/// The documentation implementation starts from byte index 1 (the length field),
/// and intentionally excludes the header byte at index 0.
///
/// This behavior differs from common CCITT helpers that process all bytes; use
/// this function for interoperability with the Silion reader protocol.
///
/// # Examples
/// ```rust
/// use rfid_silion_compat::frame::protocol_crc16;
///
/// let frame_without_crc = [0xFF, 0x00, 0x03];
/// assert_eq!(protocol_crc16(&frame_without_crc), 0x1D0C);
/// ```
pub fn protocol_crc16(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for byte in data.iter().skip(1) {
        let mut mask: u8 = 0x80;
        for _ in 0..8 {
            let xor_flag = (crc & 0x8000) != 0;
            crc <<= 1;
            if (*byte & mask) != 0 {
                crc |= 0x0001;
            }
            if xor_flag {
                crc ^= 0x1021;
            }
            mask >>= 1;
        }
    }
    crc
}

pub(crate) fn push_u16_be(buf: &mut Vec<u8>, v: u16) {
    buf.extend_from_slice(&v.to_be_bytes());
}

pub(crate) fn push_u32_be(buf: &mut Vec<u8>, v: u32) {
    buf.extend_from_slice(&v.to_be_bytes());
}
