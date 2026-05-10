use crate::async_proto::{subcommand_crc, ASYNC_MARKER, ASYNC_TERMINATOR};
use crate::codes::{AntennaPortsOption, CommandCode, RegionCode};
use crate::error::ProtocolError;
use crate::frame::{build_host_frame, push_u16_be, push_u32_be};
use crate::parsers::{AntennaPair, AntennaPower, AntennaPowerSettling};

/// Select/singulation payload used by inventory and tag access commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectContent {
    /// Address (bits).
    pub address_bits: u32,
    /// Number of selected bits.
    pub bit_len: u8,
    /// Select data bytes.
    pub data: Vec<u8>,
}

impl SelectContent {
    pub(crate) fn encode(&self, out: &mut Vec<u8>) {
        push_u32_be(out, self.address_bits);
        out.push(self.bit_len);
        out.extend_from_slice(&self.data);
    }
}

/// Option byte used by inventory commands (for example `0x22` and async start `0xAA48`).
///
/// Lower bits include select-option flags documented under Tag Inventory
/// commands. Higher bits are command-specific non-select option flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InventoryOption(u8);

impl InventoryOption {
    /// Create a default option byte with all bits cleared.
    pub const fn default() -> Self {
        Self(0)
    }

    /// Create from the raw protocol byte.
    pub const fn from_raw(raw: u8) -> Self {
        Self(raw)
    }

    /// Return the raw protocol byte.
    pub const fn raw(self) -> u8 {
        self.0
    }

    /// Return select-option bits (mask `0x2F`) as documented.
    pub const fn select_option_bits(self) -> u8 {
        self.0 & 0x2F
    }
}

impl From<u8> for InventoryOption {
    fn from(value: u8) -> Self {
        Self::from_raw(value)
    }
}

impl From<InventoryOption> for u8 {
    fn from(value: InventoryOption) -> Self {
        value.raw()
    }
}

/// Search flags used by inventory commands (for example `0x22` and `0xAA48`).
///
/// For asynchronous inventory, protocol docs define extra semantics:
/// - bits 8..=11: rest ratio steps (0..=15)
/// - bit 15: heartbeat enable
/// - bit 14: auto-stop enable
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "web-serial", derive(serde::Serialize))]
pub struct InventorySearchFlags(u16);

impl InventorySearchFlags {
    /// Create a default search flags value with all bits cleared.
    pub const fn new() -> Self {
        Self(0)
    }

    /// Create from the raw protocol value.
    pub const fn from_raw(raw: u16) -> Self {
        Self(raw)
    }

    /// Return the raw protocol value.
    pub const fn raw(self) -> u16 {
        self.0
    }

    /// Extract asynchronous rest-ratio steps from bits 8..=11.
    pub const fn async_rest_ratio_steps(self) -> u8 {
        ((self.0 >> 8) & 0x0F) as u8
    }

    /// Return whether asynchronous heartbeat is enabled (bit 15).
    pub const fn async_heartbeat_enabled(self) -> bool {
        (self.0 & 0x8000) != 0
    }

    /// Return whether asynchronous auto-stop is enabled (bit 14).
    pub const fn async_auto_stop_enabled(self) -> bool {
        (self.0 & 0x4000) != 0
    }

    /// Return whether inventory embedded command mode is enabled (bit 2).
    ///
    /// This bit is documented for command `0x22` and reused by
    /// asynchronous inventory start (`0xAA48`).
    pub const fn embedded_command_enabled(self) -> bool {
        (self.0 & 0x0004) != 0
    }

    /// Set asynchronous rest-ratio steps (0..=15) in bits 8..=11.
    pub fn with_async_rest_ratio_steps(self, steps: u8) -> Result<Self, ProtocolError> {
        if steps > 15 {
            return Err(ProtocolError::InvalidArgument(
                "async rest ratio steps must be in 0..=15",
            ));
        }
        let raw = (self.0 & !(0x0F << 8)) | ((steps as u16) << 8);
        Ok(Self(raw))
    }

    /// Set or clear asynchronous heartbeat enable (bit 15).
    pub const fn with_async_heartbeat(self, enabled: bool) -> Self {
        if enabled {
            Self(self.0 | 0x8000)
        } else {
            Self(self.0 & !0x8000)
        }
    }

    /// Set or clear asynchronous auto-stop enable (bit 14).
    pub const fn with_async_auto_stop(self, enabled: bool) -> Self {
        if enabled {
            Self(self.0 | 0x4000)
        } else {
            Self(self.0 & !0x4000)
        }
    }

    /// Set or clear inventory embedded command mode (bit 2).
    pub const fn with_embedded_command(self, enabled: bool) -> Self {
        if enabled {
            Self(self.0 | 0x0004)
        } else {
            Self(self.0 & !0x0004)
        }
    }
}

impl From<u16> for InventorySearchFlags {
    fn from(value: u16) -> Self {
        Self::from_raw(value)
    }
}

impl From<InventorySearchFlags> for u16 {
    fn from(value: InventorySearchFlags) -> Self {
        value.raw()
    }
}

impl Default for InventorySearchFlags {
    fn default() -> Self {
        Self::new()
    }
}

