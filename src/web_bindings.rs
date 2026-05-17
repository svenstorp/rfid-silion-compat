use std::cell::{RefCell, RefMut};
use std::fmt::Debug;

use js_sys::{Array, Promise, Reflect, Uint8Array};
use serde::Serialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use futures::future::{AbortHandle, Abortable};

use crate::parsers::VersionInfo;
use crate::session::AsyncInventorySession;
use crate::silion_reader::{ReaderAsyncInventoryStartData, SelectOption, SilionReader};
use crate::web_serial::WebSerialTransport;
use crate::{
    AntennaPair, AntennaPortsConfiguration, AntennaPortsOption, AntennaPower,
    AsyncInventoryMessage, InventorySearchFlags, MemBank, MetadataFlags,
    RegionCode, RunPhase,
};

fn js_error(msg: &str) -> JsValue {
    JsValue::from_str(msg)
}

const READER_UNAVAILABLE_ERROR: &str = "reader is not connected or is currently in inventory mode";

fn debug_error<E: Debug>(err: E) -> JsValue {
    JsValue::from_str(&format!("{err:?}"))
}

fn to_js_value<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    serde_wasm_bindgen::to_value(value)
        .map_err(|e| JsValue::from_str(&format!("serialization error: {e}")))
}

/// TypeScript declarations appended to generated bindings for strongly typed JS inputs.
#[wasm_bindgen(typescript_custom_section)]
const TS_INPUT_TYPES: &str = r#"
export interface MetadataFlagsInput {
    readCount?: boolean;
    rssi?: boolean;
    antennaId?: boolean;
    frequency?: boolean;
    timestamp?: boolean;
    rfu?: boolean;
    protocolId?: boolean;
    dataLength?: boolean;
}

export type RegionName = "NorthAmerica" | "China1" | "Europe" | "China2" | "FullFrequencyBand";

export interface RegionCodeInput {
    name: RegionName;
}

export type MemBankName = "Reserved" | "Epc" | "Tid" | "User";

export interface MemBankInput {
    name: MemBankName;
}

export interface EmbeddedCommandInput {
    readTidWords: number;
}

export type SelectOption =
    | { type: "disabled" }
    | { type: "passwordOnly" }
    | { type: "epc"; selectLengthBits: number; selectData: Uint8Array | number[]; invert: boolean }
    | { type: "tid"; selectAddress: number; selectLengthBits: number; selectData: Uint8Array | number[]; invert: boolean }
    | { type: "userMemory"; selectAddress: number; selectLengthBits: number; selectData: Uint8Array | number[]; invert: boolean }
    | { type: "epcBank"; selectAddress: number; selectLengthBits: number; selectData: Uint8Array | number[]; invert: boolean };
"#;

