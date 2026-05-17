use std::collections::VecDeque;

use crate::{ReaderTransport, frame::protocol_crc16};

/// One scripted request/response interaction for [`MockTransport`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MockInteraction {
    /// Expected command code in the host request.
    pub request_command: u8,
    /// Status code returned in the reader reply.
    pub response_status: u16,
    /// Data payload returned in the reader reply.
    pub response_data: Vec<u8>,
}

/// Mock transport for rustdoc examples and unit tests.
#[derive(Debug, Default)]
pub struct MockTransport {
    rx: VecDeque<u8>,
    scripted: VecDeque<MockInteraction>,
}

impl MockTransport {
    /// Build a request-aware mock transport from scripted interactions.
    ///
    /// The transport validates the command written by the host and injects the
    /// matching response frame automatically.
    pub fn scripted(interactions: Vec<MockInteraction>) -> Self {
        Self {
            rx: VecDeque::new(),
            scripted: interactions.into(),
        }
    }

    /// Build a mock transport that will yield the provided reply frames in order.
    pub fn from_replies(replies: Vec<Vec<u8>>) -> Self {
        let mut rx = VecDeque::new();
        for reply in replies {
            rx.extend(reply);
        }
        Self {
            rx,
            scripted: VecDeque::new(),
        }
    }
}

impl ReaderTransport for MockTransport {
    type Error = &'static str;

    async fn write_all(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        if let Some(next) = self.scripted.pop_front() {
            if data.len() < 3 || data[0] != 0xFF {
                return Err("invalid request frame");
            }
            if data[2] != next.request_command {
                return Err("unexpected request command");
            }
            let reply = reply_frame(
                next.request_command,
                next.response_status,
                &next.response_data,
            );
            self.rx.extend(reply);
        }
        Ok(())
    }

    async fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        if self.rx.len() < buf.len() {
            return Err("eof");
        }
        for b in buf.iter_mut() {
            *b = self.rx.pop_front().ok_or("eof")?;
        }
        Ok(())
    }
}

/// Build a full reader reply frame (header + status + payload + CRC).
pub fn reply_frame(command: u8, status: u16, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + 1 + 1 + 2 + data.len() + 2);
    out.push(0xFF);
    out.push(data.len() as u8);
    out.push(command);
    out.extend_from_slice(&status.to_be_bytes());
    out.extend_from_slice(data);
    let crc = protocol_crc16(&out);
    out.extend_from_slice(&crc.to_be_bytes());
    out
}