/// Metadata flags that control which per-tag metadata fields the reader
/// includes in inventory responses.
///
/// Defined in the Single Tag Inventory (`0x21`), Get Tag Buffer (`0x29`), and
/// Asynchronous Inventory (`0xAA`) command specifications.
///
/// Each enabled bit requests one additional metadata field in the response.
/// When all bits are zero the reader returns only the EPC and tag CRC.
///
/// | Bit | Value  | Field         | Size    | Description |
/// |-----|--------|---------------|---------|-------------|
/// |  0  | 0x0001 | Read Count    | 1 byte  | Number of times the tag was archived |
/// |  1  | 0x0002 | RSSI          | 1 byte  | Signal strength, signed (dBm) |
/// |  2  | 0x0004 | Antenna ID    | 1 byte  | Logic antenna number |
/// |  3  | 0x0008 | Frequency     | 3 bytes | Frequency at archival (kHz) |
/// |  4  | 0x0010 | Timestamp     | 4 bytes | Elapsed time from inventory start (ms) |
/// |  5  | 0x0020 | RFU           | 2 bytes | Reserved for future use |
/// |  6  | 0x0040 | Protocol ID   | 1 byte  | Tag protocol (0x05 = Gen2) |
/// |  7  | 0x0080 | Data Length   | 2 bytes | Tag data length (0x0000 for 0x21) |
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "web-serial", derive(serde::Serialize))]
pub struct MetadataFlags(u16);

impl MetadataFlags {
    /// No metadata: only EPC and tag CRC are returned.
    pub const NONE: Self = Self(0x0000);
    /// All defined metadata bits set (`0x00FF`).
    pub const ALL: Self = Self(0x00FF);

    /// Create from the raw protocol value.
    pub const fn from_raw(raw: u16) -> Self {
        Self(raw)
    }

    /// Return the raw protocol value.
    pub const fn raw(self) -> u16 {
        self.0
    }

    /// Whether the Read Count field is requested (bit 0).
    pub const fn read_count(self) -> bool {
        (self.0 & 0x0001) != 0
    }
    /// Whether the RSSI field is requested (bit 1).
    pub const fn rssi(self) -> bool {
        (self.0 & 0x0002) != 0
    }
    /// Whether the Antenna ID field is requested (bit 2).
    pub const fn antenna_id(self) -> bool {
        (self.0 & 0x0004) != 0
    }
    /// Whether the Frequency field is requested (bit 3).
    pub const fn frequency(self) -> bool {
        (self.0 & 0x0008) != 0
    }
    /// Whether the Timestamp field is requested (bit 4).
    pub const fn timestamp(self) -> bool {
        (self.0 & 0x0010) != 0
    }
    /// Whether the RFU reserved field is requested (bit 5).
    pub const fn rfu(self) -> bool {
        (self.0 & 0x0020) != 0
    }
    /// Whether the Protocol ID field is requested (bit 6).
    pub const fn protocol_id(self) -> bool {
        (self.0 & 0x0040) != 0
    }
    /// Whether the Data Length field is requested (bit 7).
    pub const fn data_length(self) -> bool {
        (self.0 & 0x0080) != 0
    }

    /// Set or clear the Read Count bit (bit 0).
    pub const fn with_read_count(self, en: bool) -> Self {
        Self(if en { self.0 | 0x0001 } else { self.0 & !0x0001 })
    }
    /// Set or clear the RSSI bit (bit 1).
    pub const fn with_rssi(self, en: bool) -> Self {
        Self(if en { self.0 | 0x0002 } else { self.0 & !0x0002 })
    }
    /// Set or clear the Antenna ID bit (bit 2).
    pub const fn with_antenna_id(self, en: bool) -> Self {
        Self(if en { self.0 | 0x0004 } else { self.0 & !0x0004 })
    }
    /// Set or clear the Frequency bit (bit 3).
    pub const fn with_frequency(self, en: bool) -> Self {
        Self(if en { self.0 | 0x0008 } else { self.0 & !0x0008 })
    }
    /// Set or clear the Timestamp bit (bit 4).
    pub const fn with_timestamp(self, en: bool) -> Self {
        Self(if en { self.0 | 0x0010 } else { self.0 & !0x0010 })
    }
    /// Set or clear the RFU reserved bit (bit 5).
    pub const fn with_rfu(self, en: bool) -> Self {
        Self(if en { self.0 | 0x0020 } else { self.0 & !0x0020 })
    }
    /// Set or clear the Protocol ID bit (bit 6).
    pub const fn with_protocol_id(self, en: bool) -> Self {
        Self(if en { self.0 | 0x0040 } else { self.0 & !0x0040 })
    }
    /// Set or clear the Data Length bit (bit 7).
    pub const fn with_data_length(self, en: bool) -> Self {
        Self(if en { self.0 | 0x0080 } else { self.0 & !0x0080 })
    }
}

impl From<u16> for MetadataFlags {
    fn from(value: u16) -> Self {
        Self::from_raw(value)
    }
}

impl From<MetadataFlags> for u16 {
    fn from(value: MetadataFlags) -> Self {
        value.raw()
    }
}

/// Tag memory bank selector used by read/write access commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MemBank {
    /// Gen2 Reserved bank (`0x00`).
    Reserved = 0x00,
    /// Gen2 EPC bank (`0x01`).
    Epc = 0x01,
    /// Gen2 TID bank (`0x02`).
    Tid = 0x02,
    /// Gen2 User bank (`0x03`).
    User = 0x03,
}

impl MemBank {
    /// Return the raw protocol value.
    pub const fn as_u8(self) -> u8 {
        self as u8
    }

