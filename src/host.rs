use crate::client::{ClientError, ReaderClient};
use crate::codes::{AntennaPortsOption, CommandCode, RegionCode};
use crate::command::{
    AntennaPortsConfiguration, AsyncInventoryStartData, AsyncSubcommandCode,
    InventorySearchFlags, MetadataFlags,
};
use crate::error::ProtocolError;
use crate::async_proto::parse_async_payload_owned;
use crate::parsers::{
    parse_tag_epc_and_meta_data,
    parse_antenna_ports_response, parse_available_regions, parse_current_region,
    parse_current_tag_protocol, parse_current_temperature, parse_frequency_hopping_table,
    parse_pin_states, parse_protocol_configuration_value, parse_reader_configuration_value,
    parse_regulatory_hop_time, parse_run_phase, parse_serial_number_info, parse_version_info,
    AntennaPortsResponse, ProtocolConfigurationValue, ReaderConfigurationValue, TagEpcAndMetaData,
    RegulatoryHopTime, RunPhase, SerialNumberInfo, VersionInfo,
};
use crate::transport::ReaderTransport;
use crate::session::AsyncInventorySession;

/// High-level host API that returns typed values for common protocol operations.
pub struct SilionHost<T: ReaderTransport> {
    client: ReaderClient<T>,
}

/// One asynchronous inventory message pushed by the reader.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AsyncInventoryMessage {
    /// Reader reply for Start AsyncInventory (`0xAA48`), no subcommand data.
    StartAck,
    /// Reader reply for Stop AsyncInventory (`0xAA49`), no subcommand data.
    StopAck,
    /// Unrequested tag information packet.
    ///
    /// Format: `MetadataFlags(2) | Tag EPC and Meta Data (N)`.
    TagInformation {
        /// Metadata flags echoed back from the start command.
        metadata_flags: MetadataFlags,
        /// Parsed tag EPC and metadata block.
        tag: TagEpcAndMetaData,
    },
    /// Unrequested heartbeat packet.
    ///
    /// Format: `"XTSJ"(4) | SearchFlags(2) | StateData(N)`.
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

impl<T: ReaderTransport> SilionHost<T> {
    /// Create a high-level host API from a transport.
    pub fn new(transport: T) -> Self {
        Self {
            client: ReaderClient::new(transport),
        }
    }

    /// Create a high-level host API from an existing low-level client.
    pub fn from_client(client: ReaderClient<T>) -> Self {
        Self { client }
    }

    /// Consume the host API and return the wrapped transport.
    pub fn into_inner(self) -> T {
        self.client.into_inner()
    }

    /// Return a mutable reference to the wrapped transport.
    ///
    /// Use this to reconfigure transport parameters (for example the read
    /// timeout on a [`SerialPortTransport`][crate::serial::SerialPortTransport])
    /// between operations.
    pub fn transport_mut(&mut self) -> &mut T {
        self.client.transport_mut()
    }

    /// Consume the host API and return the wrapped low-level client.
    pub fn into_client(self) -> ReaderClient<T> {
        self.client
    }

    /// Convert this host into a background-thread [`AsyncInventorySession`].
    ///
    /// The caller must have already sent the async inventory start command via
    /// [`enable_async_inventory`][Self::enable_async_inventory] before calling
    /// this. Ownership of the transport passes to the background reader thread.
    ///
    /// Call [`AsyncInventorySession::stop`] to send the `0xAA49` stop command,
    /// drain remaining frames, and recover the transport as a new `SilionHost`.
    pub fn into_async_session(self) -> AsyncInventorySession<T>
    where
        T: Send + 'static,
        T::Error: Send + 'static,
    {
        AsyncInventorySession::spawn(self.client)
    }

