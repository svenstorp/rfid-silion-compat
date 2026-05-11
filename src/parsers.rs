use crate::codes::{AntennaPortsOption, RegionCode};
use crate::command::MetadataFlags;
use crate::error::ProtocolError;
use std::sync::LazyLock;

static TAG_PARSE_DEBUG_ENABLED: LazyLock<bool> = LazyLock::new(|| {
    std::env::var_os("RFID_SILION_COMPAT_TAG_PARSE_DEBUG").is_some()
});

/// Version information returned by 0x03/0x04 responses.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "web-serial", derive(serde::Serialize))]
#[cfg_attr(feature = "web-serial", serde(rename_all = "camelCase"))]
pub struct VersionInfo {
    /// Bootloader version.
    pub bootloader_version: [u8; 4],
    /// Hardware version.
    pub hardware_version: [u8; 4],
    /// Firmware date.
    pub firmware_date: [u8; 4],
    /// Firmware version.
    pub firmware_version: [u8; 4],
    /// Supported protocol bitfield.
    pub supported_protocol: [u8; 4],
}

/// Parse the 20-byte data payload returned by commands `0x03`/`0x04`.
///
/// These correspond to Get Version and Boot Firmware success responses.
///
/// # Examples
/// ```rust
/// use rfid_silion_compat::parse_version_info;
///
/// let data = [
///     0x13, 0x04, 0x15, 0x00, 0xA8, 0x00, 0x00, 0x01,
///     0x20, 0x13, 0x05, 0x22, 0x13, 0x05, 0x23, 0x00,
///     0x00, 0x00, 0x00, 0x10,
/// ];
/// let version = parse_version_info(&data).unwrap();
/// assert_eq!(version.supported_protocol, [0x00, 0x00, 0x00, 0x10]);
/// ```
pub fn parse_version_info(data: &[u8]) -> Result<VersionInfo, ProtocolError> {
    if data.len() != 20 {
        return Err(ProtocolError::InvalidResponse(
            "version data length must be 20",
        ));
    }
    Ok(VersionInfo {
        bootloader_version: data[0..4]
            .try_into()
            .map_err(|_| ProtocolError::InvalidResponse("bootloader version"))?,
        hardware_version: data[4..8]
            .try_into()
            .map_err(|_| ProtocolError::InvalidResponse("hardware version"))?,
        firmware_date: data[8..12]
            .try_into()
            .map_err(|_| ProtocolError::InvalidResponse("firmware date"))?,
        firmware_version: data[12..16]
            .try_into()
            .map_err(|_| ProtocolError::InvalidResponse("firmware version"))?,
        supported_protocol: data[16..20]
            .try_into()
            .map_err(|_| ProtocolError::InvalidResponse("supported protocol"))?,
    })
}

/// Program phase returned by Get Run Phase (0x0C).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "web-serial", derive(serde::Serialize))]
#[cfg_attr(feature = "web-serial", serde(rename_all = "camelCase"))]
pub enum RunPhase {
    /// Bootloader firmware layer (0x11).
    Bootloader,
    /// Application firmware layer (0x12).
    AppFirmware,
}

/// Parse command `0x0C` (Get Run Phase) response payload.
///
/// # Examples
/// ```rust
/// use rfid_silion_compat::{parse_run_phase, RunPhase};
///
/// assert_eq!(parse_run_phase(&[0x11]).unwrap(), RunPhase::Bootloader);
/// assert_eq!(parse_run_phase(&[0x12]).unwrap(), RunPhase::AppFirmware);
/// ```
pub fn parse_run_phase(data: &[u8]) -> Result<RunPhase, ProtocolError> {
    if data.len() != 1 {
        return Err(ProtocolError::InvalidResponse(
            "run phase data length must be 1",
        ));
    }
    match data[0] {
        0x11 => Ok(RunPhase::Bootloader),
        0x12 => Ok(RunPhase::AppFirmware),
        _ => Err(ProtocolError::InvalidResponse("unknown run phase value")),
    }
}

/// Serial number information returned by Get Serial Number (0x10).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "web-serial", derive(serde::Serialize))]
#[cfg_attr(feature = "web-serial", serde(rename_all = "camelCase"))]
pub struct SerialNumberInfo {
    /// Year bytes as returned by reader.
    pub year: [u8; 4],
    /// Batch serial number bytes as returned by reader.
    pub serial_number: [u8; 8],
}

/// Parse command `0x10` (Get Serial Number) response payload.
pub fn parse_serial_number_info(data: &[u8]) -> Result<SerialNumberInfo, ProtocolError> {
    if data.len() != 12 {
        return Err(ProtocolError::InvalidResponse(
            "serial number data length must be 12",
        ));
    }
    Ok(SerialNumberInfo {
        year: data[0..4]
            .try_into()
            .map_err(|_| ProtocolError::InvalidResponse("invalid serial year"))?,
        serial_number: data[4..12]
            .try_into()
            .map_err(|_| ProtocolError::InvalidResponse("invalid serial number"))?,
    })
}