    /// Parse a raw protocol value into a typed memory bank.
    pub fn from_u8(raw: u8) -> Result<Self, ProtocolError> {
        match raw {
            0x00 => Ok(Self::Reserved),
            0x01 => Ok(Self::Epc),
            0x02 => Ok(Self::Tid),
            0x03 => Ok(Self::User),
            _ => Err(ProtocolError::InvalidArgument(
                "membank must be one of 0x00..=0x03",
            )),
        }
    }
}

impl From<MemBank> for u8 {
    fn from(value: MemBank) -> Self {
        value.as_u8()
    }
}

/// Typed inventory embedded command content.
///
/// The protocol currently documents embedded command opcode `0x28`
/// (Read Tag Data) for inventory commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryEmbeddedCommandContent {
    /// Embedded command `0x28` (Read Tag Data).
    ReadTagData(EmbeddedReadTagData),
}

impl InventoryEmbeddedCommandContent {
    fn encoded_len(&self) -> usize {
        match self {
            Self::ReadTagData(cmd) => 3 + cmd.data_field_len(),
        }
    }

    fn encode(&self, out: &mut Vec<u8>) {
        match self {
            Self::ReadTagData(cmd) => cmd.encode(out),
        }
    }
}

/// Typed fields for embedded command `0x28` (Read Tag Data).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddedReadTagData {
    /// Target memory bank.
    pub read_membank: MemBank,
    /// Start address in words.
    pub read_address_words: u32,
    /// Number of words to read.
    pub word_count: u8,
}

impl EmbeddedReadTagData {
    fn data_field_len(&self) -> usize {
        // Timeout(2) + Option(1) + MemBank(1) + Address(4) + WordCount(1)
        9
    }

    fn encode(&self, out: &mut Vec<u8>) {
        // Embedded command frame format:
        // Count(1)=1 | Length(1) | Opcode(1=0x28) | DataField(9)
        out.push(0x01);
        out.push(self.data_field_len() as u8);
        out.push(CommandCode::ReadTagData.as_u8());

        // Vendor docs state timeout/option are ignored for embedded reads.
        push_u16_be(out, 0x0000);
        out.push(0x00);
        out.push(self.read_membank.as_u8());
        push_u32_be(out, self.read_address_words);
        out.push(self.word_count);
    }
}

/// Asynchronous inventory subcommand IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum AsyncSubcommandCode {
    /// Start async inventory.
    Start = 0xAA48,
    /// Stop async inventory.
    Stop = 0xAA49,
}

/// Typed subcommand data for Start AsyncInventory (`0xAA48`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AsyncInventoryStartData {
    /// Metadata flags controlling which per-tag fields the reader returns.
    pub metadata_flags: MetadataFlags,
    /// Option byte, same meaning as command `0x22` select option bits.
    pub option: InventoryOption,
    /// Search flags (2 bytes), same meaning as command `0x22`.
    pub search_flags: InventorySearchFlags,
    /// Optional access password (4 bytes) when required by option.
    pub access_password: Option<u32>,
    /// Optional select content when select operation is enabled.
    pub select_content: Option<SelectContent>,
    /// Optional typed embedded command content.
    pub embedded_command_content: Option<InventoryEmbeddedCommandContent>,
}

impl AsyncInventoryStartData {
    fn encode(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(
            2 + 1 + 2 + if self.access_password.is_some() { 4 } else { 0 }
                + self
                    .select_content
                    .as_ref()
                    .map(|s| 4 + 1 + s.data.len())
                    .unwrap_or(0)
                + self
                    .embedded_command_content
                    .as_ref()
                    .map(InventoryEmbeddedCommandContent::encoded_len)
                    .unwrap_or(0),
        );

        push_u16_be(&mut data, self.metadata_flags.raw());
        data.push(self.option.raw());
        push_u16_be(&mut data, self.search_flags.raw());

        if let Some(password) = self.access_password {
            push_u32_be(&mut data, password);
        }

        if let Some(select) = &self.select_content {
            select.encode(&mut data);
        }

        if let Some(embedded) = &self.embedded_command_content {
            embedded.encode(&mut data);
        }
        data
    }
}

/// Typed payload variants for command `0x91` (Set Antenna Ports).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AntennaPortsConfiguration {
    /// Set the single TX/RX pair used for tag access operations.
    AccessPair(AntennaPair),
    /// Set the ordered TX/RX pairs used during inventory operations.
    InventoryPairs(Vec<AntennaPair>),
    /// Set read/write power per logical TX antenna.
    ///
    /// Power fields use `0.01 dBm` units in the protocol docs, though current
    /// firmware is documented as effectively applying about `1 dBm` precision.
    Power(Vec<AntennaPower>),
    /// Set read/write power and settling time per logical TX antenna.
    ///
    /// Power fields use `0.01 dBm` units in the protocol docs, though current
    /// firmware is documented as effectively applying about `1 dBm` precision.
    PowerAndSettling(Vec<AntennaPowerSettling>),
}

