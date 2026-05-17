use crate::async_proto::parse_async_payload_owned;
use crate::client::{ClientError, ReaderClient};
use crate::codes::{AntennaPortsOption, CommandCode, RegionCode};
use crate::command::{
    AntennaPortsConfiguration, AsyncInventoryStartData as CommandAsyncInventoryStartData,
    AsyncSubcommandCode, InventoryOption, InventorySearchFlags, MemBank, MetadataFlags,
    SelectContent, SelectMode, SelectOptionBits,
};
use crate::error::ProtocolError;
use crate::parsers::{
    AntennaPortsResponse, ProtocolConfigurationValue, ReaderConfigurationValue, RegulatoryHopTime,
    RunPhase, SerialNumberInfo, TagEpcAndMetaData, VersionInfo, parse_antenna_ports_response,
    parse_available_regions, parse_current_region, parse_current_tag_protocol,
    parse_current_temperature, parse_frequency_hopping_table, parse_pin_states,
    parse_protocol_configuration_value, parse_reader_configuration_value,
    parse_regulatory_hop_time, parse_run_phase, parse_serial_number_info,
    parse_single_tag_inventory_response, parse_tag_epc_and_meta_data, parse_version_info,
};
use crate::session::AsyncInventorySession;
use crate::transport::ReaderTransport;

/// High-level reader API that returns typed values for common protocol operations.
pub struct SilionReader<T: ReaderTransport> {
    client: ReaderClient<T>,
}

/// One asynchronous inventory message pushed by the reader.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "web-serial", derive(serde::Serialize))]
#[cfg_attr(feature = "web-serial", serde(tag = "kind", rename_all = "camelCase"))]
pub enum AsyncInventoryMessage {
    /// Reader reply for Start AsyncInventory (`0xAA48`), no subcommand data.
    StartAck,
    /// Reader reply for Stop AsyncInventory (`0xAA49`), no subcommand data.
    StopAck,
    /// Unrequested tag information packet.
    TagInformation {
        /// Metadata flags echoed back from the start command.
        metadata_flags: MetadataFlags,
        /// Parsed tag EPC and metadata block.
        tag: TagEpcAndMetaData,
    },
    /// Unrequested heartbeat packet.
    Heartbeat {
        /// Search flags from heartbeat payload.
        search_flags: InventorySearchFlags,
        /// State data bytes after search flags.
        state_data: Vec<u8>,
    },
    /// Unrecognized asynchronous subcommand payload wrapped by `Moduletech` marker.
    Subcommand {
        /// Parsed asynchronous inventory subcommand code.
        subcommand: u16,
        /// Subcommand-specific payload bytes.
        subcommand_data: Vec<u8>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "web-serial", derive(serde::Serialize))]
#[cfg_attr(feature = "web-serial", derive(serde::Deserialize))]
#[cfg_attr(feature = "web-serial", serde(tag = "type", rename_all = "camelCase"))]
/// Tag singulation/select options used by inventory and access helpers.
pub enum SelectOption {
    /// Disable select/singulation and operate on the first matching tag.
    Disabled,
    /// Select against EPC data.
    Epc {
        /// Number of select bits.
        select_length_bits: u16,
        /// Raw select bytes.
        select_data: Vec<u8>,
        /// Invert select result (match non-equal tags).
        invert: bool,
    },
    /// Select against TID memory bank.
    Tid {
        /// Bit address within the bank.
        select_address: u32,
        /// Number of select bits.
        select_length_bits: u16,
        /// Raw select bytes.
        select_data: Vec<u8>,
        /// Invert select result (match non-equal tags).
        invert: bool,
    },
    /// Select against User memory bank.
    UserMemory {
        /// Bit address within the bank.
        select_address: u32,
        /// Number of select bits.
        select_length_bits: u16,
        /// Raw select bytes.
        select_data: Vec<u8>,
        /// Invert select result (match non-equal tags).
        invert: bool,
    },
    /// Select against EPC memory bank (Gen2 bank 0x01).
    EpcBank {
        /// Bit address within the bank.
        select_address: u32,
        /// Number of select bits.
        select_length_bits: u16,
        /// Raw select bytes.
        select_data: Vec<u8>,
        /// Invert select result (match non-equal tags).
        invert: bool,
    },
    /// Send only access password without select content.
    PasswordOnly,
}