/// Parse command `0x63` (Get Current Tag Protocol) response payload.
pub fn parse_current_tag_protocol(data: &[u8]) -> Result<u16, ProtocolError> {
    if data.len() != 2 {
        return Err(ProtocolError::InvalidResponse(
            "current tag protocol data length must be 2",
        ));
    }
    Ok(u16::from_be_bytes([data[0], data[1]]))
}

/// Parse command `0x67` (Get Current Region) response payload.
///
/// # Examples
/// ```rust
/// use rfid_silion_compat::{parse_current_region, RegionCode};
///
/// assert_eq!(parse_current_region(&[0x01]).unwrap(), RegionCode::NorthAmerica);
/// ```
pub fn parse_current_region(data: &[u8]) -> Result<RegionCode, ProtocolError> {
    if data.len() != 1 {
        return Err(ProtocolError::InvalidResponse(
            "current region data length must be 1",
        ));
    }
    RegionCode::from_u8(data[0]).ok_or(ProtocolError::InvalidResponse(
        "unknown current region code",
    ))
}

/// Parse command `0x71` (Get Available Regions) response payload.
pub fn parse_available_regions(data: &[u8]) -> Result<Vec<RegionCode>, ProtocolError> {
    if data.is_empty() {
        return Err(ProtocolError::InvalidResponse(
            "available regions data cannot be empty",
        ));
    }
    data.iter()
        .map(|&raw| {
            RegionCode::from_u8(raw).ok_or(ProtocolError::InvalidResponse(
                "unknown available region code",
            ))
        })
        .collect()
}

/// Parse command `0x72` (Get Current Temperature) response payload.
pub fn parse_current_temperature(data: &[u8]) -> Result<u8, ProtocolError> {
    if data.len() != 1 {
        return Err(ProtocolError::InvalidResponse(
            "temperature data length must be 1",
        ));
    }
    Ok(data[0])
}

/// Parse pin state payload from command `0x66` (GPI) or `0x96` status response.
pub fn parse_pin_states(data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    if data.is_empty() {
        return Err(ProtocolError::InvalidResponse(
            "pin state data cannot be empty",
        ));
    }
    Ok(data.to_vec())
}