impl AntennaPortsConfiguration {
    fn encode(&self) -> Result<Vec<u8>, ProtocolError> {
        match self {
            Self::AccessPair(pair) => Ok(vec![AntennaPortsOption::AccessPair.as_u8(), pair.tx, pair.rx]),
            Self::InventoryPairs(pairs) => {
                if pairs.is_empty() {
                    return Err(ProtocolError::InvalidArgument(
                        "inventory antenna pairs cannot be empty",
                    ));
                }
                let mut data = Vec::with_capacity(1 + pairs.len() * 2);
                data.push(AntennaPortsOption::InventoryPairs.as_u8());
                for pair in pairs {
                    data.push(pair.tx);
                    data.push(pair.rx);
                }
                Ok(data)
            }
            Self::Power(entries) => {
                if entries.is_empty() {
                    return Err(ProtocolError::InvalidArgument(
                        "antenna power entries cannot be empty",
                    ));
                }
                let mut data = Vec::with_capacity(1 + entries.len() * 5);
                data.push(AntennaPortsOption::Power.as_u8());
                for entry in entries {
                    data.push(entry.tx);
                    push_u16_be(&mut data, entry.read_power);
                    push_u16_be(&mut data, entry.write_power);
                }
                Ok(data)
            }
            Self::PowerAndSettling(entries) => {
                if entries.is_empty() {
                    return Err(ProtocolError::InvalidArgument(
                        "antenna power and settling entries cannot be empty",
                    ));
                }
                let mut data = Vec::with_capacity(1 + entries.len() * 7);
                data.push(AntennaPortsOption::PowerAndSettling.as_u8());
                for entry in entries {
                    data.push(entry.tx);
                    push_u16_be(&mut data, entry.read_power);
                    push_u16_be(&mut data, entry.write_power);
                    push_u16_be(&mut data, entry.settling_time_us);
                }
                Ok(data)
            }
        }
    }
}

/// Command builders for all protocol command groups.
pub struct HostCommand;

impl HostCommand {
    /// Build a raw command packet from command code and data field bytes.
    ///
    /// Use this when the crate does not yet provide a typed builder for a
    /// command variant you need.
    ///
    /// # Examples
    /// ```rust
    /// use rfidlibrs::HostCommand;
    ///
    /// let packet = HostCommand::raw(0x03, &[]).unwrap();
    /// assert_eq!(packet, vec![0xFF, 0x00, 0x03, 0x1D, 0x0C]);
    /// ```
    pub fn raw(command: u8, data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        build_host_frame(command, data)
    }

    /// Build command `0x01` (Write Flash).
    ///
    /// This bootloader command writes firmware words into flash memory.
    /// `finflag` indicates whether this is the final chunk (`0xFF` means last).
    pub fn write_flash(finflag: u8, write_addr: u32, write_data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        if write_data.is_empty() || (write_data.len() % 4 != 0) {
            return Err(ProtocolError::InvalidArgument(
                "write_data must be non-empty and a multiple of 4 bytes",
            ));
        }
        if write_data.len() > 128 {
            return Err(ProtocolError::InvalidArgument("write_data cannot exceed 128 bytes"));
        }
        let words = (write_data.len() / 4) as u8;
        let mut data = Vec::with_capacity(1 + 4 + 1 + write_data.len());
        data.push(finflag);
        push_u32_be(&mut data, write_addr);
        data.push(words);
        data.extend_from_slice(write_data);
        build_host_frame(CommandCode::WriteFlash.as_u8(), &data)
    }

    /// Build command `0x02` (Read Flash).
    ///
    /// This bootloader command reads flash contents from `read_addr` for
    /// `read_len_words * 4` bytes.
    pub fn read_flash(read_addr: u32, read_len_words: u8) -> Result<Vec<u8>, ProtocolError> {
        if read_len_words > 32 {
            return Err(ProtocolError::InvalidArgument("read_len_words must be <= 32"));
        }
        let mut data = Vec::with_capacity(5);
        push_u32_be(&mut data, read_addr);
        data.push(read_len_words);
        build_host_frame(CommandCode::ReadFlash.as_u8(), &data)
    }

    /// Build command `0x03` (Get Version).
    ///
    /// Reader replies with bootloader/hardware/firmware version fields and
    /// supported protocol flags.
    ///
    /// # Examples
    /// ```rust
    /// use rfidlibrs::HostCommand;
    ///
    /// let packet = HostCommand::get_version().unwrap();
    /// assert_eq!(packet, vec![0xFF, 0x00, 0x03, 0x1D, 0x0C]);
    /// ```
    pub fn get_version() -> Result<Vec<u8>, ProtocolError> {
        build_host_frame(CommandCode::GetVersion.as_u8(), &[])
    }

    /// Build command `0x04` (Boot Firmware).
    ///
    /// Switches execution to app firmware when currently in bootloader.
    pub fn boot_firmware() -> Result<Vec<u8>, ProtocolError> {
        build_host_frame(CommandCode::BootFirmware.as_u8(), &[])
    }

    /// Build command `0x06` (Set Baud Rate).
    ///
    /// `baud_rate` is encoded as a 32-bit big-endian integer as required by
    /// the protocol documentation.
    ///
    /// # Examples
    /// ```rust
    /// use rfidlibrs::HostCommand;
    ///
    /// // 115200 decimal = 0x0001C200
    /// let packet = HostCommand::set_baud_rate(115_200).unwrap();
    /// assert_eq!(packet, vec![0xFF, 0x04, 0x06, 0x00, 0x01, 0xC2, 0x00, 0xA4, 0x60]);
    /// ```
    pub fn set_baud_rate(baud_rate: u32) -> Result<Vec<u8>, ProtocolError> {
        let mut data = Vec::with_capacity(4);
        push_u32_be(&mut data, baud_rate);
        build_host_frame(CommandCode::SetBaudRate.as_u8(), &data)
    }

