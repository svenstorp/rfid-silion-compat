use std::cell::{RefCell, RefMut};
use std::fmt::Debug;

use js_sys::{Array, Promise, Reflect, Uint8Array};
use serde::Serialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use futures::future::{AbortHandle, Abortable};

use crate::command::AsyncInventoryStartData;
use crate::parsers::VersionInfo;
use crate::session::AsyncInventorySession;
use crate::silion_reader::SilionReader;
use crate::web_serial::WebSerialTransport;
use crate::{
    AntennaPair, AntennaPortsConfiguration, AntennaPortsOption, AntennaPower,
    AsyncInventoryMessage, InventoryOption, InventorySearchFlags, MetadataFlags, RegionCode,
    RunPhase, SelectContent, SelectMode, SelectOptionBits,
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

fn parse_select_content(value: JsValue) -> Result<SelectContent, JsValue> {
    if value.is_null() || value.is_undefined() || !value.is_object() {
        return Err(js_error(
            "select_content must be an object: { addressBits, bitLen, data }",
        ));
    }

    let address_bits = Reflect::get(&value, &JsValue::from_str("addressBits"))
        .map_err(|_| js_error("select_content.addressBits is missing"))?
        .as_f64()
        .ok_or_else(|| js_error("select_content.addressBits must be a number"))?;
    if !(0.0..=u32::MAX as f64).contains(&address_bits) || address_bits.fract() != 0.0 {
        return Err(js_error("select_content.addressBits must be a u32 integer"));
    }

    let bit_len = Reflect::get(&value, &JsValue::from_str("bitLen"))
        .map_err(|_| js_error("select_content.bitLen is missing"))?
        .as_f64()
        .ok_or_else(|| js_error("select_content.bitLen must be a number"))?;
    if !(0.0..=u8::MAX as f64).contains(&bit_len) || bit_len.fract() != 0.0 {
        return Err(js_error("select_content.bitLen must be an integer in 0..=255"));
    }
    let bit_len = bit_len as u8;

    let data_value = Reflect::get(&value, &JsValue::from_str("data"))
        .map_err(|_| js_error("select_content.data is missing"))?;
    let data = js_value_to_bytes(data_value)?;

    let expected_len = if bit_len == 0 {
        0
    } else {
        ((bit_len as usize) + 7) / 8
    };
    if data.len() != expected_len {
        return Err(js_error(
            "select_content.data length does not match bitLen (must be ceil(bitLen/8) bytes)",
        ));
    }

    Ok(SelectContent {
        address_bits: address_bits as u32,
        bit_len,
        data,
    })
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

    /// Run command `0x97` (Set Current Region) using a raw region code byte.
    #[wasm_bindgen(js_name = setCurrentRegion)]
    pub async fn set_current_region(&self, region_code: u8) -> Result<(), JsValue> {
        let mut reader = self.reader_mut()?;
        let region =
            RegionCode::from_u8(region_code).ok_or_else(|| js_error("unknown region code"))?;
        reader.set_current_region(region).await.map_err(debug_error)
    }

    /// Run command `0x67` (Get Current Region) and return the raw region code.
    #[wasm_bindgen(js_name = getCurrentRegion)]
    pub async fn get_current_region(&self) -> Result<u8, JsValue> {
        let mut reader = self.reader_mut()?;
        let region = reader.get_current_region().await.map_err(debug_error)?;
        Ok(region.as_u8())
    }

    /// Run command `0x71` (Get Available Regions) and return region code bytes.
    #[wasm_bindgen(js_name = getAvailableRegions)]
    pub async fn get_available_regions(&self) -> Result<Uint8Array, JsValue> {
        let mut reader = self.reader_mut()?;
        let regions = reader.get_available_regions().await.map_err(debug_error)?;
        let out: Vec<u8> = regions.into_iter().map(|r| r.as_u8()).collect();
        Ok(Uint8Array::from(out.as_slice()))
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
    /// * `option_raw` - Inventory option as raw byte (corresponds to [`InventoryOption`] values)
    /// * `metadata_flags_raw` - Metadata flags as raw u16 (corresponds to [`MetadataFlags`] values)
    /// * `select_content` - Optional object `{ addressBits, bitLen, data }`, or `undefined`
    #[wasm_bindgen(js_name = singleTagInventory)]
    pub async fn single_tag_inventory(
        &self,
        timeout_ms: u16,
        option_raw: u8,
        metadata_flags_raw: u16,
        select_content: Option<JsValue>,
    ) -> Result<JsValue, JsValue> {
        let option = InventoryOption::from_raw(option_raw);
        let select_bits = SelectOptionBits::from_raw(option.select_option_bits());
        if select_bits.extended_data_length() {
            return Err(js_error(
                "option bit 0x20 (extended select data length) is not supported by this API",
            ));
        }

        let select = match select_content {
            Some(v) => {
                if v.is_null() || v.is_undefined() {
                    None
                } else {
                    Some(parse_select_content(v)?)
                }
            }
            None => None,
        };

        match select_bits.mode() {
            Some(SelectMode::Disabled) => {
                if select.is_some() {
                    return Err(js_error(
                        "select_content must be undefined when SelectMode is Disabled (0x00)",
                    ));
                }
            }
            Some(SelectMode::PasswordOnly) => {
                if select.is_some() {
                    return Err(js_error(
                        "select_content must be undefined when SelectMode is PasswordOnly (0x05)",
                    ));
                }
                return Err(js_error(
                    "SelectMode::PasswordOnly (0x05) is not supported by singleTagInventory because access password is not exposed",
                ));
            }
            Some(SelectMode::Epc)
            | Some(SelectMode::Tid)
            | Some(SelectMode::UserMemory)
            | Some(SelectMode::EpcBank) => {
                if select.is_none() {
                    return Err(js_error(
                        "select_content is required when SelectMode is 0x01..0x04",
                    ));
                }
            }
            None => {
                return Err(js_error(
                    "unsupported select mode in option_raw; expected SelectMode 0x00..0x05",
                ));
            }
        }

        let mut reader = self.reader_mut()?;
        let tag = reader
            .single_tag_inventory(
                timeout_ms,
                option,
                MetadataFlags::from_raw(metadata_flags_raw),
                select,
            )
            .await
            .map_err(debug_error)?;
        to_js_value(&tag)
    }

    /// Start asynchronous inventory with a basic default configuration.
    #[wasm_bindgen(js_name = startInventory)]
    pub async fn start_inventory(&self) -> Result<(), JsValue> {
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

        let start_data = AsyncInventoryStartData {
            metadata_flags: MetadataFlags::default()
                .with_rssi(true)
                .with_antenna_id(true)
                .with_timestamp(true),
            option: InventoryOption::default(),
            search_flags,
            access_password: None,
            select_content: None,
            embedded_command_content: None,
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