    /// Run command `0x03` (Get Version) and parse version fields.
    ///
    /// # Examples
    /// ```rust
    /// use rfidlibrs::{CommandCode, SilionHost};
    /// use rfidlibrs::test_support::{MockInteraction, MockTransport};
    ///
    /// let data = [
    ///     0x13, 0x04, 0x15, 0x00, 0xA8, 0x00, 0x00, 0x01,
    ///     0x20, 0x13, 0x05, 0x22, 0x13, 0x05, 0x23, 0x00,
    ///     0x00, 0x00, 0x00, 0x10,
    /// ];
    /// let transport = MockTransport::scripted(vec![MockInteraction {
    ///     request_command: CommandCode::GetVersion as u8,
    ///     response_status: 0x0000,
    ///     response_data: data.to_vec(),
    /// }]);
    /// let mut host = SilionHost::new(transport);
    /// let v = host.get_version().unwrap();
    /// assert_eq!(v.supported_protocol, [0x00, 0x00, 0x00, 0x10]);
    /// ```
    pub fn get_version(&mut self) -> Result<VersionInfo, ClientError<T::Error>> {
        let frame = self.client.transact(CommandCode::GetVersion as u8, &[])?;
        parse_version_info(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x04` (Boot Firmware).
    pub fn boot_firmware(&mut self) -> Result<(), ClientError<T::Error>> {
        let _ = self.client.transact(CommandCode::BootFirmware as u8, &[])?;
        Ok(())
    }

    /// Run command `0x09` (Boot Bootloader).
    pub fn boot_bootloader(&mut self) -> Result<(), ClientError<T::Error>> {
        let _ = self.client.transact(CommandCode::BootBootloader as u8, &[])?;
        Ok(())
    }

    /// Run command `0x0C` (Get Run Phase) and parse phase enum.
    ///
    /// # Examples
    /// ```rust
    /// use rfidlibrs::{CommandCode, RunPhase, SilionHost};
    /// use rfidlibrs::test_support::{MockInteraction, MockTransport};
    ///
    /// let transport = MockTransport::scripted(vec![MockInteraction {
    ///     request_command: CommandCode::GetRunPhase as u8,
    ///     response_status: 0x0000,
    ///     response_data: vec![0x12],
    /// }]);
    /// let mut host = SilionHost::new(transport);
    /// assert_eq!(host.get_run_phase().unwrap(), RunPhase::AppFirmware);
    /// ```
    pub fn get_run_phase(&mut self) -> Result<RunPhase, ClientError<T::Error>> {
        let frame = self.client.transact(CommandCode::GetRunPhase as u8, &[])?;
        parse_run_phase(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x10` (Get Serial Number).
    pub fn get_serial_number(
        &mut self,
        option: u8,
        data_flags: u8,
    ) -> Result<SerialNumberInfo, ClientError<T::Error>> {
        let frame = self
            .client
            .transact(CommandCode::GetSerialNumber as u8, &[option, data_flags])?;
        parse_serial_number_info(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x63` (Get Current Tag Protocol).
    pub fn get_current_tag_protocol(&mut self) -> Result<u16, ClientError<T::Error>> {
        let frame = self
            .client
            .transact(CommandCode::GetCurrentTagProtocol as u8, &[])?;
        parse_current_tag_protocol(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x97` (Set Current Region).
    ///
    /// Region code values are documented in the Set Current Region section.
    ///
    /// # Examples
    /// ```rust
    /// use rfidlibrs::{CommandCode, RegionCode, SilionHost};
    /// use rfidlibrs::test_support::{MockInteraction, MockTransport};
    ///
    /// let transport = MockTransport::scripted(vec![MockInteraction {
    ///     request_command: CommandCode::SetCurrentRegion as u8,
    ///     response_status: 0x0000,
    ///     response_data: vec![],
    /// }]);
    /// let mut host = SilionHost::new(transport);
    /// host.set_current_region(RegionCode::NorthAmerica).unwrap();
    /// ```
    pub fn set_current_region(
        &mut self,
        region_code: RegionCode,
    ) -> Result<(), ClientError<T::Error>> {
        let _ = self
            .client
            .transact(CommandCode::SetCurrentRegion as u8, &[region_code.as_u8()])?;
        Ok(())
    }

    /// Run command `0x67` (Get Current Region).
    pub fn get_current_region(&mut self) -> Result<RegionCode, ClientError<T::Error>> {
        let frame = self
            .client
            .transact(CommandCode::GetCurrentRegion as u8, &[])?;
        parse_current_region(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x71` (Get Available Regions).
    pub fn get_available_regions(&mut self) -> Result<Vec<RegionCode>, ClientError<T::Error>> {
        let frame = self
            .client
            .transact(CommandCode::GetAvailableRegions as u8, &[])?;
        parse_available_regions(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x72` (Get Current Temperature).
    pub fn get_current_temperature(&mut self) -> Result<u8, ClientError<T::Error>> {
        let frame = self
            .client
            .transact(CommandCode::GetCurrentTemperature as u8, &[])?;
        parse_current_temperature(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x66` (Get GPI) and return pin states.
    pub fn get_gpi(&mut self) -> Result<Vec<u8>, ClientError<T::Error>> {
        let frame = self.client.transact(CommandCode::GetGpi as u8, &[])?;
        parse_pin_states(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x96` with empty payload to read GPO states.
    pub fn get_gpo_states(&mut self) -> Result<Vec<u8>, ClientError<T::Error>> {
        let frame = self.client.transact(CommandCode::SetGpo as u8, &[])?;
        parse_pin_states(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x91` (Set Antenna Ports).
    pub fn set_antenna_ports(
        &mut self,
        config: &AntennaPortsConfiguration,
    ) -> Result<(), ClientError<T::Error>> {
        let packet = crate::command::HostCommand::set_antenna_ports(config)
            .map_err(ClientError::Protocol)?;
        let _ = self.client.transact_frame(&packet)?;
        Ok(())
    }

    /// Send command `0xAA48` to enable asynchronous inventory.
    ///
    /// After success, the reader can start pushing asynchronous `0xAA` frames.
    ///
    /// # Examples
    /// ```rust
    /// use rfidlibrs::{
    ///     AsyncInventoryStartData, CommandCode, EmbeddedReadTagData,
    ///     InventoryEmbeddedCommandContent, InventoryOption, InventorySearchFlags,
    ///     MemBank, MetadataFlags, SilionHost,
    /// };
    /// use rfidlibrs::test_support::{MockInteraction, MockTransport};
    ///
    /// let transport = MockTransport::scripted(vec![MockInteraction {
    ///     request_command: CommandCode::AsynchronousInventory as u8,
    ///     response_status: 0x0000,
    ///     response_data: b"Moduletech\xAA\x48".to_vec(),
    /// }]);
    ///
    /// let search_flags = InventorySearchFlags::new()
    ///     .with_async_heartbeat(true)
    ///     .with_async_auto_stop(false)
    ///     .with_embedded_command(true)
    ///     .with_async_rest_ratio_steps(3)
    ///     .unwrap();
    /// let start = AsyncInventoryStartData {
    ///     metadata_flags: MetadataFlags::ALL,
    ///     option: InventoryOption::default(),
    ///     search_flags,
    ///     access_password: None,
    ///     select_content: None,
    ///     embedded_command_content: Some(InventoryEmbeddedCommandContent::ReadTagData(
    ///         EmbeddedReadTagData {
    ///             read_membank: MemBank::Tid,
    ///             read_address_words: 0,
    ///             word_count: 2,
    ///         },
    ///     )),
    /// };
    ///
    /// let mut host = SilionHost::new(transport);
    /// host.enable_async_inventory(&start).unwrap();
    /// ```
    pub fn enable_async_inventory(
        &mut self,
        start: &AsyncInventoryStartData,
    ) -> Result<(), ClientError<T::Error>> {
        let packet = crate::command::HostCommand::async_start(start)
            .map_err(ClientError::Protocol)?;
        let response = self.client.transact_frame(&packet)?;
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
    pub fn disable_async_inventory(&mut self) -> Result<(), ClientError<T::Error>> {
        let packet = crate::command::HostCommand::async_stop().map_err(ClientError::Protocol)?;
        let response = self.client.transact_frame(&packet)?;
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
    ///
    /// This method does not send any command. It waits for the next frame and
    /// expects command `0xAA` with success status. The returned enum differentiates
    /// Start/Stop acknowledgements, tag-information packets, heartbeat packets,
    /// and unknown wrapped subcommands.
    ///
    /// # Examples
    /// ```rust
    /// use rfidlibrs::{
    ///     AsyncInventoryMessage, InventorySearchFlags, SilionHost,
    /// };
    /// use rfidlibrs::test_support::{reply_frame, MockTransport};
    ///
    /// // Build one heartbeat packet: "XTSJ" + search_flags(2) + state_data(1)
    /// let mut data = b"XTSJ".to_vec();
    /// data.extend_from_slice(&0x8000u16.to_be_bytes());
    /// data.push(0x01);
    /// let packet = reply_frame(0xAA, 0x0000, &data);
    ///
    /// let transport = MockTransport::from_replies(vec![packet]);
    /// let mut host = SilionHost::new(transport);
    ///
    /// match host.recv_async_inventory_message().unwrap() {
    ///     AsyncInventoryMessage::Heartbeat { search_flags, state_data } => {
    ///         assert_eq!(search_flags, InventorySearchFlags::from_raw(0x8000));
    ///         assert_eq!(state_data, vec![0x01]);
    ///     }
    ///     other => panic!("unexpected async message: {other:?}"),
    /// }
    /// ```
    pub fn recv_async_inventory_message(
        &mut self,
    ) -> Result<AsyncInventoryMessage, ClientError<T::Error>> {
        let frame = self.client.read_frame()?;
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

    /// Run command `0x65` (Get Frequency Hopping table form).
    ///
    /// Returned values are kHz frequencies.
    pub fn get_frequency_hopping_table(&mut self) -> Result<Vec<u32>, ClientError<T::Error>> {
        let frame = self
            .client
            .transact(CommandCode::GetFrequencyHopping as u8, &[])?;
        parse_frequency_hopping_table(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x65` with option `0x01` (Regulatory Hopping Time).
    pub fn get_regulatory_hop_time(
        &mut self,
    ) -> Result<RegulatoryHopTime, ClientError<T::Error>> {
        let frame = self
            .client
            .transact(CommandCode::GetFrequencyHopping as u8, &[0x01])?;
        parse_regulatory_hop_time(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x61` (Get Antenna Ports) and decode by option.
    pub fn get_antenna_ports(
        &mut self,
        option: AntennaPortsOption,
    ) -> Result<AntennaPortsResponse, ClientError<T::Error>> {
        let frame = self
            .client
            .transact(CommandCode::GetAntennaPorts as u8, &[option.as_u8()])?;
        parse_antenna_ports_response(option, &frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x6A` (Get Reader Configuration).
    pub fn get_reader_configuration(
        &mut self,
        option: u8,
        key: u8,
    ) -> Result<ReaderConfigurationValue, ClientError<T::Error>> {
        let frame = self
            .client
            .transact(CommandCode::GetReaderConfiguration as u8, &[option, key])?;
        parse_reader_configuration_value(&frame.data).map_err(ClientError::Protocol)
    }

    /// Run command `0x6B` (Get Protocol Configuration).
    ///
    /// # Examples
    /// ```rust
    /// use rfidlibrs::{CommandCode, SilionHost};
    /// use rfidlibrs::test_support::{MockInteraction, MockTransport};
    ///
    /// let transport = MockTransport::scripted(vec![MockInteraction {
    ///     request_command: CommandCode::GetProtocolConfiguration as u8,
    ///     response_status: 0x0000,
    ///     response_data: vec![0x05, 0x00, 0x00],
    /// }]);
    /// let mut host = SilionHost::new(transport);
    /// let cfg = host.get_protocol_configuration(0x05, 0x00).unwrap();
    /// assert_eq!(cfg.protocol_value, 0x05);
    /// assert_eq!(cfg.parameter, 0x00);
    /// assert_eq!(cfg.value, Some(0x00));
    /// ```
    pub fn get_protocol_configuration(
        &mut self,
        protocol_value: u8,
        parameter: u8,
    ) -> Result<ProtocolConfigurationValue, ClientError<T::Error>> {
        let frame = self.client.transact(
            CommandCode::GetProtocolConfiguration as u8,
            &[protocol_value, parameter],
        )?;
        parse_protocol_configuration_value(&frame.data).map_err(ClientError::Protocol)
    }
}

/// Parse the data bytes of a validated `0xAA` asynchronous inventory frame
/// into a typed [`AsyncInventoryMessage`].
///
/// The caller is responsible for verifying that the frame's command byte is
/// `0xAA` and that the status is success before calling this function.
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
            return Err(ProtocolError::InvalidResponse("heartbeat payload too short"));
        }
        let search_flags =
            InventorySearchFlags::from_raw(u16::from_be_bytes([data[4], data[5]]));
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