/// Parse command `0x65` response payload in hop-table format.
///
/// The returned values are frequencies in kHz.
///
/// # Examples
/// ```rust
/// use rfid_silion_compat::parse_frequency_hopping_table;
///
/// let data = [0x00, 0x0D, 0xF7, 0x32, 0x00, 0x0D, 0xC8, 0x52];
/// let freqs = parse_frequency_hopping_table(&data).unwrap();
/// assert_eq!(freqs, vec![915_250, 903_250]);
/// ```
pub fn parse_frequency_hopping_table(data: &[u8]) -> Result<Vec<u32>, ProtocolError> {
    if data.is_empty() || (data.len() % 4 != 0) {
        return Err(ProtocolError::InvalidResponse(
            "frequency table length must be a non-empty multiple of 4",
        ));
    }
    let mut out = Vec::with_capacity(data.len() / 4);
    for chunk in data.chunks_exact(4) {
        out.push(u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
    }
    Ok(out)
}

/// Regulatory hopping time response from Get Frequency Hopping with option 0x01.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "web-serial", derive(serde::Serialize))]
#[cfg_attr(feature = "web-serial", serde(rename_all = "camelCase"))]
pub struct RegulatoryHopTime {
    /// Option value, expected to be 0x01.
    pub option: u8,
    /// Hop time in milliseconds.
    pub hop_time_ms: u32,
}

/// Parse command `0x65` regulatory hopping-time payload (`option=0x01`).
pub fn parse_regulatory_hop_time(data: &[u8]) -> Result<RegulatoryHopTime, ProtocolError> {
    if data.len() != 5 {
        return Err(ProtocolError::InvalidResponse(
            "regulatory hop time data length must be 5",
        ));
    }
    Ok(RegulatoryHopTime {
        option: data[0],
        hop_time_ms: u32::from_be_bytes([data[1], data[2], data[3], data[4]]),
    })
}

/// Reader configuration triplet returned by Get Reader Configuration (0x6A).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "web-serial", derive(serde::Serialize))]
#[cfg_attr(feature = "web-serial", serde(rename_all = "camelCase"))]
pub struct ReaderConfigurationValue {
    /// Option field.
    pub option: u8,
    /// Configuration key.
    pub key: u8,
    /// Configuration value.
    pub value: u8,
}

/// Parse command `0x6A` (Get Reader Configuration) response payload.
pub fn parse_reader_configuration_value(
    data: &[u8],
) -> Result<ReaderConfigurationValue, ProtocolError> {
    if data.len() != 3 {
        return Err(ProtocolError::InvalidResponse(
            "reader configuration data length must be 3",
        ));
    }
    Ok(ReaderConfigurationValue {
        option: data[0],
        key: data[1],
        value: data[2],
    })
}

/// Protocol configuration triplet/quad returned by Get Protocol Configuration (0x6B).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "web-serial", derive(serde::Serialize))]
#[cfg_attr(feature = "web-serial", serde(rename_all = "camelCase"))]
pub struct ProtocolConfigurationValue {
    /// Protocol value.
    pub protocol_value: u8,
    /// Parameter value.
    pub parameter: u8,
    /// Optional option byte.
    pub option: Option<u8>,
    /// Optional value byte.
    pub value: Option<u8>,
}

/// Parse command `0x6B` (Get Protocol Configuration) response payload.
///
/// Different parameters return either:
/// - protocol+parameter only,
/// - protocol+parameter+value,
/// - protocol+parameter+option+value.
pub fn parse_protocol_configuration_value(
    data: &[u8],
) -> Result<ProtocolConfigurationValue, ProtocolError> {
    match data.len() {
        2 => Ok(ProtocolConfigurationValue {
            protocol_value: data[0],
            parameter: data[1],
            option: None,
            value: None,
        }),
        3 => Ok(ProtocolConfigurationValue {
            protocol_value: data[0],
            parameter: data[1],
            option: None,
            value: Some(data[2]),
        }),
        4 => Ok(ProtocolConfigurationValue {
            protocol_value: data[0],
            parameter: data[1],
            option: Some(data[2]),
            value: Some(data[3]),
        }),
        _ => Err(ProtocolError::InvalidResponse(
            "protocol configuration data length must be 2, 3, or 4",
        )),
    }
}

/// One parsed `Tag EPC and Meta Data` block as documented for command `0x29`
/// (Get Tag Buffer) and reused by asynchronous inventory tag uploads.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "web-serial", derive(serde::Serialize))]
#[cfg_attr(feature = "web-serial", serde(rename_all = "camelCase"))]
pub struct TagEpcAndMetaData {
    /// Number of times this tag was archived (bit 0).
    pub read_count: Option<u8>,
    /// RSSI in dBm (signed byte, bit 1).
    pub rssi_dbm: Option<i8>,
    /// Antenna logic ID (bit 2).
    pub antenna_id: Option<u8>,
    /// Frequency in kHz (bit 3).
    pub frequency_khz: Option<u32>,
    /// Time from inventory start to first archive, in milliseconds (bit 4).
    pub timestamp_ms: Option<u32>,
    /// Reserved RFU field (bit 5).
    pub rfu: Option<u16>,
    /// Tag protocol ID (`0x05` means Gen2, bit 6).
    pub protocol_id: Option<u8>,
    /// Embedded command tag data bit length (bit 7).
    pub tag_data_bit_length: Option<u16>,
    /// Embedded command tag data bytes (bit 7).
    pub tag_data: Option<Vec<u8>>,
    /// EPC length in bits including PC and tag CRC.
    pub epc_bit_length: u16,
    /// PC word from EPC bank.
    pub pc_word: u16,
    /// EPC ID bytes.
    pub epc_id: Vec<u8>,
    /// Tag CRC from EPC bank.
    pub tag_crc: u16,
}

/// Module-level helper to read a single byte with bounds checking and optional debug output.
fn read_u8_from_data(
    idx: &mut usize,
    data: &[u8],
    what: &'static str,
) -> Result<u8, ProtocolError> {
    if *idx + 1 > data.len() {
        if *TAG_PARSE_DEBUG_ENABLED {
            eprintln!(
                "[rfid-silion-compat][tag-parse] {what} (idx={idx}, need=1, len={len})",
                idx = *idx,
                len = data.len()
            );
        }
        return Err(ProtocolError::InvalidResponse(what));
    }
    let out = data[*idx];
    *idx += 1;
    Ok(out)
}

/// Module-level helper to read a 16-bit big-endian value with bounds checking and optional debug output.
fn read_u16_from_data(
    idx: &mut usize,
    data: &[u8],
    what: &'static str,
) -> Result<u16, ProtocolError> {
    if *idx + 2 > data.len() {
        if *TAG_PARSE_DEBUG_ENABLED {
            eprintln!(
                "[rfid-silion-compat][tag-parse] {what} (idx={idx}, need=2, len={len})",
                idx = *idx,
                len = data.len()
            );
        }
        return Err(ProtocolError::InvalidResponse(what));
    }
    let out = u16::from_be_bytes([data[*idx], data[*idx + 1]]);
    *idx += 2;
    Ok(out)
}

/// Module-level helper to read a 24-bit big-endian value with bounds checking and optional debug output.
fn read_u24_from_data(
    idx: &mut usize,
    data: &[u8],
    what: &'static str,
) -> Result<u32, ProtocolError> {
    if *idx + 3 > data.len() {
        if *TAG_PARSE_DEBUG_ENABLED {
            eprintln!(
                "[rfid-silion-compat][tag-parse] {what} (idx={idx}, need=3, len={len})",
                idx = *idx,
                len = data.len()
            );
        }
        return Err(ProtocolError::InvalidResponse(what));
    }
    let out = u32::from_be_bytes([0, data[*idx], data[*idx + 1], data[*idx + 2]]);
    *idx += 3;
    Ok(out)
}

/// Module-level helper to read a 32-bit big-endian value with bounds checking and optional debug output.
fn read_u32_from_data(
    idx: &mut usize,
    data: &[u8],
    what: &'static str,
) -> Result<u32, ProtocolError> {
    if *idx + 4 > data.len() {
        if *TAG_PARSE_DEBUG_ENABLED {
            eprintln!(
                "[rfid-silion-compat][tag-parse] {what} (idx={idx}, need=4, len={len})",
                idx = *idx,
                len = data.len()
            );
        }
        return Err(ProtocolError::InvalidResponse(what));
    }
    let out =
        u32::from_be_bytes([data[*idx], data[*idx + 1], data[*idx + 2], data[*idx + 3]]);
    *idx += 4;
    Ok(out)
}

/// Parse one `Tag EPC and Meta Data` block.
///
/// The field presence and order are controlled by [`MetadataFlags`], and must
/// match the bit layout defined in the protocol documentation.
pub fn parse_tag_epc_and_meta_data(
    metadata_flags: MetadataFlags,
    data: &[u8],
) -> Result<TagEpcAndMetaData, ProtocolError> {
    let mut idx = 0usize;

    if *TAG_PARSE_DEBUG_ENABLED {
        eprintln!(
            "[rfid-silion-compat][tag-parse] start flags=0x{flags:04X} data_len={len}",
            flags = metadata_flags.raw(),
            len = data.len()
        );
        for (line_idx, chunk) in data.chunks(16).enumerate() {
            eprintln!(
                "[rfid-silion-compat][tag-parse] raw +0x{offset:04X}: {chunk:02X?}",
                offset = line_idx * 16,
            );
        }
    }

    let read_count = if metadata_flags.read_count() {
        Some(read_u8_from_data(&mut idx, data, "tag metadata missing read count")?)
    } else {
        None
    };
    if *TAG_PARSE_DEBUG_ENABLED {
        eprintln!("[rfid-silion-compat][tag-parse] field read_count={read_count:?} idx={idx}");
    }

    let rssi_dbm = if metadata_flags.rssi() {
        Some(read_u8_from_data(&mut idx, data, "tag metadata missing RSSI")? as i8)
    } else {
        None
    };
    if *TAG_PARSE_DEBUG_ENABLED {
        eprintln!("[rfid-silion-compat][tag-parse] field rssi_dbm={rssi_dbm:?} idx={idx}");
    }

    let antenna_id = if metadata_flags.antenna_id() {
        Some(read_u8_from_data(&mut idx, data, "tag metadata missing antenna id")?)
    } else {
        None
    };
    if *TAG_PARSE_DEBUG_ENABLED {
        eprintln!("[rfid-silion-compat][tag-parse] field antenna_id={antenna_id:?} idx={idx}");
    }

    let frequency_khz = if metadata_flags.frequency() {
        Some(read_u24_from_data(&mut idx, data, "tag metadata missing frequency")?)
    } else {
        None
    };
    if *TAG_PARSE_DEBUG_ENABLED {
        eprintln!(
            "[rfid-silion-compat][tag-parse] field frequency_khz={frequency_khz:?} idx={idx}"
        );
    }

    let timestamp_ms = if metadata_flags.timestamp() {
        Some(read_u32_from_data(&mut idx, data, "tag metadata missing timestamp")?)
    } else {
        None
    };
    if *TAG_PARSE_DEBUG_ENABLED {
        eprintln!("[rfid-silion-compat][tag-parse] field timestamp_ms={timestamp_ms:?} idx={idx}");
    }

    let rfu = if metadata_flags.rfu() {
        Some(read_u16_from_data(&mut idx, data, "tag metadata missing RFU")?)
    } else {
        None
    };
    if *TAG_PARSE_DEBUG_ENABLED {
        eprintln!("[rfid-silion-compat][tag-parse] field rfu={rfu:?} idx={idx}");
    }

    let protocol_id = if metadata_flags.protocol_id() {
        Some(read_u8_from_data(&mut idx, data, "tag metadata missing protocol id")?)
    } else {
        None
    };
    if *TAG_PARSE_DEBUG_ENABLED {
        eprintln!("[rfid-silion-compat][tag-parse] field protocol_id={protocol_id:?} idx={idx}");
    }

    let (tag_data_bit_length, tag_data) = if metadata_flags.data_length() {
        let bits = read_u16_from_data(&mut idx, data, "tag metadata missing tag data length")?;
        if bits % 8 != 0 {
            if *TAG_PARSE_DEBUG_ENABLED {
                eprintln!(
                    "[rfid-silion-compat][tag-parse] tag_data_bit_length not byte-aligned: bits={bits} idx={idx}"
                );
            }
            return Err(ProtocolError::InvalidResponse(
                "tag data length must be byte-aligned",
            ));
        }
        let bytes = (bits / 8) as usize;
        if idx + bytes > data.len() {
            if *TAG_PARSE_DEBUG_ENABLED {
                eprintln!(
                    "[rfid-silion-compat][tag-parse] tag metadata missing tag data bytes (idx={idx}, need={need}, len={len})",
                    need = bytes,
                    len = data.len()
                );
            }
            return Err(ProtocolError::InvalidResponse(
                "tag metadata missing tag data bytes",
            ));
        }
        let payload = data[idx..idx + bytes].to_vec();
        idx += bytes;
        (Some(bits), Some(payload))
    } else {
        (None, None)
    };
    if *TAG_PARSE_DEBUG_ENABLED {
        eprintln!(
            "[rfid-silion-compat][tag-parse] field tag_data_bit_length={tag_data_bit_length:?} tag_data_len={tag_data_len:?} idx={idx}",
            tag_data_len = tag_data.as_ref().map(|v| v.len())
        );
    }

    // The EPC length is specified as a bit length in the protocol, but in reality it is a byte length (including PC and CRC).
    // The reader always returns a byte length, so we read it as a byte length and convert to bits for the struct field..
    /*let epc_bit_length = read_u16(&mut idx, data, "tag metadata missing EPC length")?;
    if epc_bit_length % 8 != 0 {
        if *TAG_PARSE_DEBUG_ENABLED {
            eprintln!(
                "[rfid-silion-compat][tag-parse] epc_bit_length not byte-aligned: bits={bits}",
                bits = epc_bit_length
            );
        }
        return Err(ProtocolError::InvalidResponse(
            "EPC length must be byte-aligned",
        ));
    }*/
    let epc_total_bytes = read_u8_from_data(&mut idx, data, "tag metadata missing EPC length")? as usize;
    let epc_bit_length = (epc_total_bytes as u16) * 8;

    if epc_total_bytes < 4 {
        if *TAG_PARSE_DEBUG_ENABLED {
            eprintln!(
                "[rfid-silion-compat][tag-parse] EPC length too small: epc_total_bytes={epc_total_bytes}"
            );
        }
        return Err(ProtocolError::InvalidResponse(
            "EPC length must include PC and tag CRC",
        ));
    }

    if *TAG_PARSE_DEBUG_ENABLED {
        eprintln!(
            "[rfid-silion-compat][tag-parse] epc_total_bytes={epc_total} idx_before_pc={idx}",
            epc_total = epc_total_bytes
        );
    }

    let pc_word = read_u16_from_data(&mut idx, data, "tag metadata missing PC word")?;

    let epc_id_len = epc_total_bytes - 4;
    if *TAG_PARSE_DEBUG_ENABLED {
        eprintln!(
            "[rfid-silion-compat][tag-parse] pc_word=0x{pc:04X} epc_id_len={epc_id_len} idx_before_epc={idx} data_len={len}",
            pc = pc_word,
            len = data.len()
        );
    }
    if idx + epc_id_len > data.len() {
        if *TAG_PARSE_DEBUG_ENABLED {
            eprintln!(
                "[rfid-silion-compat][tag-parse] tag metadata missing EPC ID (idx={idx}, epc_id_len={epc_id_len}, len={len})",
                len = data.len()
            );
            let from = idx.saturating_sub(8);
            let to = core::cmp::min(data.len(), idx + 16);
            eprintln!(
                "[rfid-silion-compat][tag-parse] around idx [{from}..{to}): {window:02X?}",
                window = &data[from..to]
            );
            let cand_from = idx.saturating_sub(6);
            let cand_to = core::cmp::min(data.len().saturating_sub(1), idx + 2);
            for off in cand_from..=cand_to {
                if off + 1 >= data.len() {
                    break;
                }
                let raw = u16::from_be_bytes([data[off], data[off + 1]]);
                if raw % 8 == 0 {
                    let bytes = (raw / 8) as usize;
                    eprintln!(
                        "[rfid-silion-compat][tag-parse] candidate u16@{off} = 0x{raw:04X} ({raw} bits, {bytes} bytes)",
                    );
                } else {
                    eprintln!(
                        "[rfid-silion-compat][tag-parse] candidate u16@{off} = 0x{raw:04X} (non-byte-aligned bits)",
                    );
                }
            }
        }
        return Err(ProtocolError::InvalidResponse(
            "tag metadata missing EPC ID",
        ));
    }
    let epc_id = data[idx..idx + epc_id_len].to_vec();
    idx += epc_id_len;

    let tag_crc = read_u16_from_data(&mut idx, data, "tag metadata missing tag CRC")?;

    if idx != data.len() {
        if *TAG_PARSE_DEBUG_ENABLED {
            eprintln!(
                "[rfid-silion-compat][tag-parse] trailing bytes after tag metadata block (idx={idx}, len={len}, trailing={trailing})",
                len = data.len(),
                trailing = data.len() - idx
            );
        }
        return Err(ProtocolError::InvalidResponse(
            "trailing bytes after tag metadata block",
        ));
    }

    if *TAG_PARSE_DEBUG_ENABLED {
        eprintln!(
            "[rfid-silion-compat][tag-parse] done read_count={read_count:?} rssi={rssi:?} ant={ant:?} freq={freq:?} ts={ts:?} rfu={rfu:?} proto={proto:?} tag_data_bits={tag_data_bits:?} epc_bits={epc_bits} epc_len={epc_len} crc=0x{crc:04X}",
            rssi = rssi_dbm,
            ant = antenna_id,
            freq = frequency_khz,
            ts = timestamp_ms,
            proto = protocol_id,
            tag_data_bits = tag_data_bit_length,
            epc_bits = epc_bit_length,
            epc_len = epc_id.len(),
            crc = tag_crc
        );
    }

    Ok(TagEpcAndMetaData {
        read_count,
        rssi_dbm,
        antenna_id,
        frequency_khz,
        timestamp_ms,
        rfu,
        protocol_id,
        tag_data_bit_length,
        tag_data,
        epc_bit_length,
        pc_word,
        epc_id,
        tag_crc,
    })
}

/// Parse full command `0x21` (Single Tag Inventory) response payload.
///
/// Format is determined by bit4 of the echoed response option byte:
/// - bit4=0: `Option(1) | EPC ID(N) | Tag CRC(2)`
/// - bit4=1: `Option(1) | MetadataFlags(2) | <single-tag metadata payload>`
///
/// Returns the echoed metadata flags together with parsed tag data.
pub fn parse_single_tag_inventory_response(
    request_option_raw: u8,
    data: &[u8],
) -> Result<(MetadataFlags, TagEpcAndMetaData), ProtocolError> {
    if data.is_empty() {
        return Err(ProtocolError::InvalidResponse(
            "single tag inventory response missing option",
        ));
    }

    let response_option = data[0];
    if response_option != request_option_raw {
        return Err(ProtocolError::InvalidResponse(
            "single tag inventory response option does not match request",
        ));
    }

    if (response_option & 0x10) != 0 {
        if data.len() < 3 {
            return Err(ProtocolError::InvalidResponse(
                "single tag inventory response missing metadata flags",
            ));
        }
        let metadata_flags = MetadataFlags::from_raw(u16::from_be_bytes([data[1], data[2]]));
        let tag = parse_single_tag_inventory_payload(metadata_flags, &data[3..])?;
        Ok((metadata_flags, tag))
    } else {
        let metadata_flags = MetadataFlags::NONE;
        let tag = parse_single_tag_inventory_epc_only_payload(&data[1..])?;
        Ok((metadata_flags, tag))
    }
}

fn parse_single_tag_inventory_epc_only_payload(
    data: &[u8],
) -> Result<TagEpcAndMetaData, ProtocolError> {
    if data.len() < 3 {
        return Err(ProtocolError::InvalidResponse(
            "single tag EPC-only payload too short",
        ));
    }

    let epc_id_len = data.len() - 2;
    let epc_id = data[..epc_id_len].to_vec();
    let tag_crc = u16::from_be_bytes([data[epc_id_len], data[epc_id_len + 1]]);

    Ok(TagEpcAndMetaData {
        read_count: None,
        rssi_dbm: None,
        antenna_id: None,
        frequency_khz: None,
        timestamp_ms: None,
        rfu: None,
        protocol_id: None,
        tag_data_bit_length: None,
        tag_data: None,
        epc_bit_length: (epc_id.len() as u16) * 8,
        pc_word: 0,
        epc_id,
        tag_crc,
    })
}

/// Parse the payload portion of command `0x21` (Single Tag Inventory)
/// after `Option(1) + MetadataFlags(2)`.
///
/// This parser follows the Single Tag Inventory response table, where
/// `EPC ID` is variable-length and `Tag CRC` is the final 2 bytes.
pub fn parse_single_tag_inventory_payload(
    metadata_flags: MetadataFlags,
    data: &[u8],
) -> Result<TagEpcAndMetaData, ProtocolError> {
    let mut idx = 0usize;

    let read_count = if metadata_flags.read_count() {
        Some(read_u8_from_data(&mut idx, data, "single tag metadata missing read count")?)
    } else {
        None
    };

    let rssi_dbm = if metadata_flags.rssi() {
        Some(read_u8_from_data(&mut idx, data, "single tag metadata missing RSSI")? as i8)
    } else {
        None
    };

    let antenna_id = if metadata_flags.antenna_id() {
        Some(read_u8_from_data(
            &mut idx,
            data,
            "single tag metadata missing antenna id",
        )?)
    } else {
        None
    };

    let frequency_khz = if metadata_flags.frequency() {
        Some(read_u24_from_data(
            &mut idx,
            data,
            "single tag metadata missing frequency",
        )?)
    } else {
        None
    };

    let timestamp_ms = if metadata_flags.timestamp() {
        Some(read_u32_from_data(
            &mut idx,
            data,
            "single tag metadata missing timestamp",
        )?)
    } else {
        None
    };

    let rfu = if metadata_flags.rfu() {
        Some(read_u16_from_data(&mut idx, data, "single tag metadata missing RFU")?)
    } else {
        None
    };

    let protocol_id = if metadata_flags.protocol_id() {
        Some(read_u8_from_data(
            &mut idx,
            data,
            "single tag metadata missing protocol id",
        )?)
    } else {
        None
    };

    let (tag_data_bit_length, tag_data) = if metadata_flags.data_length() {
        let bits = read_u16_from_data(
            &mut idx,
            data,
            "single tag metadata missing tag data length",
        )?;
        if bits % 8 != 0 {
            return Err(ProtocolError::InvalidResponse(
                "single tag data length must be byte-aligned",
            ));
        }
        let bytes = (bits / 8) as usize;
        if idx + bytes > data.len() {
            return Err(ProtocolError::InvalidResponse(
                "single tag metadata missing tag data bytes",
            ));
        }
        let payload = data[idx..idx + bytes].to_vec();
        idx += bytes;
        (Some(bits), Some(payload))
    } else {
        (None, None)
    };

    if data.len() < idx + 2 {
        return Err(ProtocolError::InvalidResponse(
            "single tag payload missing tag CRC",
        ));
    }

    let epc_id_len = data.len() - idx - 2;
    if epc_id_len == 0 {
        return Err(ProtocolError::InvalidResponse(
            "single tag payload missing EPC ID",
        ));
    }
    let epc_id = data[idx..idx + epc_id_len].to_vec();
    idx += epc_id_len;

    let tag_crc = read_u16_from_data(&mut idx, data, "single tag payload missing tag CRC")?;

    if idx != data.len() {
        return Err(ProtocolError::InvalidResponse(
            "trailing bytes after single tag payload",
        ));
    }

    Ok(TagEpcAndMetaData {
        read_count,
        rssi_dbm,
        antenna_id,
        frequency_khz,
        timestamp_ms,
        rfu,
        protocol_id,
        tag_data_bit_length,
        tag_data,
        epc_bit_length: (epc_id.len() as u16) * 8,
        pc_word: 0,
        epc_id,
        tag_crc,
    })
}

/// TX/RX antenna pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "web-serial", derive(serde::Serialize))]
#[cfg_attr(feature = "web-serial", serde(rename_all = "camelCase"))]
pub struct AntennaPair {
    /// TX logical antenna number.
    pub tx: u8,
    /// RX logical antenna number.
    pub rx: u8,
}

/// Per-antenna power values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "web-serial", derive(serde::Serialize))]
#[cfg_attr(feature = "web-serial", serde(rename_all = "camelCase"))]
pub struct AntennaPower {
    /// TX logical antenna number.
    pub tx: u8,
    /// Read power value in units of `0.01 dBm`.
    ///
    /// The protocol documentation notes that firmware currently applies this
    /// with an effective precision of about `1 dBm`.
    pub read_power: u16,
    /// Write power value in units of `0.01 dBm`.
    ///
    /// The protocol documentation notes that firmware currently applies this
    /// with an effective precision of about `1 dBm`.
    pub write_power: u16,
}

/// Per-antenna power and settling time values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "web-serial", derive(serde::Serialize))]
#[cfg_attr(feature = "web-serial", serde(rename_all = "camelCase"))]
pub struct AntennaPowerSettling {
    /// TX logical antenna number.
    pub tx: u8,
    /// Read power value in units of `0.01 dBm`.
    ///
    /// The protocol documentation notes that firmware currently applies this
    /// with an effective precision of about `1 dBm`.
    pub read_power: u16,
    /// Write power value in units of `0.01 dBm`.
    ///
    /// The protocol documentation notes that firmware currently applies this
    /// with an effective precision of about `1 dBm`.
    pub write_power: u16,
    /// Settling time in microseconds.
    pub settling_time_us: u16,
}

/// Parsed response data for Get Antenna Ports (0x61).
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "web-serial", derive(serde::Serialize))]
#[cfg_attr(feature = "web-serial", serde(rename_all = "camelCase"))]
pub enum AntennaPortsResponse {
    /// Option 0x00, single TX/RX pair for tag access operations.
    AccessPair(AntennaPair),
    /// Option 0x02, inventory antenna pairs.
    InventoryPairs(Vec<AntennaPair>),
    /// Option 0x03, antenna power entries.
    ///
    /// Read/write power values use `0.01 dBm` units in the protocol.
    Power(Vec<AntennaPower>),
    /// Option 0x04, antenna power + settling entries.
    ///
    /// Read/write power values use `0.01 dBm` units in the protocol.
    PowerAndSettling(Vec<AntennaPowerSettling>),
    /// Option 0x05, antenna connection states in order.
    ConnectionStates(Vec<u8>),
}

