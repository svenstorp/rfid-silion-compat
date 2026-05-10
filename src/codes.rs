use std::fmt;

/// Command codes defined by the Silion protocol documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CommandCode {
    /// 0x01
    WriteFlash = 0x01,
    /// 0x02
    ReadFlash = 0x02,
    /// 0x03
    GetVersion = 0x03,
    /// 0x04
    BootFirmware = 0x04,
    /// 0x06
    SetBaudRate = 0x06,
    /// 0x08
    VerifyFirmware = 0x08,
    /// 0x09
    BootBootloader = 0x09,
    /// 0x0C
    GetRunPhase = 0x0C,
    /// 0x10
    GetSerialNumber = 0x10,
    /// 0x21
    SingleTagInventory = 0x21,
    /// 0x22
    SynchronousInventory = 0x22,
    /// 0x23
    WriteTagEpc = 0x23,
    /// 0x24
    WriteTagData = 0x24,
    /// 0x25
    LockTag = 0x25,
    /// 0x26
    KillTag = 0x26,
    /// 0x28
    ReadTagData = 0x28,
    /// 0x29
    GetTagBuffer = 0x29,
    /// 0x61
    GetAntennaPorts = 0x61,
    /// 0x63
    GetCurrentTagProtocol = 0x63,
    /// 0x65
    GetFrequencyHopping = 0x65,
    /// 0x66
    GetGpi = 0x66,
    /// 0x67
    GetCurrentRegion = 0x67,
    /// 0x6A
    GetReaderConfiguration = 0x6A,
    /// 0x6B
    GetProtocolConfiguration = 0x6B,
    /// 0x71
    GetAvailableRegions = 0x71,
    /// 0x72
    GetCurrentTemperature = 0x72,
    /// 0x91
    SetAntennaPorts = 0x91,
    /// 0x93
    SetCurrentTagProtocol = 0x93,
    /// 0x95
    SetFrequencyHopping = 0x95,
    /// 0x96
    SetGpo = 0x96,
    /// 0x97
    SetCurrentRegion = 0x97,
    /// 0x9A
    SetReaderConfiguration = 0x9A,
    /// 0x9B
    SetProtocolConfiguration = 0x9B,
    /// 0xAA, Asynchronous inventory command family.
    AsynchronousInventory = 0xAA,
}

impl CommandCode {
    /// Return the raw 8-bit command code.
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

/// Region code values documented for Set/Get Current Region commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum RegionCode {
    /// North America (902-928 MHz), code `0x01`.
    NorthAmerica = 0x01,
    /// China 1 (920-925 MHz), code `0x06`.
    China1 = 0x06,
    /// Europe (865-867 MHz), code `0x08`.
    Europe = 0x08,
    /// China 2 (840-845 MHz), code `0x0A`.
    China2 = 0x0A,
    /// Full Frequency Band (840-960 MHz), code `0xFF`.
    FullFrequencyBand = 0xFF,
}

impl RegionCode {
    /// Convert a raw 8-bit region code to the documented enum if known.
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            0x01 => Some(Self::NorthAmerica),
            0x06 => Some(Self::China1),
            0x08 => Some(Self::Europe),
            0x0A => Some(Self::China2),
            0xFF => Some(Self::FullFrequencyBand),
            _ => None,
        }
    }

    /// Return the raw region code byte.
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

impl From<RegionCode> for u8 {
    fn from(value: RegionCode) -> Self {
        value as u8
    }
}

impl fmt::Display for RegionCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::NorthAmerica => "North America",
            Self::China1 => "China 1",
            Self::Europe => "Europe",
            Self::China2 => "China 2",
            Self::FullFrequencyBand => "Full Frequency Band",
        };
        f.write_str(name)
    }
}

/// Option values documented for Get Antenna Ports (`0x61`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AntennaPortsOption {
    /// `0x00`: get the single TX/RX pair used for tag access operations.
    AccessPair = 0x00,
    /// `0x02`: get the TX/RX pairs used for inventory operations.
    InventoryPairs = 0x02,
    /// `0x03`: get read/write power settings for each logical antenna.
    Power = 0x03,
    /// `0x04`: get read/write power plus settling time for each logical antenna.
    PowerAndSettling = 0x04,
    /// `0x05`: get antenna connection state bytes.
    ConnectionStates = 0x05,
}

impl AntennaPortsOption {
    /// Convert a raw 8-bit option code to the documented enum if known.
    pub const fn from_u8(raw: u8) -> Option<Self> {
        match raw {
            0x00 => Some(Self::AccessPair),
            0x02 => Some(Self::InventoryPairs),
            0x03 => Some(Self::Power),
            0x04 => Some(Self::PowerAndSettling),
            0x05 => Some(Self::ConnectionStates),
            _ => None,
        }
    }

    /// Return the raw antenna ports option byte.
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
}

impl From<AntennaPortsOption> for u8 {
    fn from(value: AntennaPortsOption) -> Self {
        value as u8
    }
}