#[wasm_bindgen]
extern "C" {
    /// JavaScript metadata flags input type.
    #[wasm_bindgen(typescript_type = "MetadataFlagsInput")]
    pub type JsMetadataFlagsInput;

    /// JavaScript select content input type.
    #[wasm_bindgen(typescript_type = "SelectContentInput")]
    pub type JsSelectContentInput;

    /// JavaScript region code input type.
    #[wasm_bindgen(typescript_type = "RegionCodeInput")]
    pub type JsRegionCodeInput;

    /// JavaScript memory bank input type.
    #[wasm_bindgen(typescript_type = "MemBankInput")]
    pub type JsMemBankInput;

    /// JavaScript embedded-command input type.
    #[wasm_bindgen(typescript_type = "EmbeddedCommandInput")]
    pub type JsEmbeddedCommandInput;

    /// JavaScript select mode type.
    #[wasm_bindgen(typescript_type = "SelectOption")]
    pub type JsSelectOption;
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RegionCodeJs {
    name: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MemBankJs {
    name: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct MetadataFlagsJs {
    read_count: bool,
    rssi: bool,
    antenna_id: bool,
    frequency: bool,
    timestamp: bool,
    rfu: bool,
    protocol_id: bool,
    data_length: bool,
}

impl TryFrom<VersionInfo> for JsValue {
    type Error = JsValue;

    fn try_from(value: VersionInfo) -> Result<Self, Self::Error> {
        to_js_value(&value)
    }
}

impl TryFrom<AsyncInventoryMessage> for JsValue {
    type Error = JsValue;

    fn try_from(value: AsyncInventoryMessage) -> Result<Self, Self::Error> {
        to_js_value(&value)
    }
}

impl TryFrom<RunPhase> for JsValue {
    type Error = JsValue;

    fn try_from(value: RunPhase) -> Result<Self, Self::Error> {
        to_js_value(&value)
    }
}

impl TryFrom<RegionCode> for JsValue {
    type Error = JsValue;

    fn try_from(value: RegionCode) -> Result<Self, Self::Error> {
        let name = match value {
            RegionCode::NorthAmerica => "NorthAmerica",
            RegionCode::China1 => "China1",
            RegionCode::Europe => "Europe",
            RegionCode::China2 => "China2",
            RegionCode::FullFrequencyBand => "FullFrequencyBand",
        };
        to_js_value(&RegionCodeJs { name })
    }
}

impl TryFrom<MetadataFlags> for JsValue {
    type Error = JsValue;

    fn try_from(value: MetadataFlags) -> Result<Self, Self::Error> {
        to_js_value(&MetadataFlagsJs {
            read_count: value.read_count(),
            rssi: value.rssi(),
            antenna_id: value.antenna_id(),
            frequency: value.frequency(),
            timestamp: value.timestamp(),
            rfu: value.rfu(),
            protocol_id: value.protocol_id(),
            data_length: value.data_length(),
        })
    }
}

impl TryFrom<MemBank> for JsValue {
    type Error = JsValue;

    fn try_from(value: MemBank) -> Result<Self, Self::Error> {
        let name = match value {
            MemBank::Reserved => "Reserved",
            MemBank::Epc => "Epc",
            MemBank::Tid => "Tid",
            MemBank::User => "User",
        };
        to_js_value(&MemBankJs { name })
    }
}

fn parse_metadata_flags(value: JsValue) -> Result<MetadataFlags, JsValue> {
    if value.is_object() {
        let mut flags = MetadataFlags::NONE;
        for (name, setter) in [
            (
                "readCount",
                MetadataFlags::with_read_count as fn(MetadataFlags, bool) -> MetadataFlags,
            ),
            ("rssi", MetadataFlags::with_rssi),
            ("antennaId", MetadataFlags::with_antenna_id),
            ("frequency", MetadataFlags::with_frequency),
            ("timestamp", MetadataFlags::with_timestamp),
            ("rfu", MetadataFlags::with_rfu),
            ("protocolId", MetadataFlags::with_protocol_id),
            ("dataLength", MetadataFlags::with_data_length),
        ] {
            let field_value = Reflect::get(&value, &JsValue::from_str(name))
                .map_err(|_| js_error("metadataFlags field is invalid"))?;
            if !field_value.is_undefined() && !field_value.is_null() {
                let enabled = field_value
                    .as_bool()
                    .ok_or_else(|| js_error("metadataFlags fields must be booleans"))?;
                flags = setter(flags, enabled);
            }
        }

        return Ok(flags);
    }

    Err(js_error(
        "metadataFlags must be an object with typed boolean fields",
    ))
}

fn parse_optional_metadata_flags(value: Option<JsValue>) -> Result<Option<MetadataFlags>, JsValue> {
    match value {
        Some(v) if v.is_null() || v.is_undefined() => Ok(None),
        Some(v) => parse_metadata_flags(v).map(Some),
        None => Ok(None),
    }
}

fn parse_select_option(value: JsValue) -> Result<SelectOption, JsValue> {
    serde_wasm_bindgen::from_value(value)
        .map_err(|e| JsValue::from_str(&format!("invalid select option: {e}")))
}

fn parse_embedded_command(
    value: &JsValue,
) -> Option<crate::command::InventoryEmbeddedCommandContent> {
    if value.is_object() {
        // Look for { readTidWords: N }
        let tid_words = js_sys::Reflect::get(value, &JsValue::from_str("readTidWords"))
            .ok()?
            .as_f64()?;
        if tid_words > 0.0 && tid_words <= 32.0 && tid_words.fract() == 0.0 {
            return Some(
                crate::command::InventoryEmbeddedCommandContent::ReadTagData(
                    crate::command::EmbeddedReadTagData {
                        read_membank: crate::command::MemBank::Tid,
                        read_address_words: 0,
                        word_count: tid_words as u8,
                    },
                ),
            );
        }
    }
    None
}

fn parse_mem_bank(value: JsValue) -> Result<MemBank, JsValue> {
    if value.is_object() {
        let name_value = Reflect::get(&value, &JsValue::from_str("name"))
            .map_err(|_| js_error("memBank.name is invalid"))?;
        if !name_value.is_undefined() && !name_value.is_null() {
            let name = name_value
                .as_string()
                .ok_or_else(|| js_error("memBank.name must be a string"))?;
            let mem_bank = match name.as_str() {
                "Reserved" => MemBank::Reserved,
                "Epc" => MemBank::Epc,
                "Tid" => MemBank::Tid,
                "User" => MemBank::User,
                _ => return Err(js_error("unknown memBank.name")),
            };
            return Ok(mem_bank);
        }
    }

    Err(js_error("memBank must be an object with { name }"))
}

fn parse_region_code(value: JsValue) -> Result<RegionCode, JsValue> {
    if value.is_object() {
        let name_value = Reflect::get(&value, &JsValue::from_str("name"))
            .map_err(|_| js_error("regionCode.name is invalid"))?;
        if !name_value.is_undefined() && !name_value.is_null() {
            let name = name_value
                .as_string()
                .ok_or_else(|| js_error("regionCode.name must be a string"))?;
            let region = match name.as_str() {
                "NorthAmerica" => RegionCode::NorthAmerica,
                "China1" => RegionCode::China1,
                "Europe" => RegionCode::Europe,
                "China2" => RegionCode::China2,
                "FullFrequencyBand" => RegionCode::FullFrequencyBand,
                _ => return Err(js_error("unknown regionCode.name")),
            };
            return Ok(region);
        }
    }

    Err(js_error("regionCode must be an object with { name }"))
}

fn js_value_to_bytes(data: JsValue) -> Result<Vec<u8>, JsValue> {
    if data.is_instance_of::<Uint8Array>() {
        return Ok(Uint8Array::new(&data).to_vec());
    }

    if Array::is_array(&data) {
        let values = Array::from(&data);
        let mut out = Vec::with_capacity(values.length() as usize);
        for v in values.iter() {
            let n = v
                .as_f64()
                .ok_or_else(|| js_error("array element is not a number"))?;
            if !(0.0..=255.0).contains(&n) {
                return Err(js_error("array element is out of u8 range"));
            }
            out.push(n as u8);
        }
        return Ok(out);
    }

    Err(js_error("expected Uint8Array or number[]"))
}

/// Browser-facing wasm-bindgen wrapper around `SilionReader<WebSerialTransport>`.
///
/// JavaScript can use this class to connect over Web Serial, send transactions,
/// and drive asynchronous inventory receive loops.
#[wasm_bindgen(js_name = SilionReader)]
pub struct WasmSilionReader {
    reader: RefCell<Option<SilionReader<WebSerialTransport>>>,
    session: RefCell<Option<AsyncInventorySession<WebSerialTransport>>>,
    pending_recv_abort: RefCell<Option<AbortHandle>>,
}

#[wasm_bindgen(js_class = SilionReader)]
impl WasmSilionReader {
    /// Helper: Borrow reader mutably, or error if not connected/in inventory.
    fn reader_mut(&self) -> Result<RefMut<'_, SilionReader<WebSerialTransport>>, JsValue> {
        let reader = self.reader.borrow_mut();
        if reader.is_none() {
            return Err(js_error(READER_UNAVAILABLE_ERROR));
        }
        Ok(RefMut::map(reader, |slot| {
            slot.as_mut()
                .expect("reader presence was checked before RefMut::map")
        }))
    }

    /// Helper: Build an already-resolved promise used to yield back to the JS event loop.
    async fn yield_once() {
        let _ = JsFuture::from(Promise::resolve(&JsValue::UNDEFINED)).await;
    }

    /// Open a browser serial port and create a reader instance.
    ///
    /// Call from a user gesture handler (for example a button click), because
    /// Web Serial `requestPort` requires direct user interaction.
    #[wasm_bindgen(js_name = connect)]
    pub async fn connect(baud_rate: u32) -> Result<WasmSilionReader, JsValue> {
        let transport = WebSerialTransport::request_port(baud_rate).await?;
        Ok(Self {
            reader: RefCell::new(Some(SilionReader::new(transport))),
            session: RefCell::new(None),
            pending_recv_abort: RefCell::new(None),
        })
    }

    /// Run one raw command transaction and return response payload bytes.
    #[wasm_bindgen(js_name = transact)]
    pub async fn transact(&self, command: u8, data: &[u8]) -> Result<Uint8Array, JsValue> {
        let mut reader = self.reader_mut()?;
        let frame = reader
            .transact_raw(command, data)
            .await
            .map_err(debug_error)?;
        Ok(Uint8Array::from(frame.data.as_slice()))
    }

    /// Read version information and return it as a JavaScript object.
    #[wasm_bindgen(js_name = getVersion)]
    pub async fn get_version(&self) -> Result<JsValue, JsValue> {
        let mut reader = self.reader_mut()?;
        let version = reader.get_version().await.map_err(debug_error)?;
        version.try_into()
    }

    /// Run command `0x04` (Boot Firmware).
    #[wasm_bindgen(js_name = bootFirmware)]
    pub async fn boot_firmware(&self) -> Result<(), JsValue> {
        let mut reader = self.reader_mut()?;
        reader.boot_firmware().await.map_err(debug_error)
    }

    /// Run command `0x09` (Boot Bootloader).
    #[wasm_bindgen(js_name = bootBootloader)]
    pub async fn boot_bootloader(&self) -> Result<(), JsValue> {
        let mut reader = self.reader_mut()?;
        reader.boot_bootloader().await.map_err(debug_error)
    }

    /// Run command `0x0C` (Get Run Phase).
    #[wasm_bindgen(js_name = getRunPhase)]
    pub async fn get_run_phase(&self) -> Result<JsValue, JsValue> {
        let mut reader = self.reader_mut()?;
        let phase = reader.get_run_phase().await.map_err(debug_error)?;
        phase.try_into()
    }

    /// Run command `0x10` (Get Serial Number).
    #[wasm_bindgen(js_name = getSerialNumber)]
    pub async fn get_serial_number(&self, option: u8, data_flags: u8) -> Result<JsValue, JsValue> {
        let mut reader = self.reader_mut()?;
        let serial = reader
            .get_serial_number(option, data_flags)
            .await
            .map_err(debug_error)?;
        to_js_value(&serial)
    }

    /// Run command `0x63` (Get Current Tag Protocol).
    #[wasm_bindgen(js_name = getCurrentTagProtocol)]
    pub async fn get_current_tag_protocol(&self) -> Result<u16, JsValue> {
        let mut reader = self.reader_mut()?;
        reader.get_current_tag_protocol().await.map_err(debug_error)
    }

    /// Run command `0x97` (Set Current Region).
    ///
    /// Accepts an object with `{ name }`.
    ///
    /// JavaScript examples:
    ///
    /// ```javascript
    /// await reader.setCurrentRegion({ name: "Europe" });
    /// ```
    #[wasm_bindgen(js_name = setCurrentRegion)]
    pub async fn set_current_region(&self, region_code: JsRegionCodeInput) -> Result<(), JsValue> {
        let mut reader = self.reader_mut()?;
        let region = parse_region_code(region_code.into())?;
        reader.set_current_region(region).await.map_err(debug_error)
    }

    /// Run command `0x67` (Get Current Region) and return a typed region object.
    ///
    /// JavaScript return shape:
    ///
    /// ```javascript
    /// const region = await reader.getCurrentRegion();
    /// // { name: "Europe" }
    /// ```
    #[wasm_bindgen(js_name = getCurrentRegion)]
    pub async fn get_current_region(&self) -> Result<JsValue, JsValue> {
        let mut reader = self.reader_mut()?;
        let region = reader.get_current_region().await.map_err(debug_error)?;
        region.try_into()
    }

    /// Run command `0x71` (Get Available Regions) and return typed region objects.
    ///
    /// JavaScript return shape:
    ///
    /// ```javascript
    /// const regions = await reader.getAvailableRegions();
    /// // [ { name: "NorthAmerica" }, { name: "Europe" }, ... ]
    /// ```
    #[wasm_bindgen(js_name = getAvailableRegions)]
    pub async fn get_available_regions(&self) -> Result<JsValue, JsValue> {
        let mut reader = self.reader_mut()?;
        let regions = reader.get_available_regions().await.map_err(debug_error)?;
        let out = Array::new();
        for region in regions {
            let js_region: JsValue = region.try_into()?;
            out.push(&js_region);
        }
        Ok(out.into())
    }

    /// Run command `0x72` (Get Current Temperature).
    #[wasm_bindgen(js_name = getCurrentTemperature)]
    pub async fn get_current_temperature(&self) -> Result<u8, JsValue> {
        let mut reader = self.reader_mut()?;
        reader.get_current_temperature().await.map_err(debug_error)
    }

    /// Run command `0x66` (Get GPI).
    #[wasm_bindgen(js_name = getGpi)]
    pub async fn get_gpi(&self) -> Result<Uint8Array, JsValue> {
        let mut reader = self.reader_mut()?;
        let states = reader.get_gpi().await.map_err(debug_error)?;
        Ok(Uint8Array::from(states.as_slice()))
    }

    /// Run command `0x96` with empty payload to get GPO states.
    #[wasm_bindgen(js_name = getGpoStates)]
    pub async fn get_gpo_states(&self) -> Result<Uint8Array, JsValue> {
        let mut reader = self.reader_mut()?;
        let states = reader.get_gpo_states().await.map_err(debug_error)?;
        Ok(Uint8Array::from(states.as_slice()))
    }

    /// Set command `0x91` access pair configuration.
    #[wasm_bindgen(js_name = setAntennaAccessPair)]
    pub async fn set_antenna_access_pair(&self, tx: u8, rx: u8) -> Result<(), JsValue> {
        let mut reader = self.reader_mut()?;
        reader
            .set_antenna_ports(&AntennaPortsConfiguration::AccessPair(AntennaPair {
                tx,
                rx,
            }))
            .await
            .map_err(debug_error)
    }

    /// Set command `0x91` inventory-pairs configuration from `[tx, rx, ...]` bytes.
    #[wasm_bindgen(js_name = setAntennaInventoryPairs)]
    pub async fn set_antenna_inventory_pairs(&self, pairs: &[u8]) -> Result<(), JsValue> {
        if pairs.is_empty() || (pairs.len() % 2 != 0) {
            return Err(js_error(
                "pairs must be non-empty and contain [tx, rx] byte pairs",
            ));
        }

        let mut reader = self.reader_mut()?;
        let mut out = Vec::with_capacity(pairs.len() / 2);
        for chunk in pairs.chunks_exact(2) {
            out.push(AntennaPair {
                tx: chunk[0],
                rx: chunk[1],
            });
        }
        reader
            .set_antenna_ports(&AntennaPortsConfiguration::InventoryPairs(out))
            .await
            .map_err(debug_error)
    }

    /// Set command `0x91` power for one TX antenna.
    #[wasm_bindgen(js_name = setAntennaPower)]
    pub async fn set_antenna_power(
        &self,
        tx: u8,
        read_power: u16,
        write_power: u16,
    ) -> Result<(), JsValue> {
        let mut reader = self.reader_mut()?;
        reader
            .set_antenna_ports(&AntennaPortsConfiguration::Power(vec![AntennaPower {
                tx,
                read_power,
                write_power,
            }]))
            .await
            .map_err(debug_error)
    }

    /// Run command `0x65` (Get Frequency Hopping table form).
    #[wasm_bindgen(js_name = getFrequencyHoppingTable)]
    pub async fn get_frequency_hopping_table(&self) -> Result<JsValue, JsValue> {
        let mut reader = self.reader_mut()?;
        let table = reader
            .get_frequency_hopping_table()
            .await
            .map_err(debug_error)?;
        to_js_value(&table)
    }

    /// Run command `0x65` with option `0x01` (Regulatory Hopping Time).
    #[wasm_bindgen(js_name = getRegulatoryHopTime)]
    pub async fn get_regulatory_hop_time(&self) -> Result<JsValue, JsValue> {
        let mut reader = self.reader_mut()?;
        let hop = reader
            .get_regulatory_hop_time()
            .await
            .map_err(debug_error)?;
        to_js_value(&hop)
    }

    /// Run command `0x61` (Get Antenna Ports) and decode by option.
    #[wasm_bindgen(js_name = getAntennaPorts)]
    pub async fn get_antenna_ports(&self, option: u8) -> Result<JsValue, JsValue> {
        let mut reader = self.reader_mut()?;
        let parsed_option = AntennaPortsOption::from_u8(option)
            .ok_or_else(|| js_error("unknown antenna ports option"))?;
        let response = reader
            .get_antenna_ports(parsed_option)
            .await
            .map_err(debug_error)?;
        to_js_value(&response)
    }

    /// Run command `0x6A` (Get Reader Configuration).
    #[wasm_bindgen(js_name = getReaderConfiguration)]
    pub async fn get_reader_configuration(&self, option: u8, key: u8) -> Result<JsValue, JsValue> {
        let mut reader = self.reader_mut()?;
        let value = reader
            .get_reader_configuration(option, key)
            .await
            .map_err(debug_error)?;
        to_js_value(&value)
    }

    /// Run command `0x6B` (Get Protocol Configuration).
    #[wasm_bindgen(js_name = getProtocolConfiguration)]
    pub async fn get_protocol_configuration(
        &self,
        protocol_value: u8,
        parameter: u8,
    ) -> Result<JsValue, JsValue> {
        let mut reader = self.reader_mut()?;
        let value = reader
            .get_protocol_configuration(protocol_value, parameter)
            .await
            .map_err(debug_error)?;
        to_js_value(&value)
    }

    /// Run command `0x21` (Single Tag Inventory).
    ///
    /// Performs a single tag read with the specified timeout. Returns tag EPC and metadata.
    ///
    /// # Arguments
    ///
    /// * `timeout_ms` - Timeout in milliseconds
    /// * `select_option` - Select option typed object
    /// * `metadata_flags` - Optional metadata flags typed object (`undefined`/`null` means EPC-only)
    ///
    /// Note: metadata mode is derived automatically from `metadata_flags` presence.
    ///
    /// JavaScript examples:
    ///
    /// ```javascript
    /// // Typed objects
    /// await reader.singleTagInventory(
    ///   1000,
    ///   { kind: "epc", selectLengthBits: 96, selectData: new Uint8Array(12), invert: false },
    ///   { rssi: true, antennaId: true, timestamp: true },
    /// );
    ///
    /// // EPC-only (no metadata)
    /// await reader.singleTagInventory(1000, { kind: "disabled" }, undefined);
    /// ```
    #[wasm_bindgen(js_name = singleTagInventory)]
    pub async fn single_tag_inventory(
        &self,
        timeout_ms: u16,
        select_option: JsSelectOption,
        metadata_flags: Option<JsMetadataFlagsInput>,
    ) -> Result<JsValue, JsValue> {
        let select_option = parse_select_option(select_option.into())?;
        let metadata_flags = parse_optional_metadata_flags(metadata_flags.map(Into::into))?;

        let mut reader = self.reader_mut()?;
        let tag = reader
            .single_tag_inventory(timeout_ms, select_option, metadata_flags)
            .await
            .map_err(debug_error)?;
        to_js_value(&tag)
    }

    /// Run command `0x28` (Read Tag Data).
    ///
    /// # Arguments
    ///
    /// * `timeout_ms` - Timeout in milliseconds
    /// * `select_option` - Select option typed object
    /// * `metadata_flags` - Optional metadata flags typed object (`undefined`/`null` means EPC + CRC only)
    /// * `mem_bank` - Memory bank typed object `{ name: "Reserved"|"Epc"|"Tid"|"User" }`
    /// * `read_address_words` - Start address in words
    /// * `word_count` - Number of words to read
    ///
    /// Note: metadata mode is derived automatically from `metadata_flags` presence.
    ///
    /// JavaScript example:
    ///
    /// ```javascript
    /// await reader.readTagData(
    ///   1000,
    ///   { type: "epc", selectLengthBits: 96, selectData: new Uint8Array(12), invert: false },
    ///   { dataLength: true },
    ///   { name: "Tid" },
    ///   0,
    ///   6,
    /// );
    /// ```
    #[wasm_bindgen(js_name = readTagData)]
    pub async fn read_tag_data(
        &self,
        timeout_ms: u16,
        select_option: JsSelectOption,
        metadata_flags: Option<JsMetadataFlagsInput>,
        mem_bank: JsMemBankInput,
        read_address_words: u32,
        word_count: u8,
    ) -> Result<JsValue, JsValue> {
        let select_option = parse_select_option(select_option.into())?;
        let metadata_flags = parse_optional_metadata_flags(metadata_flags.map(Into::into))?;
        let mem_bank = parse_mem_bank(mem_bank.into())?;

        let mut reader = self.reader_mut()?;
        let tag = reader
            .read_tag_data(
                timeout_ms,
                select_option,
                metadata_flags,
                mem_bank,
                read_address_words,
                word_count,
            )
            .await
            .map_err(debug_error)?;
        to_js_value(&tag)
    }

    /// Start asynchronous inventory with a basic default configuration.
    #[wasm_bindgen(js_name = startInventory)]
    pub async fn start_inventory(
        &self,
        embedded_command: Option<JsEmbeddedCommandInput>,
    ) -> Result<(), JsValue> {
        if self.session.borrow().is_some() {
            return Err(js_error("inventory is already running"));
        }

        let mut reader = self
            .reader
            .borrow_mut()
            .take()
            .ok_or_else(|| js_error("reader is not connected"))?;

        let search_flags = InventorySearchFlags::new()
            .with_async_heartbeat(true)
            .with_async_auto_stop(false);

        let start_data = ReaderAsyncInventoryStartData {
            metadata_flags: MetadataFlags::default()
                .with_rssi(true)
                .with_antenna_id(true)
                .with_timestamp(true)
                .with_data_length(true),
            select_option: SelectOption::Disabled,
            search_flags: search_flags.with_embedded_command(embedded_command.is_some()),
            access_password: None,
            embedded_command_content: embedded_command
                .map(Into::into)
                .as_ref()
                .and_then(parse_embedded_command),
        };

        reader
            .enable_async_inventory(&start_data)
            .await
            .map_err(debug_error)?;

        *self.session.borrow_mut() = Some(reader.into_async_session());
        Ok(())
    }

    /// Receive one asynchronous inventory message and return a JS object.
    #[wasm_bindgen(js_name = recvInventoryMessage)]
    pub async fn recv_inventory_message(&self) -> Result<JsValue, JsValue> {
        let mut session = self
            .session
            .borrow_mut()
            .take()
            .ok_or_else(|| js_error("inventory is not running"))?;

        let (abort_handle, abort_registration) = AbortHandle::new_pair();
        *self.pending_recv_abort.borrow_mut() = Some(abort_handle);

        let recv_result = Abortable::new(session.recv(), abort_registration).await;

        *self.pending_recv_abort.borrow_mut() = None;
        *self.session.borrow_mut() = Some(session);

        match recv_result {
            Ok(Ok(message)) => message.try_into(),
            Ok(Err(e)) => Err(debug_error(e)),
            Err(_) => Err(js_error("inventory receive aborted")),
        }
    }

    /// Stop asynchronous inventory and return to command mode.
    #[wasm_bindgen(js_name = stopInventory)]
    pub async fn stop_inventory(&self) -> Result<(), JsValue> {
        if let Some(abort) = self.pending_recv_abort.borrow_mut().take() {
            abort.abort();
        }

        loop {
            if let Some(session) = self.session.borrow_mut().take() {
                let reader = session.stop_no_wait().await.map_err(debug_error)?;
                *self.reader.borrow_mut() = Some(reader);
                return Ok(());
            }

            if self.reader.borrow().is_some() {
                return Ok(());
            }

            Self::yield_once().await;
        }
    }

    /// Close the underlying serial device.
    ///
    /// If inventory is currently running, this method stops inventory first.
    #[wasm_bindgen(js_name = close)]
    pub async fn close(&self) -> Result<(), JsValue> {
        if self.session.borrow().is_some() {
            self.stop_inventory().await?;
        }
        let reader = self
            .reader
            .borrow_mut()
            .take()
            .ok_or_else(|| js_error("reader is not connected"))?;
        let mut transport = reader.into_inner();
        transport.close().await
    }

    /// Return whether asynchronous inventory is currently running.
    #[wasm_bindgen(js_name = isInventoryRunning)]
    pub fn is_inventory_running(&self) -> bool {
        self.session.borrow().is_some()
    }
}

/// Convert a JavaScript byte array to a hexadecimal string.
#[wasm_bindgen(js_name = bytesToHex)]
pub fn bytes_to_hex(data: JsValue) -> Result<String, JsValue> {
    let bytes = js_value_to_bytes(data)?;

    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02X}"));
    }
    Ok(out)
}

/// Convert a JavaScript array of numbers into a `Uint8Array`.
#[wasm_bindgen(js_name = arrayToBytes)]
pub fn array_to_bytes(values: Array) -> Result<Uint8Array, JsValue> {
    let bytes = js_value_to_bytes(values.into())?;
    Ok(Uint8Array::from(bytes.as_slice()))
}
