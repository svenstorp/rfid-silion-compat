use core::fmt;

/// Errors returned by this crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    /// Packet is too short to be valid.
    PacketTooShort,
    /// Packet does not start with 0xFF.
    InvalidHeader(u8),
    /// Data length field does not match packet size.
    LengthMismatch {
        /// Length declared in packet.
        declared: usize,
        /// Actual observed payload length.
        actual: usize,
    },
    /// The CRC in packet does not match computed CRC.
    InvalidCrc {
        /// CRC computed from packet bytes.
        expected: u16,
        /// CRC read from packet bytes.
        actual: u16,
    },
    /// Host command data exceeded 255 bytes.
    DataTooLong(usize),
    /// A command-specific argument is invalid.
    InvalidArgument(&'static str),
    /// Reader response had an unexpected format.
    InvalidResponse(&'static str),
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PacketTooShort => write!(f, "packet too short"),
            Self::InvalidHeader(h) => write!(f, "invalid header 0x{h:02X}"),
            Self::LengthMismatch { declared, actual } => {
                write!(f, "length mismatch: declared={declared}, actual={actual}")
            }
            Self::InvalidCrc { expected, actual } => {
                write!(
                    f,
                    "invalid CRC: expected=0x{expected:04X}, actual=0x{actual:04X}"
                )
            }
            Self::DataTooLong(n) => write!(f, "data length {n} exceeds 255 bytes"),
            Self::InvalidArgument(msg) => write!(f, "invalid argument: {msg}"),
            Self::InvalidResponse(msg) => write!(f, "invalid response: {msg}"),
        }
    }
}

impl std::error::Error for ProtocolError {}