    /// Build command `0x08` (Verify Firmware).
    ///
    /// This is the bootloader verification command used after firmware burn.
    pub fn verify_firmware(check_addr: u32, check_data_len_words: u32, check_crc: u32) -> Result<Vec<u8>, ProtocolError> {
        let mut data = Vec::with_capacity(12);
        push_u32_be(&mut data, check_addr);
        push_u32_be(&mut data, check_data_len_words);
        push_u32_be(&mut data, check_crc);
        build_host_frame(CommandCode::VerifyFirmware.as_u8(), &data)
    }

    /// Build command `0x09` (Boot Bootloader).
    ///
    /// Requests transition from app firmware back to bootloader.
    pub fn boot_bootloader() -> Result<Vec<u8>, ProtocolError> {
        build_host_frame(CommandCode::BootBootloader.as_u8(), &[])
    }

    /// Build command `0x0C` (Get Run Phase).
    ///
    /// Reader returns whether it is currently in bootloader or app firmware.
    ///
    /// # Examples
    /// ```rust
    /// use rfidlibrs::HostCommand;
    ///
    /// let packet = HostCommand::get_run_phase().unwrap();
    /// assert_eq!(packet, vec![0xFF, 0x00, 0x0C, 0x1D, 0x03]);
    /// ```
    pub fn get_run_phase() -> Result<Vec<u8>, ProtocolError> {
        build_host_frame(CommandCode::GetRunPhase.as_u8(), &[])
    }

    /// Build command `0x10` (Get Serial Number).
    ///
    /// `option` and `data_flags` are reserved by the vendor docs.
    pub fn get_serial_number(option: u8, data_flags: u8) -> Result<Vec<u8>, ProtocolError> {
        build_host_frame(CommandCode::GetSerialNumber.as_u8(), &[option, data_flags])
    }

    /// Build command `0x21` (Single Tag Inventory).
    ///
    /// Inventories one tag within `timeout_ms`. Optional metadata and select
    /// filter fields follow the protocol option bits.
    pub fn single_tag_inventory(timeout_ms: u16, option: u8, metadata_flags: Option<MetadataFlags>, select: Option<SelectContent>) -> Result<Vec<u8>, ProtocolError> {
        let mut data = Vec::new();
        push_u16_be(&mut data, timeout_ms);
        data.push(option);
        if let Some(flags) = metadata_flags {
            push_u16_be(&mut data, flags.raw());
        }
        if let Some(sel) = select {
            sel.encode(&mut data);
        }
        build_host_frame(CommandCode::SingleTagInventory.as_u8(), &data)
    }

    /// Build command `0x22` (Synchronous Inventory).
    ///
    /// Performs timed multi-tag inventory and archives tag results into reader
    /// tag buffer for later retrieval by command `0x29`.
    pub fn synchronous_inventory(
        option: u8,
        search_flags: u16,
        timeout_ms: u16,
        access_password: Option<u32>,
        select: Option<SelectContent>,
        embedded_command: Option<&[u8]>,
    ) -> Result<Vec<u8>, ProtocolError> {
        Self::synchronous_inventory_raw_embedded(
            InventoryOption::from_raw(option),
            InventorySearchFlags::from_raw(search_flags),
            timeout_ms,
            access_password,
            select,
            embedded_command,
        )
    }

    /// Build command `0x22` (Synchronous Inventory) using typed option/flags
    /// and typed embedded command content.
    pub fn synchronous_inventory_typed(
        option: InventoryOption,
        search_flags: InventorySearchFlags,
        timeout_ms: u16,
        access_password: Option<u32>,
        select: Option<SelectContent>,
        embedded_command: Option<InventoryEmbeddedCommandContent>,
    ) -> Result<Vec<u8>, ProtocolError> {
        let mut data = Vec::new();
        data.push(option.raw());
        push_u16_be(&mut data, search_flags.raw());
        push_u16_be(&mut data, timeout_ms);
        if let Some(pw) = access_password {
            push_u32_be(&mut data, pw);
        }
        if let Some(sel) = select {
            sel.encode(&mut data);
        }
        if let Some(embedded) = embedded_command {
            embedded.encode(&mut data);
        }
        build_host_frame(CommandCode::SynchronousInventory.as_u8(), &data)
    }

    /// Build command `0x22` (Synchronous Inventory) with raw embedded bytes.
    ///
    /// Prefer [`HostCommand::synchronous_inventory_typed`] where possible.
    pub fn synchronous_inventory_raw_embedded(
        option: InventoryOption,
        search_flags: InventorySearchFlags,
        timeout_ms: u16,
        access_password: Option<u32>,
        select: Option<SelectContent>,
        embedded_command: Option<&[u8]>,
    ) -> Result<Vec<u8>, ProtocolError> {
        let mut data = Vec::new();
        data.push(option.raw());
        push_u16_be(&mut data, search_flags.raw());
        push_u16_be(&mut data, timeout_ms);
        if let Some(pw) = access_password {
            push_u32_be(&mut data, pw);
        }
        if let Some(sel) = select {
            sel.encode(&mut data);
        }
        if let Some(embedded) = embedded_command {
            data.extend_from_slice(embedded);
        }
        build_host_frame(CommandCode::SynchronousInventory.as_u8(), &data)
    }

