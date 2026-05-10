use std::collections::VecDeque;
use std::fmt;

use rfid_silion_compat::{
    ClientError, CommandCode, HostCommand, ReaderClient, ReaderTransport, RegionCode,
    parse_current_region, parse_version_info, protocol_crc16,
};

#[derive(Debug)]
struct MockTransport {
    rx: VecDeque<u8>,
    tx: Vec<u8>,
}

impl MockTransport {
    fn new(response_frames: Vec<Vec<u8>>) -> Self {
        let mut rx = VecDeque::new();
        for frame in response_frames {
            rx.extend(frame);
        }
        Self { rx, tx: Vec::new() }
    }
}

impl ReaderTransport for MockTransport {
    type Error = MockError;

    async fn write_all(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.tx.extend_from_slice(data);
        Ok(())
    }

    async fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        if self.rx.len() < buf.len() {
            return Err(MockError::Eof);
        }
        for b in buf.iter_mut() {
            *b = self.rx.pop_front().ok_or(MockError::Eof)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
enum MockError {
    Eof,
}

impl fmt::Display for MockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Eof => write!(f, "mock EOF"),
        }
    }
}

impl std::error::Error for MockError {}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let version_reply = build_response(
        0x03,
        0x0000,
        &[
            0x13, 0x04, 0x15, 0x00, 0xA8, 0x00, 0x00, 0x01, 0x20, 0x13, 0x05, 0x22, 0x13, 0x05,
            0x23, 0x00, 0x00, 0x00, 0x00, 0x10,
        ],
    );
    let region_reply = build_response(0x67, 0x0000, &[0x01]);

    let transport = MockTransport::new(vec![version_reply, region_reply]);
    let mut client = ReaderClient::new(transport);

    let version_frame =
        futures::executor::block_on(client.transact(CommandCode::GetVersion as u8, &[]))
            .map_err(render_client_error)?;
    let version = parse_version_info(&version_frame.data)?;
    println!("Firmware version bytes: {:02X?}", version.firmware_version);

    let region_frame =
        futures::executor::block_on(client.transact(CommandCode::GetCurrentRegion as u8, &[]))
            .map_err(render_client_error)?;
    let region = parse_current_region(&region_frame.data)?;
    assert_eq!(region, RegionCode::NorthAmerica);
    println!("Current region: {region}");

    let sent = client.into_inner().tx;
    let expected_get_version = HostCommand::get_version()?;
    assert_eq!(&sent[..expected_get_version.len()], &expected_get_version);

    Ok(())
}

fn render_client_error(e: ClientError<MockError>) -> Box<dyn std::error::Error> {
    Box::new(e)
}

fn build_response(command: u8, status: u16, data: &[u8]) -> Vec<u8> {
    let mut frame = Vec::with_capacity(1 + 1 + 1 + 2 + data.len() + 2);
    frame.push(0xFF);
    frame.push(data.len() as u8);
    frame.push(command);
    frame.extend_from_slice(&status.to_be_bytes());
    frame.extend_from_slice(data);
    let crc = protocol_crc16(&frame);
    frame.extend_from_slice(&crc.to_be_bytes());
    frame
}
