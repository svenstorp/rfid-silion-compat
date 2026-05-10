use std::collections::VecDeque;

use rfidlibrs::{protocol_crc16, CommandCode, ReaderTransport, SilionHost};

#[derive(Debug)]
struct MockTransport {
    rx: VecDeque<u8>,
}

impl MockTransport {
    fn from_frames(frames: Vec<Vec<u8>>) -> Self {
        let mut rx = VecDeque::new();
        for frame in frames {
            rx.extend(frame);
        }
        Self { rx }
    }
}

impl ReaderTransport for MockTransport {
    type Error = &'static str;

    fn write_all(&mut self, _data: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        if self.rx.len() < buf.len() {
            return Err("eof");
        }
        for b in buf.iter_mut() {
            *b = self.rx.pop_front().ok_or("eof")?;
        }
        Ok(())
    }
}

fn mk_frame(command: u8, status: u16, data: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(0xFF);
    out.push(data.len() as u8);
    out.push(command);
    out.extend_from_slice(&status.to_be_bytes());
    out.extend_from_slice(data);
    let crc = protocol_crc16(&out);
    out.extend_from_slice(&crc.to_be_bytes());
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let version = mk_frame(
        CommandCode::GetVersion as u8,
        0x0000,
        &[
            0x13, 0x04, 0x15, 0x00, 0xA8, 0x00, 0x00, 0x01, 0x20, 0x13, 0x05, 0x22, 0x13, 0x05,
            0x23, 0x00, 0x00, 0x00, 0x00, 0x10,
        ],
    );
    let region = mk_frame(CommandCode::GetCurrentRegion as u8, 0x0000, &[0x01]);
    let temp = mk_frame(CommandCode::GetCurrentTemperature as u8, 0x0000, &[0x27]);

    let transport = MockTransport::from_frames(vec![version, region, temp]);
    let mut host = SilionHost::new(transport);

    let v = host.get_version()?;
    println!("FW version bytes: {:02X?}", v.firmware_version);

    let region = host.get_current_region()?;
    println!("Region: {region}");

    let temperature = host.get_current_temperature()?;
    println!("Temperature: {temperature}C");

    Ok(())
}