    /// Build command `0x29` (Get Tag Buffer).
    ///
    /// Retrieves archived tag EPC/metadata records from synchronous inventory.
    pub fn get_tag_buffer(metadata_flags: MetadataFlags, option: u8) -> Result<Vec<u8>, ProtocolError> {
        let mut data = Vec::with_capacity(3);
        push_u16_be(&mut data, metadata_flags.raw());
        data.push(option);
        build_host_frame(CommandCode::GetTagBuffer.as_u8(), &data)
    }

    /// Build command `0x23` (Write Tag EPC).
    ///
    /// Writes EPC data and lets reader update EPC length bits in PC word.
    pub fn write_tag_epc(timeout_ms: u16, option: u8, rfu: Option<u8>, access_password: Option<u32>, select: Option<SelectContent>, epc: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        if epc.is_empty() {
            return Err(ProtocolError::InvalidArgument("epc cannot be empty"));
        }
        let mut data = Vec::new();
        push_u16_be(&mut data, timeout_ms);
        data.push(option);
        if let Some(v) = rfu {
            data.push(v);
        }
        if let Some(pw) = access_password {
            push_u32_be(&mut data, pw);
        }
        if let Some(sel) = select {
            sel.encode(&mut data);
        }
        data.extend_from_slice(epc);
        build_host_frame(CommandCode::WriteTagEpc.as_u8(), &data)
    }

    /// Build command `0x24` (Write Tag Data).
    ///
    /// Writes user-supplied bytes to a target bank/address on selected tag.
    pub fn write_tag_data(
        timeout_ms: u16,
        option: u8,
        write_address_words: u32,
        write_membank: MemBank,
        access_password: Option<u32>,
        select: Option<SelectContent>,
        write_data: &[u8],
    ) -> Result<Vec<u8>, ProtocolError> {
        if write_data.is_empty() || (write_data.len() % 2 != 0) {
            return Err(ProtocolError::InvalidArgument(
                "write_data must be non-empty and multiple of 2 bytes",
            ));
        }
        if write_data.len() > 64 {
            return Err(ProtocolError::InvalidArgument("write_data must be <= 64 bytes"));
        }
        let mut data = Vec::new();
        push_u16_be(&mut data, timeout_ms);
        data.push(option);
        push_u32_be(&mut data, write_address_words);
        data.push(write_membank.as_u8());
        if let Some(pw) = access_password {
            push_u32_be(&mut data, pw);
        }
        if let Some(sel) = select {
            sel.encode(&mut data);
        }
        data.extend_from_slice(write_data);
        build_host_frame(CommandCode::WriteTagData.as_u8(), &data)
    }

    /// Build command `0x25` (Lock Tag).
    ///
    /// Applies Gen2 lock actions defined by `mask_bits` and `action_bits`.
    pub fn lock_tag(
        timeout_ms: u16,
        option: u8,
        access_password: u32,
        mask_bits: u16,
        action_bits: u16,
        select: Option<SelectContent>,
    ) -> Result<Vec<u8>, ProtocolError> {
        let mut data = Vec::new();
        push_u16_be(&mut data, timeout_ms);
        data.push(option);
        push_u32_be(&mut data, access_password);
        push_u16_be(&mut data, mask_bits);
        push_u16_be(&mut data, action_bits);
        if let Some(sel) = select {
            sel.encode(&mut data);
        }
        build_host_frame(CommandCode::LockTag.as_u8(), &data)
    }

    /// Build command `0x26` (Kill Tag).
    ///
    /// Permanently kills a matching tag using `kill_password`.
    pub fn kill_tag(
        timeout_ms: u16,
        option: u8,
        kill_password: u32,
        rfu: u8,
        select: Option<SelectContent>,
    ) -> Result<Vec<u8>, ProtocolError> {
        let mut data = Vec::new();
        push_u16_be(&mut data, timeout_ms);
        data.push(option);
        push_u32_be(&mut data, kill_password);
        data.push(rfu);
        if let Some(sel) = select {
            sel.encode(&mut data);
        }
        build_host_frame(CommandCode::KillTag.as_u8(), &data)
    }

    /// Build command `0x28` (Read Tag Data).
    ///
    /// Reads memory words from a tag bank and optionally requests metadata.
    pub fn read_tag_data(
        timeout_ms: u16,
        option: u8,
        metadata_flags: Option<MetadataFlags>,
        read_membank: MemBank,
        read_address_words: u32,
        word_count: u8,
        access_password: Option<u32>,
        select: Option<SelectContent>,
    ) -> Result<Vec<u8>, ProtocolError> {
        if word_count == 0 || word_count > 96 {
            return Err(ProtocolError::InvalidArgument("word_count must be in 1..=96"));
        }
        let mut data = Vec::new();
        push_u16_be(&mut data, timeout_ms);
        data.push(option);
        if let Some(flags) = metadata_flags {
            push_u16_be(&mut data, flags.raw());
        }
        data.push(read_membank.as_u8());
        push_u32_be(&mut data, read_address_words);
        data.push(word_count);
        if let Some(pw) = access_password {
            push_u32_be(&mut data, pw);
        }
        if let Some(sel) = select {
            sel.encode(&mut data);
        }
        build_host_frame(CommandCode::ReadTagData.as_u8(), &data)
    }

