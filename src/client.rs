use core::fmt;

use crate::codes::StatusCode;
use crate::error::ProtocolError;
use crate::frame::{build_host_frame, parse_reader_frame, ReaderFrame};
use crate::transport::ReaderTransport;

/// Errors produced by the protocol client.
#[derive(Debug)]
pub enum ClientError<TE> {
    /// Underlying transport error.
    Transport(TE),
    /// Packet or payload format error.
    Protocol(ProtocolError),
    /// Reader returned a non-success status code.
    ReaderStatus {
        /// Raw status code.
        status_raw: u16,
        /// Parsed status when known.
        status: Option<StatusCode>,
    },
    /// Response command code does not match request command code.
    UnexpectedResponseCommand {
        /// Sent command code.
        expected: u8,
        /// Received command code.
        actual: u8,
    },
}

impl<TE: fmt::Display> fmt::Display for ClientError<TE> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Transport(e) => write!(f, "transport error: {e}"),
            Self::Protocol(e) => write!(f, "protocol error: {e}"),
            Self::ReaderStatus { status_raw, status } => {
                write!(f, "reader returned status 0x{status_raw:04X} ({status:?})")
            }
            Self::UnexpectedResponseCommand { expected, actual } => {
                write!(f, "unexpected response command: expected 0x{expected:02X}, got 0x{actual:02X}")
            }
        }
    }
}

impl<TE: fmt::Debug + fmt::Display> std::error::Error for ClientError<TE> {}

/// Synchronous protocol client over a `ReaderTransport`.
pub struct ReaderClient<T: ReaderTransport> {
    transport: T,
}

impl<T: ReaderTransport> ReaderClient<T> {
    /// Create a low-level protocol client over a transport.
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    /// Consume this client and return the wrapped transport.
    pub fn into_inner(self) -> T {
        self.transport
    }

    /// Return a mutable reference to the wrapped transport.
    ///
    /// Use this to reconfigure transport parameters (for example the read
    /// timeout) without consuming the client.
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// Write a pre-built host frame to the transport without reading a response.
    ///
    /// Use this when the response will arrive as an unsolicited pushed frame,
    /// for example when sending the async inventory stop command while a
    /// background reader thread drains incoming frames.
    pub fn write_frame(&mut self, data: &[u8]) -> Result<(), ClientError<T::Error>> {
        self.transport
            .write_all(data)
            .map_err(ClientError::Transport)
    }

    /// Read and parse one reader-to-host frame without sending a request first.
    ///
    /// This is useful for unsolicited messages (for example asynchronous
    /// inventory events) that the reader pushes after a command has enabled a
    /// streaming mode.
    pub fn read_frame(&mut self) -> Result<ReaderFrame, ClientError<T::Error>> {
        let mut prefix = [0u8; 5];
        self.transport
            .read_exact(&mut prefix)
            .map_err(ClientError::Transport)?;

        let data_len = prefix[1] as usize;
        let mut suffix = vec![0u8; data_len + 2];
        self.transport
            .read_exact(&mut suffix)
            .map_err(ClientError::Transport)?;

        let mut packet = Vec::with_capacity(prefix.len() + suffix.len());
        packet.extend_from_slice(&prefix);
        packet.extend_from_slice(&suffix);

        parse_reader_frame(&packet).map_err(ClientError::Protocol)
    }

    /// Send one pre-built host frame and parse a single response frame.
    ///
    /// This method does not enforce command echo matching or success status;
    /// use [`ReaderClient::transact`] for command-oriented flow.
    ///
    /// # Examples
    /// ```rust
    /// use std::collections::VecDeque;
    /// use rfidlibrs::{protocol_crc16, ReaderClient, ReaderTransport};
    ///
    /// struct MockTransport { rx: VecDeque<u8> }
    ///
    /// impl ReaderTransport for MockTransport {
    ///     type Error = &'static str;
    ///
    ///     fn write_all(&mut self, _data: &[u8]) -> Result<(), Self::Error> { Ok(()) }
    ///
    ///     fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
    ///         if self.rx.len() < buf.len() { return Err("eof"); }
    ///         for b in buf.iter_mut() { *b = self.rx.pop_front().ok_or("eof")?; }
    ///         Ok(())
    ///     }
    /// }
    ///
    /// fn mk_frame(command: u8, status: u16, data: &[u8]) -> Vec<u8> {
    ///     let mut out = vec![0xFF, data.len() as u8, command];
    ///     out.extend_from_slice(&status.to_be_bytes());
    ///     out.extend_from_slice(data);
    ///     let crc = protocol_crc16(&out);
    ///     out.extend_from_slice(&crc.to_be_bytes());
    ///     out
    /// }
    ///
    /// let packet = mk_frame(0x0C, 0x0000, &[0x12]);
    /// let transport = MockTransport { rx: packet.into_iter().collect() };
    /// let mut client = ReaderClient::new(transport);
    /// let frame = client.transact_frame(&[0xFF, 0x00, 0x0C, 0x1D, 0x03]).unwrap();
    /// assert_eq!(frame.command, 0x0C);
    /// assert_eq!(frame.data, vec![0x12]);
    /// ```
    pub fn transact_frame(&mut self, request: &[u8]) -> Result<ReaderFrame, ClientError<T::Error>> {
        self.transport
            .write_all(request)
            .map_err(ClientError::Transport)?;
        self.read_frame()
    }

    /// Build and send one command payload, then parse one validated response.
    ///
    /// Additional checks beyond [`ReaderClient::transact_frame`]:
    /// - response command must match `command`,
    /// - status must be `0x0000` (success).
    ///
    /// Non-success reader statuses are returned as [`ClientError::ReaderStatus`].
    ///
    /// # Examples
    /// ```rust
    /// use std::collections::VecDeque;
    /// use rfidlibrs::{protocol_crc16, CommandCode, ReaderClient, ReaderTransport};
    ///
    /// struct MockTransport { rx: VecDeque<u8> }
    ///
    /// impl ReaderTransport for MockTransport {
    ///     type Error = &'static str;
    ///     fn write_all(&mut self, _data: &[u8]) -> Result<(), Self::Error> { Ok(()) }
    ///     fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
    ///         if self.rx.len() < buf.len() { return Err("eof"); }
    ///         for b in buf.iter_mut() { *b = self.rx.pop_front().ok_or("eof")?; }
    ///         Ok(())
    ///     }
    /// }
    ///
    /// let mut reply = vec![0xFF, 0x01, CommandCode::GetCurrentRegion as u8, 0x00, 0x00, 0x01];
    /// let crc = protocol_crc16(&reply);
    /// reply.extend_from_slice(&crc.to_be_bytes());
    ///
    /// let transport = MockTransport { rx: reply.into_iter().collect() };
    /// let mut client = ReaderClient::new(transport);
    /// let frame = client.transact(CommandCode::GetCurrentRegion as u8, &[]).unwrap();
    /// assert_eq!(frame.data, vec![0x01]);
    /// ```
    pub fn transact(
        &mut self,
        command: u8,
        data: &[u8],
    ) -> Result<ReaderFrame, ClientError<T::Error>> {
        let request = build_host_frame(command, data).map_err(ClientError::Protocol)?;
        let response = self.transact_frame(&request)?;

        if response.command != command {
            return Err(ClientError::UnexpectedResponseCommand {
                expected: command,
                actual: response.command,
            });
        }

        if response.status_raw != StatusCode::Success as u16 {
            return Err(ClientError::ReaderStatus {
                status_raw: response.status_raw,
                status: response.status,
            });
        }

        Ok(response)
    }
}

