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

/// Protocol client over a [`ReaderTransport`].
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
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// Write a pre-built host frame to the transport without reading a response.
    pub async fn write_frame(&mut self, data: &[u8]) -> Result<(), ClientError<T::Error>> {
        self.transport
            .write_all(data)
            .await
            .map_err(ClientError::Transport)
    }

    /// Read and parse one reader-to-host frame without sending a request first.
    pub async fn read_frame(&mut self) -> Result<ReaderFrame, ClientError<T::Error>> {
        let mut prefix = [0u8; 5];
        self.transport
            .read_exact(&mut prefix)
            .await
            .map_err(ClientError::Transport)?;

        let data_len = prefix[1] as usize;
        let mut suffix = vec![0u8; data_len + 2];
        self.transport
            .read_exact(&mut suffix)
            .await
            .map_err(ClientError::Transport)?;

        let mut packet = Vec::with_capacity(prefix.len() + suffix.len());
        packet.extend_from_slice(&prefix);
        packet.extend_from_slice(&suffix);

        parse_reader_frame(&packet).map_err(ClientError::Protocol)
    }

    /// Send one pre-built host frame and parse a single response frame.
    pub async fn transact_frame(
        &mut self,
        request: &[u8],
    ) -> Result<ReaderFrame, ClientError<T::Error>> {
        self.transport
            .write_all(request)
            .await
            .map_err(ClientError::Transport)?;
        self.read_frame().await
    }

    /// Build and send one command payload, then parse one validated response.
    pub async fn transact(
        &mut self,
        command: u8,
        data: &[u8],
    ) -> Result<ReaderFrame, ClientError<T::Error>> {
        let request = build_host_frame(command, data).map_err(ClientError::Protocol)?;
        let response = self.transact_frame(&request).await?;

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

#[cfg(test)]
mod tests {
    use super::ReaderClient;
    use crate::client::ClientError;
    use crate::codes::CommandCode;
    use crate::test_support::{MockInteraction, MockTransport};

    #[test]
    fn transact_success() {
        let transport = MockTransport::scripted(vec![MockInteraction {
            request_command: CommandCode::GetCurrentRegion as u8,
            response_status: 0x0000,
            response_data: vec![0x01],
        }]);

        let mut client = ReaderClient::new(transport);
        let frame = futures::executor::block_on(client.transact(CommandCode::GetCurrentRegion as u8, &[]))
            .expect("transact should succeed");
        assert_eq!(frame.command, CommandCode::GetCurrentRegion as u8);
        assert_eq!(frame.data, vec![0x01]);
    }

    #[test]
    fn transact_reader_status_error() {
        let transport = MockTransport::scripted(vec![MockInteraction {
            request_command: CommandCode::GetCurrentRegion as u8,
            response_status: 0x010B,
            response_data: vec![],
        }]);

        let mut client = ReaderClient::new(transport);
        let err = futures::executor::block_on(client.transact(CommandCode::GetCurrentRegion as u8, &[]))
            .expect_err("transact should fail with reader status");

        match err {
            ClientError::ReaderStatus { status_raw, .. } => assert_eq!(status_raw, 0x010B),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