    /// Build command `0x91` (Set Antenna Ports).
    ///
    /// The payload shape depends on the configuration variant and is encoded
    /// according to the option-specific layout from the protocol docs.
    ///
    /// # Examples
    /// ```rust
    /// use rfidlibrs::{AntennaPair, AntennaPortsConfiguration, HostCommand};
    ///
    /// let packet = HostCommand::set_antenna_ports(&AntennaPortsConfiguration::AccessPair(
    ///     AntennaPair { tx: 0x01, rx: 0x01 },
    /// ))
    /// .unwrap();
    /// assert_eq!(packet, vec![0xFF, 0x03, 0x91, 0x00, 0x01, 0x01, 0x62, 0x87]);
    /// ```
    pub fn set_antenna_ports(config: &AntennaPortsConfiguration) -> Result<Vec<u8>, ProtocolError> {
        let data = config.encode()?;
        build_host_frame(CommandCode::SetAntennaPorts.as_u8(), &data)
    }

    /// Build command `0x93` (Set Current Tag Protocol).
    ///
    /// Current firmware expects `protocol` equal to `0x0005` (GEN2).
    pub fn set_current_tag_protocol(protocol: u16) -> Result<Vec<u8>, ProtocolError> {
        let mut data = Vec::with_capacity(2);
        push_u16_be(&mut data, protocol);
        build_host_frame(CommandCode::SetCurrentTagProtocol.as_u8(), &data)
    }

    /// Build command `0x95` (Set Frequency Hopping).
    ///
    /// Sets hop table or reserved regulatory hopping time format.
    pub fn set_frequency_hopping(data_field: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        build_host_frame(CommandCode::SetFrequencyHopping.as_u8(), data_field)
    }

    /// Build command `0x96` (Set GPO / Get GPO status).
    ///
    /// Non-empty `data_field` sets GPO pin states. Empty data requests current
    /// GPO status in the response payload.
    pub fn set_gpo(data_field: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        build_host_frame(CommandCode::SetGpo.as_u8(), data_field)
    }

    /// Build command `0x97` (Set Current Region).
    ///
    /// Selects working region code used by frequency/hopping constraints.
    ///
    /// # Examples
    /// ```rust
    /// use rfidlibrs::{HostCommand, RegionCode};
    ///
    /// let packet = HostCommand::set_current_region(RegionCode::NorthAmerica).unwrap();
    /// assert_eq!(packet, vec![0xFF, 0x01, 0x97, 0x01, 0x4B, 0xBC]);
    /// ```
    pub fn set_current_region(region_code: RegionCode) -> Result<Vec<u8>, ProtocolError> {
        build_host_frame(CommandCode::SetCurrentRegion.as_u8(), &[region_code.as_u8()])
    }

    /// Build command `0x9A` (Set Reader Configuration).
    ///
    /// Sets one reader key/value under `option` 0x01 format.
    pub fn set_reader_configuration(option: u8, key: u8, value: u8) -> Result<Vec<u8>, ProtocolError> {
        build_host_frame(CommandCode::SetReaderConfiguration.as_u8(), &[option, key, value])
    }

    /// Build command `0x9B` (Set Protocol Configuration).
    ///
    /// Sets protocol parameter values (session, target, Q, etc.) according to
    /// option/value presence required by the selected parameter.
    pub fn set_protocol_configuration(protocol_value: u8, parameter: u8, option: Option<u8>, value: Option<u8>) -> Result<Vec<u8>, ProtocolError> {
        let mut data = vec![protocol_value, parameter];
        if let Some(opt) = option {
            data.push(opt);
        }
        if let Some(v) = value {
            data.push(v);
        }
        build_host_frame(CommandCode::SetProtocolConfiguration.as_u8(), &data)
    }

    /// Build command `0x61` (Get Antenna Ports).
    ///
    /// `option` selects which antenna view is requested (access pair,
    /// inventory pairs, powers, powers+settling, or connection states).
    pub fn get_antenna_ports(option: AntennaPortsOption) -> Result<Vec<u8>, ProtocolError> {
        build_host_frame(CommandCode::GetAntennaPorts.as_u8(), &[option.as_u8()])
    }

    /// Build command `0x63` (Get Current Tag Protocol).
    ///
    /// Returns active tag protocol (currently GEN2 `0x0005`).
    pub fn get_current_tag_protocol() -> Result<Vec<u8>, ProtocolError> {
        build_host_frame(CommandCode::GetCurrentTagProtocol.as_u8(), &[])
    }

    /// Build command `0x65` (Get Frequency Hopping).
    ///
    /// `None` requests the full hop table. `Some(0x01)` requests regulatory
    /// hopping time payload format.
    ///
    /// # Examples
    /// ```rust
    /// use rfidlibrs::HostCommand;
    ///
    /// let table_req = HostCommand::get_frequency_hopping(None).unwrap();
    /// assert_eq!(table_req, vec![0xFF, 0x00, 0x65, 0x1D, 0x6A]);
    ///
    /// let hop_time_req = HostCommand::get_frequency_hopping(Some(0x01)).unwrap();
    /// assert_eq!(hop_time_req, vec![0xFF, 0x01, 0x65, 0x01, 0xB9, 0xBC]);
    /// ```
    pub fn get_frequency_hopping(option: Option<u8>) -> Result<Vec<u8>, ProtocolError> {
        match option {
            Some(v) => build_host_frame(CommandCode::GetFrequencyHopping.as_u8(), &[v]),
            None => build_host_frame(CommandCode::GetFrequencyHopping.as_u8(), &[]),
        }
    }

