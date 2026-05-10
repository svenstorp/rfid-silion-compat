#![deny(missing_docs)]

//! Host-side implementation of the Silion reader communication protocol.
//!
//! The implementation follows the packet format defined in:
//! <https://en.silion.com.cn/En/doc_center/ModuleAPI_Docs/Communication_Protocol_Doc/html/Protocol_Introduction.html>
//! and command pages linked from that document.
//!
//! ## Packet Lifecycle
//!
//! At the API boundary there are two levels of representation:
//! - **Full wire packet**: includes leading header (`0xFF`) and trailing CRC.
//! - **Parsed frame fields**: command/status plus stripped `data` payload bytes.
//!
//! Builder APIs such as [`build_host_frame`] and [`HostCommand`] return full
//! wire packets. Parser APIs such as [`parse_reader_frame`] consume full wire
//! packets and return structured data.
//!
//! ### Build A Full Host Packet
//! ```rust
//! use rfidlibrs::build_host_frame;
//!
//! // Get Version (0x03) has an empty data field.
//! let packet = build_host_frame(0x03, &[]).unwrap();
//! assert_eq!(packet, vec![0xFF, 0x00, 0x03, 0x1D, 0x0C]);
//! ```
//!
//! ### Parse A Full Reader Packet
//! ```rust
//! use rfidlibrs::parse_reader_frame;
//!
//! // Reader response for Get Run Phase (0x0C), status=0x0000, data=[0x12].
//! let packet = [0xFF, 0x01, 0x0C, 0x00, 0x00, 0x12, 0x63, 0x43];
//! let frame = parse_reader_frame(&packet).unwrap();
//!
//! assert_eq!(frame.command, 0x0C);
//! assert_eq!(frame.status_raw, 0x0000);
//! assert_eq!(frame.data, vec![0x12]);
//! ```

mod error;
mod codes;
mod frame;
mod command;
mod async_proto;
mod parsers;
mod transport;
mod client;
mod silion_reader;
mod session;

/// Shared mock helpers used by rustdoc examples and unit tests.
#[doc(hidden)]
pub mod test_support;

#[cfg(feature = "serial")]
/// Serial port transport adapter backed by the `tokio-serial` crate.
pub mod serial;

#[cfg(all(target_arch = "wasm32", feature = "web-serial"))]
/// Web Serial transport adapter for browser targets.
pub mod web_serial;

#[cfg(all(target_arch = "wasm32", feature = "web-serial"))]
/// wasm-bindgen JavaScript bindings for the reader API.
pub mod web_bindings;