impl SelectOption {
    fn into_option_content(self) -> (InventoryOption, Option<SelectContent>) {
        match self {
            SelectOption::Disabled => (InventoryOption::default(), None),
            SelectOption::Epc {
                select_length_bits,
                select_data,
                invert,
            } => (
                SelectOptionBits::new(SelectMode::Epc)
                    .with_invert_flag(invert)
                    .with_extended_data_length(select_length_bits > 255)
                    .into(),
                Some(SelectContent {
                    address_bits: 0,
                    bit_len: select_length_bits,
                    data: select_data,
                }),
            ),
            SelectOption::Tid {
                select_address,
                select_length_bits,
                select_data,
                invert,
            } => (
                SelectOptionBits::new(SelectMode::Tid)
                    .with_invert_flag(invert)
                    .with_extended_data_length(select_length_bits > 255)
                    .into(),
                Some(SelectContent {
                    address_bits: select_address,
                    bit_len: select_length_bits,
                    data: select_data,
                }),
            ),
            SelectOption::UserMemory {
                select_address,
                select_length_bits,
                select_data,
                invert,
            } => (
                SelectOptionBits::new(SelectMode::UserMemory)
                    .with_invert_flag(invert)
                    .with_extended_data_length(select_length_bits > 255)
                    .into(),
                Some(SelectContent {
                    address_bits: select_address,
                    bit_len: select_length_bits,
                    data: select_data,
                }),
            ),
            SelectOption::EpcBank {
                select_address,
                select_length_bits,
                select_data,
                invert,
            } => (
                SelectOptionBits::new(SelectMode::EpcBank)
                    .with_invert_flag(invert)
                    .with_extended_data_length(select_length_bits > 255)
                    .into(),
                Some(SelectContent {
                    address_bits: select_address,
                    bit_len: select_length_bits,
                    data: select_data,
                }),
            ),
            SelectOption::PasswordOnly => {
                (SelectOptionBits::new(SelectMode::PasswordOnly).into(), None)
            }
        }
    }
}

/// Start configuration for asynchronous inventory.
///
/// This wrapper uses [`SelectOption`] so callers do not need to manually map
/// to low-level `InventoryOption + SelectContent` fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReaderAsyncInventoryStartData {
    /// Metadata flags controlling which per-tag fields the reader returns.
    pub metadata_flags: MetadataFlags,
    /// Tag singulation/select configuration.
    pub select_option: SelectOption,
    /// Search flags (2 bytes), same meaning as command `0x22`.
    pub search_flags: InventorySearchFlags,
    /// Optional access password (4 bytes) when required by option.
    pub access_password: Option<u32>,
    /// Optional typed embedded command content.
    pub embedded_command_content: Option<crate::InventoryEmbeddedCommandContent>,
}

impl ReaderAsyncInventoryStartData {
    fn to_command_data(&self) -> CommandAsyncInventoryStartData {
        let (option, select_content) = self.select_option.clone().into_option_content();
        CommandAsyncInventoryStartData {
            metadata_flags: self.metadata_flags,
            option,
            search_flags: self.search_flags,
            access_password: self.access_password,
            select_content,
            embedded_command_content: self.embedded_command_content.clone(),
        }
    }
}

impl<T: ReaderTransport> SilionReader<T> {
    /// Create a high-level reader API from a transport.
    pub fn new(transport: T) -> Self {
        Self {
            client: ReaderClient::new(transport),
        }
    }

    /// Create a high-level reader API from an existing low-level client.
    pub fn from_client(client: ReaderClient<T>) -> Self {
        Self { client }
    }

    /// Consume the reader API and return the wrapped transport.
    pub fn into_inner(self) -> T {
        self.client.into_inner()
    }

    /// Return a mutable reference to the wrapped transport.
    pub fn transport_mut(&mut self) -> &mut T {
        self.client.transport_mut()
    }

    /// Consume the reader API and return the wrapped low-level client.
    pub fn into_client(self) -> ReaderClient<T> {
        self.client
    }

    /// Run a raw command transaction and return the validated response frame.
    ///
    /// This is useful for callers that need command coverage beyond the typed
    /// helper methods on `SilionReader`.
    pub async fn transact_raw(
        &mut self,
        command: u8,
        data: &[u8],
    ) -> Result<crate::ReaderFrame, ClientError<T::Error>> {
        self.client.transact(command, data).await
    }