/// Parse command `0x61` (Get Antenna Ports) response payload.
///
/// `request_option` must be the same option sent in the host command so the
/// parser can decode the option-specific response layout.
///
/// # Examples
/// ```rust
/// use rfid_silion_compat::{
///     parse_antenna_ports_response, AntennaPortsOption, AntennaPortsResponse, AntennaPower,
/// };
///
/// let data = [0x03, 0x01, 0x0B, 0xB8, 0x0B, 0xB8];
/// let parsed = parse_antenna_ports_response(AntennaPortsOption::Power, &data).unwrap();
/// assert_eq!(
///     parsed,
///     AntennaPortsResponse::Power(vec![AntennaPower {
///         tx: 0x01,
///         read_power: 0x0BB8,
///         write_power: 0x0BB8,
///     }])
/// );
/// ```
pub fn parse_antenna_ports_response(
    request_option: AntennaPortsOption,
    data: &[u8],
) -> Result<AntennaPortsResponse, ProtocolError> {
    match request_option {
        AntennaPortsOption::AccessPair => {
            if data.len() != 2 {
                return Err(ProtocolError::InvalidResponse(
                    "get antenna option 0x00 response length must be 2",
                ));
            }
            Ok(AntennaPortsResponse::AccessPair(AntennaPair {
                tx: data[0],
                rx: data[1],
            }))
        }
        AntennaPortsOption::InventoryPairs => {
            if data.len() < 1 || data[0] != 0x02 {
                return Err(ProtocolError::InvalidResponse(
                    "get antenna option 0x02 response must start with option byte 0x02",
                ));
            }
            let pairs = &data[1..];
            if pairs.is_empty() || (pairs.len() % 2 != 0) {
                return Err(ProtocolError::InvalidResponse(
                    "get antenna option 0x02 pairs must be non-empty and even-sized",
                ));
            }
            let mut out = Vec::with_capacity(pairs.len() / 2);
            for ch in pairs.chunks_exact(2) {
                out.push(AntennaPair {
                    tx: ch[0],
                    rx: ch[1],
                });
            }
            Ok(AntennaPortsResponse::InventoryPairs(out))
        }
        AntennaPortsOption::Power => {
            if data.len() < 1 || data[0] != 0x03 {
                return Err(ProtocolError::InvalidResponse(
                    "get antenna option 0x03 response must start with option byte 0x03",
                ));
            }
            let entries = &data[1..];
            if entries.is_empty() || (entries.len() % 5 != 0) {
                return Err(ProtocolError::InvalidResponse(
                    "get antenna option 0x03 entries must be non-empty and 5-byte aligned",
                ));
            }
            let mut out = Vec::with_capacity(entries.len() / 5);
            for ch in entries.chunks_exact(5) {
                out.push(AntennaPower {
                    tx: ch[0],
                    read_power: u16::from_be_bytes([ch[1], ch[2]]),
                    write_power: u16::from_be_bytes([ch[3], ch[4]]),
                });
            }
            Ok(AntennaPortsResponse::Power(out))
        }
        AntennaPortsOption::PowerAndSettling => {
            if data.len() < 1 || data[0] != 0x04 {
                return Err(ProtocolError::InvalidResponse(
                    "get antenna option 0x04 response must start with option byte 0x04",
                ));
            }
            let entries = &data[1..];
            if entries.is_empty() || (entries.len() % 7 != 0) {
                return Err(ProtocolError::InvalidResponse(
                    "get antenna option 0x04 entries must be non-empty and 7-byte aligned",
                ));
            }
            let mut out = Vec::with_capacity(entries.len() / 7);
            for ch in entries.chunks_exact(7) {
                out.push(AntennaPowerSettling {
                    tx: ch[0],
                    read_power: u16::from_be_bytes([ch[1], ch[2]]),
                    write_power: u16::from_be_bytes([ch[3], ch[4]]),
                    settling_time_us: u16::from_be_bytes([ch[5], ch[6]]),
                });
            }
            Ok(AntennaPortsResponse::PowerAndSettling(out))
        }
        AntennaPortsOption::ConnectionStates => {
            if data.is_empty() || data[0] != 0x05 {
                return Err(ProtocolError::InvalidResponse(
                    "get antenna option 0x05 response must start with option byte 0x05",
                ));
            }
            Ok(AntennaPortsResponse::ConnectionStates(data[1..].to_vec()))
        }
    }
}