pub use error::ProtocolError;
pub use codes::{AntennaPortsOption, CommandCode, RegionCode, StatusCode};
pub use frame::{build_host_frame, parse_reader_frame, protocol_crc16, ReaderFrame};
pub use command::{
    AntennaPortsConfiguration, AsyncInventoryStartData, AsyncSubcommandCode,
    EmbeddedReadTagData, HostCommand, InventoryEmbeddedCommandContent,
    InventoryOption, InventorySearchFlags, MemBank, MetadataFlags, SelectContent,
};
pub use async_proto::{
    parse_async_payload, parse_async_payload_owned, subcommand_crc, AsyncPayload, AsyncPayloadOwned,
};
pub use transport::ReaderTransport;
pub use parsers::{
    parse_antenna_ports_response, parse_available_regions, parse_current_region,
    parse_current_tag_protocol, parse_current_temperature, parse_frequency_hopping_table,
    parse_pin_states, parse_protocol_configuration_value, parse_reader_configuration_value,
    parse_regulatory_hop_time, parse_run_phase, parse_serial_number_info,
    parse_tag_epc_and_meta_data, parse_version_info,
    AntennaPair, AntennaPortsResponse, AntennaPower, AntennaPowerSettling,
    ProtocolConfigurationValue, ReaderConfigurationValue, RegulatoryHopTime, RunPhase,
    SerialNumberInfo, TagEpcAndMetaData, VersionInfo,
};
pub use client::{ClientError, ReaderClient};
pub use silion_reader::{AsyncInventoryMessage, SilionReader};
pub use session::AsyncInventorySession;

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;

    use super::*;

    #[test]
    fn crc16_matches_doc_example_for_get_version() {
        let msg = [0xFF, 0x00, 0x03];
        assert_eq!(protocol_crc16(&msg), 0x1D0C);
    }

    #[test]
    fn build_get_version_packet_matches_example() {
        let p = HostCommand::get_version().unwrap();
        assert_eq!(p, vec![0xFF, 0x00, 0x03, 0x1D, 0x0C]);
    }

    #[test]
    fn parse_reader_frame_success() {
        let packet = [0xFF, 0x01, 0x0C, 0x00, 0x00, 0x12, 0x63, 0x43];
        let f = parse_reader_frame(&packet).unwrap();
        assert_eq!(f.command, 0x0C);
        assert_eq!(f.status_raw, 0x0000);
        assert_eq!(f.status, Some(StatusCode::Success));
        assert_eq!(f.data, vec![0x12]);
    }

    #[test]
    fn build_set_current_region_packet() {
        let p = HostCommand::set_current_region(RegionCode::NorthAmerica).unwrap();
        assert_eq!(p, vec![0xFF, 0x01, 0x97, 0x01, 0x4B, 0xBC]);
    }

    #[test]
    fn build_set_antenna_access_pair_packet() {
        let p = HostCommand::set_antenna_ports(&AntennaPortsConfiguration::AccessPair(
            AntennaPair { tx: 0x01, rx: 0x01 },
        ))
        .unwrap();
        assert_eq!(p, vec![0xFF, 0x03, 0x91, 0x00, 0x01, 0x01, 0x62, 0x87]);
    }

    #[test]
    fn async_stop_matches_document_example() {
        let p = HostCommand::async_stop().unwrap();
        assert_eq!(
            p,
            vec![
                0xFF, 0x0E, 0xAA, 0x4D, 0x6F, 0x64, 0x75, 0x6C, 0x65, 0x74, 0x65, 0x63, 0x68,
                0xAA, 0x49, 0xF3, 0xBB, 0x03, 0x91,
            ]
        );
    }

    #[test]
    fn parse_version_info_ok() {
        let data = [
            0x13, 0x04, 0x15, 0x00, 0xA8, 0x00, 0x00, 0x01, 0x20, 0x13, 0x05, 0x22, 0x13,
            0x05, 0x23, 0x00, 0x00, 0x00, 0x00, 0x10,
        ];
        let v = parse_version_info(&data).unwrap();
        assert_eq!(v.supported_protocol, [0x00, 0x00, 0x00, 0x10]);
    }

    #[test]
    fn parse_get_run_phase() {
        assert_eq!(parse_run_phase(&[0x11]).unwrap(), RunPhase::Bootloader);
        assert_eq!(parse_run_phase(&[0x12]).unwrap(), RunPhase::AppFirmware);
    }

    #[test]
    fn parse_get_frequency_hopping_table() {
        let data = [0x00, 0x0D, 0xF7, 0x32, 0x00, 0x0D, 0xC8, 0x52];
        let freqs = parse_frequency_hopping_table(&data).unwrap();
        assert_eq!(freqs, vec![915_250, 903_250]);
    }

    #[test]
    fn parse_get_antenna_ports_power() {
        let data = [0x03, 0x01, 0x0B, 0xB8, 0x0B, 0xB8];
        let parsed = parse_antenna_ports_response(AntennaPortsOption::Power, &data).unwrap();
        assert_eq!(
            parsed,
            AntennaPortsResponse::Power(vec![AntennaPower {
                tx: 0x01,
                read_power: 0x0BB8,
                write_power: 0x0BB8,
            }])
        );
    }

    #[derive(Debug)]
    struct TestTransport {
        rx: VecDeque<u8>,
        tx: Vec<u8>,
    }

    impl TestTransport {
        fn from_frames(frames: Vec<Vec<u8>>) -> Self {
            let mut rx = VecDeque::new();
            for frame in frames {
                rx.extend(frame);
            }
            Self { rx, tx: Vec::new() }
        }
    }

    impl ReaderTransport for TestTransport {
        type Error = &'static str;

        async fn write_all(&mut self, data: &[u8]) -> Result<(), Self::Error> {
            self.tx.extend_from_slice(data);
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

    fn frame(command: u8, status: u16, data: &[u8]) -> Vec<u8> {
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

    #[test]
    fn silion_reader_get_version_and_region() {
        let version = frame(
            CommandCode::GetVersion as u8,
            0x0000,
            &[
                0x13, 0x04, 0x15, 0x00, 0xA8, 0x00, 0x00, 0x01, 0x20, 0x13, 0x05, 0x22, 0x13,
                0x05, 0x23, 0x00, 0x00, 0x00, 0x00, 0x10,
            ],
        );
        let region = frame(CommandCode::GetCurrentRegion as u8, 0x0000, &[0x01]);

        let transport = TestTransport::from_frames(vec![version, region]);
        let mut reader = SilionReader::new(transport);

        let v = futures::executor::block_on(reader.get_version()).unwrap();
        assert_eq!(v.supported_protocol, [0, 0, 0, 0x10]);
        let r = futures::executor::block_on(reader.get_current_region()).unwrap();
        assert_eq!(r, RegionCode::NorthAmerica);
    }
}