    /// Convert this host into an awaited-read [`AsyncInventorySession`].
    pub fn into_async_session(self) -> AsyncInventorySession<T> {
        AsyncInventorySession::new(self.client)
    }

    /// Run command `0x03` (Get Version) and parse version fields.
    pub async fn get_version(&mut self) -> Result<VersionInfo, ClientError<T::Error>> {
        let frame = self
            .client
            .transact(CommandCode::GetVersion as u8, &[])
            .await?;
        parse_version_info(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x04` (Boot Firmware).
    pub async fn boot_firmware(&mut self) -> Result<(), ClientError<T::Error>> {
        let _ = self
            .client
            .transact(CommandCode::BootFirmware as u8, &[])
            .await?;
        Ok(())
    }

    /// Run command `0x09` (Boot Bootloader).
    pub async fn boot_bootloader(&mut self) -> Result<(), ClientError<T::Error>> {
        let _ = self
            .client
            .transact(CommandCode::BootBootloader as u8, &[])
            .await?;
        Ok(())
    }

    /// Run command `0x0C` (Get Run Phase) and parse phase enum.
    pub async fn get_run_phase(&mut self) -> Result<RunPhase, ClientError<T::Error>> {
        let frame = self
            .client
            .transact(CommandCode::GetRunPhase as u8, &[])
            .await?;
        parse_run_phase(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x10` (Get Serial Number).
    pub async fn get_serial_number(
        &mut self,
        option: u8,
        data_flags: u8,
    ) -> Result<SerialNumberInfo, ClientError<T::Error>> {
        let frame = self
            .client
            .transact(CommandCode::GetSerialNumber as u8, &[option, data_flags])
            .await?;
        parse_serial_number_info(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x63` (Get Current Tag Protocol).
    pub async fn get_current_tag_protocol(&mut self) -> Result<u16, ClientError<T::Error>> {
        let frame = self
            .client
            .transact(CommandCode::GetCurrentTagProtocol as u8, &[])
            .await?;
        parse_current_tag_protocol(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x97` (Set Current Region).
    pub async fn set_current_region(
        &mut self,
        region_code: RegionCode,
    ) -> Result<(), ClientError<T::Error>> {
        let _ = self
            .client
            .transact(CommandCode::SetCurrentRegion as u8, &[region_code.as_u8()])
            .await?;
        Ok(())
    }

    /// Run command `0x67` (Get Current Region).
    pub async fn get_current_region(&mut self) -> Result<RegionCode, ClientError<T::Error>> {
        let frame = self
            .client
            .transact(CommandCode::GetCurrentRegion as u8, &[])
            .await?;
        parse_current_region(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x71` (Get Available Regions).
    pub async fn get_available_regions(
        &mut self,
    ) -> Result<Vec<RegionCode>, ClientError<T::Error>> {
        let frame = self
            .client
            .transact(CommandCode::GetAvailableRegions as u8, &[])
            .await?;
        parse_available_regions(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x72` (Get Current Temperature).
    pub async fn get_current_temperature(&mut self) -> Result<u8, ClientError<T::Error>> {
        let frame = self
            .client
            .transact(CommandCode::GetCurrentTemperature as u8, &[])
            .await?;
        parse_current_temperature(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x66` (Get GPI) and return pin states.
    pub async fn get_gpi(&mut self) -> Result<Vec<u8>, ClientError<T::Error>> {
        let frame = self.client.transact(CommandCode::GetGpi as u8, &[]).await?;
        parse_pin_states(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x96` with empty payload to read GPO states.
    pub async fn get_gpo_states(&mut self) -> Result<Vec<u8>, ClientError<T::Error>> {
        let frame = self.client.transact(CommandCode::SetGpo as u8, &[]).await?;
        parse_pin_states(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x91` (Set Antenna Ports).
    pub async fn set_antenna_ports(
        &mut self,
        config: &AntennaPortsConfiguration,
    ) -> Result<(), ClientError<T::Error>> {
        let packet = crate::command::HostCommand::set_antenna_ports(config)
            .map_err(ClientError::Protocol)?;
        let _ = self.client.transact_frame(&packet).await?;
        Ok(())
    }

    /// Send command `0xAA48` to enable asynchronous inventory.
    pub async fn enable_async_inventory(
        &mut self,
        start: &ReaderAsyncInventoryStartData,
    ) -> Result<(), ClientError<T::Error>> {
        let command_data = start.to_command_data();
        let packet = crate::command::HostCommand::async_start(&command_data)
            .map_err(ClientError::Protocol)?;
        let response = self.client.transact_frame(&packet).await?;
        if response.command != CommandCode::AsynchronousInventory as u8 {
            return Err(ClientError::UnexpectedResponseCommand {
                expected: CommandCode::AsynchronousInventory as u8,
                actual: response.command,
            });
        }
        if response.status_raw != 0x0000 {
            return Err(ClientError::ReaderStatus {
                status_raw: response.status_raw,
                status: response.status,
            });
        }
        Ok(())
    }

    /// Send command `0xAA49` to disable asynchronous inventory.
    pub async fn disable_async_inventory(&mut self) -> Result<(), ClientError<T::Error>> {
        let packet = crate::command::HostCommand::async_stop().map_err(ClientError::Protocol)?;
        let response = self.client.transact_frame(&packet).await?;
        if response.command != CommandCode::AsynchronousInventory as u8 {
            return Err(ClientError::UnexpectedResponseCommand {
                expected: CommandCode::AsynchronousInventory as u8,
                actual: response.command,
            });
        }
        if response.status_raw != 0x0000 {
            return Err(ClientError::ReaderStatus {
                status_raw: response.status_raw,
                status: response.status,
            });
        }
        Ok(())
    }

    /// Receive one pushed asynchronous inventory message from the reader.
    pub async fn recv_async_inventory_message(
        &mut self,
    ) -> Result<AsyncInventoryMessage, ClientError<T::Error>> {
        let frame = self.client.read_frame().await?;
        if frame.command != CommandCode::AsynchronousInventory as u8 {
            return Err(ClientError::UnexpectedResponseCommand {
                expected: CommandCode::AsynchronousInventory as u8,
                actual: frame.command,
            });
        }
        if frame.status_raw != 0x0000 {
            return Err(ClientError::ReaderStatus {
                status_raw: frame.status_raw,
                status: frame.status,
            });
        }
        parse_async_frame_data(&frame.data).map_err(ClientError::Protocol)
    }

    /// Perform a single tag inventory read using command `0x21` (Single Tag Inventory).
    ///
    /// This command inventories one tag within the specified timeout and returns the
    /// parsed tag data with requested metadata.
    ///
    /// # Arguments
    ///
    /// * `timeout_ms` - Maximum time to wait for a tag response in milliseconds
    /// * `option` - Select option flags (see [`SelectOptionBits`])
    /// * `metadata_flags` - Which metadata fields to include in the response
    /// * `select_content` - Optional tag singulation/select rule
    ///
    /// Returns the tag EPC and metadata, or an error if no tag is found or timeout occurs.
    pub async fn single_tag_inventory(
        &mut self,
        timeout_ms: u16,
        select_option: SelectOption,
        metadata_flags: Option<MetadataFlags>,
    ) -> Result<TagEpcAndMetaData, ClientError<T::Error>> {
        // Derive option bit 4 (`0x10`) from metadata presence.
        let (option, select_content) = select_option.into_option_content();
        let option = option.with_single_tag_metadata(metadata_flags.is_some());

        let packet = crate::command::HostCommand::single_tag_inventory(
            timeout_ms,
            option,
            metadata_flags,
            select_content,
        )
        .map_err(ClientError::Protocol)?;

        let frame = self.client.transact_frame(&packet).await?;
        if frame.command != CommandCode::SingleTagInventory as u8 {
            return Err(ClientError::UnexpectedResponseCommand {
                expected: CommandCode::SingleTagInventory as u8,
                actual: frame.command,
            });
        }
        if frame.status_raw != 0x0000 {
            return Err(ClientError::ReaderStatus {
                status_raw: frame.status_raw,
                status: frame.status,
            });
        }

        let (_response_metadata_flags, tag) =
            parse_single_tag_inventory_response(option.raw(), &frame.data)
                .map_err(ClientError::Protocol)?;
        Ok(tag)
    }

    /// Run command `0x28` (Read Tag Data) and parse the returned tag payload.
    ///
    /// This helper performs a single-tag read against the requested memory bank,
    /// optionally including metadata fields in the response.
    ///
    /// # Arguments
    ///
    /// * `timeout_ms` - Maximum time to wait for a tag response in milliseconds
    /// * `select_option` - Tag singulation/select rule applied before the read
    /// * `metadata_flags` - Optional metadata fields to request alongside tag data
    /// * `read_membank` - Memory bank to read from (Gen2 0x01=EPC, 0x02=TID, 0x03=User)
    /// * `read_address_words` - Start address in 16-bit words within `read_membank`
    /// * `word_count` - Number of 16-bit words to read (protocol range `1..=96`)
    ///
    /// Returns parsed tag data and metadata when enabled, or an error if the command
    /// fails, times out, or the reader returns a non-success status.
    pub async fn read_tag_data(
        &mut self,
        timeout_ms: u16,
        select_option: SelectOption,
        metadata_flags: Option<MetadataFlags>,
        read_membank: MemBank,
        read_address_words: u32,
        word_count: u8,
    ) -> Result<TagEpcAndMetaData, ClientError<T::Error>> {
        let (option, select_content) = select_option.into_option_content();
        let option = option.with_single_tag_metadata(metadata_flags.is_some());

        let packet = crate::command::HostCommand::read_tag_data(
            timeout_ms,
            option.raw(),
            metadata_flags,
            read_membank,
            read_address_words,
            word_count,
            None,
            select_content,
        )
        .map_err(ClientError::Protocol)?;

        let frame = self.client.transact_frame(&packet).await?;
        if frame.command != CommandCode::ReadTagData as u8 {
            return Err(ClientError::UnexpectedResponseCommand {
                expected: CommandCode::ReadTagData as u8,
                actual: frame.command,
            });
        }
        if frame.status_raw != 0x0000 {
            return Err(ClientError::ReaderStatus {
                status_raw: frame.status_raw,
                status: frame.status,
            });
        }

        let returned_options = InventoryOption::from_raw(frame.data[0]);
        if returned_options.single_tag_metadata_enabled() {
            return parse_tag_epc_and_meta_data(
                metadata_flags.unwrap_or(MetadataFlags::NONE),
                &frame.data,
            )
            .map_err(ClientError::Protocol);
        } else {
            let frame_data = frame.data[1..].to_vec();
            return Ok(TagEpcAndMetaData {
                read_count: None,
                rssi_dbm: None,
                antenna_id: None,
                frequency_khz: None,
                timestamp_ms: None,
                rfu: None,
                protocol_id: None,
                tag_data_bit_length: Some(frame_data.len() as u16 * 8),
                tag_data: Some(frame_data),
                epc_bit_length: None,
                pc_word: None,
                epc_id: Vec::new(),
                tag_crc: 0,
            });
        }
    }

    /// Run command `0x65` (Get Frequency Hopping table form).
    pub async fn get_frequency_hopping_table(&mut self) -> Result<Vec<u32>, ClientError<T::Error>> {
        let frame = self
            .client
            .transact(CommandCode::GetFrequencyHopping as u8, &[])
            .await?;
        parse_frequency_hopping_table(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x65` with option `0x01` (Regulatory Hopping Time).
    pub async fn get_regulatory_hop_time(
        &mut self,
    ) -> Result<RegulatoryHopTime, ClientError<T::Error>> {
        let frame = self
            .client
            .transact(CommandCode::GetFrequencyHopping as u8, &[0x01])
            .await?;
        parse_regulatory_hop_time(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x61` (Get Antenna Ports) and decode by option.
    pub async fn get_antenna_ports(
        &mut self,
        option: AntennaPortsOption,
    ) -> Result<AntennaPortsResponse, ClientError<T::Error>> {
        let frame = self
            .client
            .transact(CommandCode::GetAntennaPorts as u8, &[option.as_u8()])
            .await?;
        parse_antenna_ports_response(option, &frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x6A` (Get Reader Configuration).
    pub async fn get_reader_configuration(
        &mut self,
        option: u8,
        key: u8,
    ) -> Result<ReaderConfigurationValue, ClientError<T::Error>> {
        let frame = self
            .client
            .transact(CommandCode::GetReaderConfiguration as u8, &[option, key])
            .await?;
        parse_reader_configuration_value(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x6B` (Get Protocol Configuration).
    pub async fn get_protocol_configuration(
        &mut self,
        protocol_value: u8,
        parameter: u8,
    ) -> Result<ProtocolConfigurationValue, ClientError<T::Error>> {
        let frame = self
            .client
            .transact(
                CommandCode::GetProtocolConfiguration as u8,
                &[protocol_value, parameter],
            )
            .await?;
        parse_protocol_configuration_value(&frame.data).map_err(ClientError::Protocol)
    }
}

/// Parse the data bytes of a validated `0xAA` asynchronous inventory frame
/// into a typed [`AsyncInventoryMessage`].
pub(crate) fn parse_async_frame_data(data: &[u8]) -> Result<AsyncInventoryMessage, ProtocolError> {
    if data.starts_with(b"Moduletech") {
        let payload = parse_async_payload_owned(data)?;
        return Ok(match payload.subcommand {
            x if x == AsyncSubcommandCode::Start as u16 => AsyncInventoryMessage::StartAck,
            x if x == AsyncSubcommandCode::Stop as u16 => AsyncInventoryMessage::StopAck,
            _ => AsyncInventoryMessage::Subcommand {
                subcommand: payload.subcommand,
                subcommand_data: payload.subcommand_data,
            },
        });
    }

    if data.starts_with(b"XTSJ") {
        if data.len() < 6 {
            return Err(ProtocolError::InvalidResponse(
                "heartbeat payload too short",
            ));
        }
        let search_flags = InventorySearchFlags::from_raw(u16::from_be_bytes([data[4], data[5]]));
        return Ok(AsyncInventoryMessage::Heartbeat {
            search_flags,
            state_data: data[6..].to_vec(),
        });
    }

    if data.len() < 2 {
        return Err(ProtocolError::InvalidResponse(
            "tag information payload too short",
        ));
    }

    let metadata_flags = MetadataFlags::from_raw(u16::from_be_bytes([data[0], data[1]]));

    Ok(AsyncInventoryMessage::TagInformation {
        metadata_flags,
        tag: parse_tag_epc_and_meta_data(metadata_flags, &data[2..])?,
    })
}

#[cfg(test)]
mod tests {
    use super::SilionReader;
    use crate::codes::CommandCode;
    use crate::test_support::{MockInteraction, MockTransport, reply_frame};
    use crate::{AsyncInventoryMessage, InventorySearchFlags, RegionCode};

    #[test]
    fn get_version_and_region() {
        let version_data = vec![
            0x13, 0x04, 0x15, 0x00, 0xA8, 0x00, 0x00, 0x01, 0x20, 0x13, 0x05, 0x22, 0x13, 0x05,
            0x23, 0x00, 0x00, 0x00, 0x00, 0x10,
        ];
        let transport = MockTransport::scripted(vec![
            MockInteraction {
                request_command: CommandCode::GetVersion as u8,
                response_status: 0x0000,
                response_data: version_data,
            },
            MockInteraction {
                request_command: CommandCode::GetCurrentRegion as u8,
                response_status: 0x0000,
                response_data: vec![0x01],
            },
        ]);

        let mut reader = SilionReader::new(transport);
        let version =
            futures::executor::block_on(reader.get_version()).expect("version should parse");
        assert_eq!(version.supported_protocol, [0x00, 0x00, 0x00, 0x10]);

        let region =
            futures::executor::block_on(reader.get_current_region()).expect("region should parse");
        assert_eq!(region, RegionCode::NorthAmerica);
    }

    #[test]
    fn recv_async_inventory_heartbeat() {
        let mut data = b"XTSJ".to_vec();
        data.extend_from_slice(&0x8000u16.to_be_bytes());
        data.push(0x01);
        let packet = reply_frame(0xAA, 0x0000, &data);
        let transport = MockTransport::from_replies(vec![packet]);

        let mut reader = SilionReader::new(transport);
        let message = futures::executor::block_on(reader.recv_async_inventory_message())
            .expect("async message should parse");

        match message {
            AsyncInventoryMessage::Heartbeat {
                search_flags,
                state_data,
            } => {
                assert_eq!(search_flags, InventorySearchFlags::from_raw(0x8000));
                assert_eq!(state_data, vec![0x01]);
            }
            other => panic!("unexpected async message: {other:?}"),
        }
    }

    #[test]
    #[cfg(feature = "web-serial")]
    fn tag_information_serialization() {
        use crate::command::MetadataFlags;
        use crate::parsers::TagEpcAndMetaData;

        let tag = TagEpcAndMetaData {
            read_count: Some(1),
            rssi_dbm: Some(-50),
            antenna_id: Some(1),
            frequency_khz: Some(902250),
            timestamp_ms: Some(1000),
            rfu: None,
            protocol_id: Some(5),
            tag_data_bit_length: None,
            tag_data: None,
            epc_bit_length: 96,
            pc_word: 0x3000,
            epc_id: vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06],
            tag_crc: 0x1234,
        };

        let msg = AsyncInventoryMessage::TagInformation {
            metadata_flags: MetadataFlags::from_raw(0x00FF),
            tag,
        };

        let json_str = serde_json::to_string(&msg).expect("should serialize to JSON");
        let json_value: serde_json::Value =
            serde_json::from_str(&json_str).expect("should parse JSON");

        // Check that the message has the expected kind
        assert_eq!(
            json_value.get("kind").and_then(|v| v.as_str()),
            Some("tagInformation")
        );

        // With no `flatten`, tag data must be nested under `tag`.
        let tag_obj = json_value
            .get("tag")
            .and_then(|v| v.as_object())
            .expect("tag should be an object");
        assert!(
            tag_obj.get("epcId").is_some(),
            "tag.epcId should be present"
        );
        assert!(
            tag_obj.get("readCount").is_some(),
            "tag.readCount should be present"
        );
        assert!(
            tag_obj.get("rssiDbm").is_some(),
            "tag.rssiDbm should be present"
        );
        assert!(
            tag_obj.get("antennaId").is_some(),
            "tag.antennaId should be present"
        );
        assert!(
            tag_obj.get("frequencyKhz").is_some(),
            "tag.frequencyKhz should be present"
        );
        assert!(
            tag_obj.get("timestampMs").is_some(),
            "tag.timestampMs should be present"
        );
        assert!(
            tag_obj.get("protocolId").is_some(),
            "tag.protocolId should be present"
        );
        assert!(
            tag_obj.get("epcBitLength").is_some(),
            "tag.epcBitLength should be present"
        );
        assert!(
            tag_obj.get("pcWord").is_some(),
            "tag.pcWord should be present"
        );
        assert!(
            tag_obj.get("tagCrc").is_some(),
            "tag.tagCrc should be present"
        );
    }

    #[test]
    fn single_tag_inventory_success() {
        use crate::command::MetadataFlags;
        use crate::silion_reader::SelectOption;

        // Construct `0x21` response data in metadata mode:
        // Option (1) + MetadataFlags (2) + EPC ID + Tag CRC
        let mut response_data = Vec::new();

        // Option (bit 4 set: metadata mode)
        response_data.push(0x10);
        // MetadataFlags (none)
        response_data.extend_from_slice(&0x0000u16.to_be_bytes());

        // EPC ID: 6 bytes
        response_data.extend_from_slice(&[0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);

        // Tag CRC: 0x1234
        response_data.extend_from_slice(&0x1234u16.to_be_bytes());

        let transport = MockTransport::from_replies(vec![reply_frame(
            CommandCode::SingleTagInventory as u8,
            0x0000,
            &response_data,
        )]);

        let mut reader = SilionReader::new(transport);
        let tag = futures::executor::block_on(reader.single_tag_inventory(
            5000,                                  // 5 second timeout
            SelectOption::Disabled,                // option
            Some(MetadataFlags::from_raw(0x0000)), // no metadata fields
        ))
        .expect("single tag inventory should succeed");

        assert_eq!(tag.pc_word, Some(0x0000));
        assert_eq!(tag.epc_bit_length, Some(48));
        assert_eq!(tag.epc_id, vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06]);
        assert_eq!(tag.tag_crc, 0x1234);
        assert_eq!(tag.read_count, None); // Not requested
        assert_eq!(tag.rssi_dbm, None); // Not requested
    }
}