    /// Build command `0x66` (Get GPI).
    ///
    /// Returns input pin states ordered by pin number.
    pub fn get_gpi() -> Result<Vec<u8>, ProtocolError> {
        build_host_frame(CommandCode::GetGpi.as_u8(), &[])
    }

    /// Build command `0x67` (Get Current Region).
    ///
    /// Returns active region code.
    pub fn get_current_region() -> Result<Vec<u8>, ProtocolError> {
        build_host_frame(CommandCode::GetCurrentRegion.as_u8(), &[])
    }

    /// Build command `0x71` (Get Available Regions).
    ///
    /// Returns region codes supported by the connected reader firmware.
    pub fn get_available_regions() -> Result<Vec<u8>, ProtocolError> {
        build_host_frame(CommandCode::GetAvailableRegions.as_u8(), &[])
    }

    /// Build command `0x6A` (Get Reader Configuration).
    ///
    /// Requests one key under a given option namespace.
    pub fn get_reader_configuration(option: u8, key: u8) -> Result<Vec<u8>, ProtocolError> {
        build_host_frame(CommandCode::GetReaderConfiguration.as_u8(), &[option, key])
    }

    /// Build command `0x6B` (Get Protocol Configuration).
    ///
    /// Requests one protocol parameter for a selected protocol id.
    ///
    /// # Examples
    /// ```rust
    /// use rfidlibrs::HostCommand;
    ///
    /// // Protocol 0x05 (GEN2), parameter 0x00 (session)
    /// let packet = HostCommand::get_protocol_configuration(0x05, 0x00).unwrap();
    /// assert_eq!(packet, vec![0xFF, 0x02, 0x6B, 0x05, 0x00, 0x3A, 0x6F]);
    /// ```
    pub fn get_protocol_configuration(protocol_value: u8, parameter: u8) -> Result<Vec<u8>, ProtocolError> {
        build_host_frame(
            CommandCode::GetProtocolConfiguration.as_u8(),
            &[protocol_value, parameter],
        )
    }

    /// Build command `0x72` (Get Current Temperature).
    ///
    /// Returns reader board temperature as one byte.
    pub fn get_current_temperature() -> Result<Vec<u8>, ProtocolError> {
        build_host_frame(CommandCode::GetCurrentTemperature.as_u8(), &[])
    }

    /// Build command `0xAA` Start Async Inventory subcommand (`0xAA48`).
    ///
    /// `start` follows the documented subcommand data format:
    /// `MetadataFlags(2) | Option(1) | SearchFlags(2) | [AccessPassword(4)] |
    /// [SelectContent] | [EmbeddedCommandContent]`.
    ///
    /// # Examples
    /// ```rust
    /// use rfidlibrs::{
    ///     AsyncInventoryStartData, EmbeddedReadTagData, HostCommand,
    ///     InventoryEmbeddedCommandContent, InventoryOption, InventorySearchFlags, MemBank,
    ///     MetadataFlags,
    /// };
    ///
    /// let search_flags = InventorySearchFlags::new()
    ///     .with_async_heartbeat(true)
    ///     .with_async_auto_stop(false)
    ///     .with_embedded_command(true)
    ///     .with_async_rest_ratio_steps(3)
    ///     .unwrap();
    ///
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
    /// let packet = HostCommand::async_start(&start).unwrap();
    /// assert_eq!(packet[0], 0xFF);
    /// assert_eq!(packet[2], 0xAA);
    /// ```
    pub fn async_start(start: &AsyncInventoryStartData) -> Result<Vec<u8>, ProtocolError> {
        let subcommand_data = start.encode();
        Self::async_inventory(AsyncSubcommandCode::Start, &subcommand_data)
    }

    /// Build command `0xAA` Stop Async Inventory subcommand (`0xAA49`).
    ///
    /// # Examples
    /// ```rust
    /// use rfidlibrs::HostCommand;
    ///
    /// let packet = HostCommand::async_stop().unwrap();
    /// assert_eq!(
    ///     packet,
    ///     vec![
    ///         0xFF, 0x0E, 0xAA, 0x4D, 0x6F, 0x64, 0x75, 0x6C, 0x65, 0x74,
    ///         0x65, 0x63, 0x68, 0xAA, 0x49, 0xF3, 0xBB, 0x03, 0x91,
    ///     ]
    /// );
    /// ```
    pub fn async_stop() -> Result<Vec<u8>, ProtocolError> {
        Self::async_inventory(AsyncSubcommandCode::Stop, &[])
    }

    /// Build a generic `0xAA` asynchronous inventory command packet.
    ///
    /// This inserts the fixed marker (`Moduletech`), subcommand, subcommand
    /// payload, sub-CRC (8-bit sum), and terminator (`0xBB`) before wrapping the
    /// bytes in a normal host frame.
    pub fn async_inventory(subcommand: AsyncSubcommandCode, subcommand_data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
        let mut data = Vec::with_capacity(10 + 2 + subcommand_data.len() + 2);
        data.extend_from_slice(ASYNC_MARKER);
        push_u16_be(&mut data, subcommand as u16);
        data.extend_from_slice(subcommand_data);
        let sub_crc = subcommand_crc(subcommand as u16, subcommand_data);
        data.push(sub_crc);
        data.push(ASYNC_TERMINATOR);
        build_host_frame(CommandCode::AsynchronousInventory.as_u8(), &data)
    }
}