/// Reader status code values from the documentation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum StatusCode {
    /// 0x0000
    Success = 0x0000,
    /// 0x0100
    DataLengthMismatch = 0x0100,
    /// 0x0101
    UnavailableCommand = 0x0101,
    /// 0x0105
    UnavailableParameter = 0x0105,
    /// 0x010A
    UnavailableBaudRate = 0x010A,
    /// 0x010B
    UnavailableRegion = 0x010B,
    /// 0x0200
    AppFirmwareCrcError = 0x0200,
    /// 0x0302
    FlashWriteFailed = 0x0302,
    /// 0x0400
    NoTagFound = 0x0400,
    /// 0x0402
    ProtocolUnavailable = 0x0402,
    /// 0x040A
    GeneralTagError = 0x040A,
    /// 0x040B
    ReadLengthOutOfLimit = 0x040B,
    /// 0x040C
    UnavailableKillPassword = 0x040C,
    /// 0x0420
    Gen2ProtocolError = 0x0420,
    /// 0x0423
    MemoryOverrunBadPc = 0x0423,
    /// 0x0424
    MemoryLocked = 0x0424,
    /// 0x042B
    InsufficientPower = 0x042B,
    /// 0x042F
    NonSpecificError = 0x042F,
    /// 0x0430
    UnknownTagError = 0x0430,
    /// 0x0500
    UnavailableFrequency = 0x0500,
    /// 0x0504
    TemperatureOverrun = 0x0504,
    /// 0x0505
    HighReturnLoss = 0x0505,
    /// 0x7F00
    UnknownSeriousError = 0x7F00,
    /// 0xFF01
    InitTimerFlashGpioError = 0xFF01,
    /// 0xFF02
    OemInitFailed = 0xFF02,
    /// 0xFF03
    CommandInterfaceInitFailed = 0xFF03,
    /// 0xFF04
    MacRegisterRwInitFailed = 0xFF04,
    /// 0xFF05
    MacRegisterInitFailed = 0xFF05,
    /// 0xFF06
    R2000Arm7InterfaceInitFailed = 0xFF06,
    /// 0xFF07
    R2000Arm7DetectFailed1 = 0xFF07,
    /// 0xFF08
    R2000Arm7DetectFailed2 = 0xFF08,
    /// 0xFF09
    GpioConfigError = 0xFF09,
    /// 0xFF0A
    R2000RegisterInitFailed = 0xFF0A,
    /// 0xFF0B
    EpcProtocolInitFailed = 0xFF0B,
    /// 0xFF0C
    OemMacMappingInitFailed = 0xFF0C,
    /// 0xFF0D
    SerialInitFailed = 0xFF0D,
    /// 0xFF0E
    AppMainHandlerInterfaceError = 0xFF0E,
}

impl StatusCode {
    /// Convert raw 16-bit status to enum if known.
    pub const fn from_u16(raw: u16) -> Option<Self> {
        match raw {
            0x0000 => Some(Self::Success),
            0x0100 => Some(Self::DataLengthMismatch),
            0x0101 => Some(Self::UnavailableCommand),
            0x0105 => Some(Self::UnavailableParameter),
            0x010A => Some(Self::UnavailableBaudRate),
            0x010B => Some(Self::UnavailableRegion),
            0x0200 => Some(Self::AppFirmwareCrcError),
            0x0302 => Some(Self::FlashWriteFailed),
            0x0400 => Some(Self::NoTagFound),
            0x0402 => Some(Self::ProtocolUnavailable),
            0x040A => Some(Self::GeneralTagError),
            0x040B => Some(Self::ReadLengthOutOfLimit),
            0x040C => Some(Self::UnavailableKillPassword),
            0x0420 => Some(Self::Gen2ProtocolError),
            0x0423 => Some(Self::MemoryOverrunBadPc),
            0x0424 => Some(Self::MemoryLocked),
            0x042B => Some(Self::InsufficientPower),
            0x042F => Some(Self::NonSpecificError),
            0x0430 => Some(Self::UnknownTagError),
            0x0500 => Some(Self::UnavailableFrequency),
            0x0504 => Some(Self::TemperatureOverrun),
            0x0505 => Some(Self::HighReturnLoss),
            0x7F00 => Some(Self::UnknownSeriousError),
            0xFF01 => Some(Self::InitTimerFlashGpioError),
            0xFF02 => Some(Self::OemInitFailed),
            0xFF03 => Some(Self::CommandInterfaceInitFailed),
            0xFF04 => Some(Self::MacRegisterRwInitFailed),
            0xFF05 => Some(Self::MacRegisterInitFailed),
            0xFF06 => Some(Self::R2000Arm7InterfaceInitFailed),
            0xFF07 => Some(Self::R2000Arm7DetectFailed1),
            0xFF08 => Some(Self::R2000Arm7DetectFailed2),
            0xFF09 => Some(Self::GpioConfigError),
            0xFF0A => Some(Self::R2000RegisterInitFailed),
            0xFF0B => Some(Self::EpcProtocolInitFailed),
            0xFF0C => Some(Self::OemMacMappingInitFailed),
            0xFF0D => Some(Self::SerialInitFailed),
            0xFF0E => Some(Self::AppMainHandlerInterfaceError),
            _ => None,
        }
    }
}
