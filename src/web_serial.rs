use std::collections::VecDeque;

use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use crate::transport::ReaderTransport;

#[wasm_bindgen(inline_js = r#"
export async function webSerialRequestPort(baudRate) {
  if (!('serial' in navigator)) {
    throw new Error('Web Serial API is not available in this browser');
  }

  const port = await navigator.serial.requestPort();
  await port.open({ baudRate });

  if (!port.readable || !port.writable) {
    throw new Error('Selected serial device does not expose readable/writable streams');
  }

  const reader = port.readable.getReader();
  const writer = port.writable.getWriter();
  return { port, reader, writer };
}

export async function webSerialWrite(handle, data) {
  await handle.writer.write(data);
}

export async function webSerialRead(handle) {
  const result = await handle.reader.read();
  if (result.done) {
    return null;
  }
  return result.value;
}

export async function webSerialClose(handle) {
  try {
    await handle.reader.cancel();
  } catch (_) {
    // Reader may already be closed or canceled.
  }

  handle.reader.releaseLock();
  handle.writer.releaseLock();
  await handle.port.close();
}
"#)]
extern "C" {
    #[wasm_bindgen(catch)]
    fn webSerialRequestPort(baud_rate: u32) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch)]
    fn webSerialWrite(
        handle: &JsValue,
        data: &js_sys::Uint8Array,
    ) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch)]
    fn webSerialRead(handle: &JsValue) -> Result<js_sys::Promise, JsValue>;

    #[wasm_bindgen(catch)]
    fn webSerialClose(handle: &JsValue) -> Result<js_sys::Promise, JsValue>;
}

/// Browser transport backed by the Web Serial API.
///
/// This transport is only available on wasm32 targets with the `web-serial`
/// crate feature enabled.
///
/// Unlike [`crate::SerialTransport`], this transport is fully
/// asynchronous because Web Serial I/O is Promise based.
pub struct WebSerialTransport {
    handle: JsValue,
    rx_buffer: VecDeque<u8>,
}

impl WebSerialTransport {
    /// Prompt the user to select a serial device and open it with `baud_rate`.
    ///
    /// The browser may require this to be called from a user gesture handler
    /// (for example a button click).
    pub async fn request_port(baud_rate: u32) -> Result<Self, JsValue> {
        let handle_promise = webSerialRequestPort(baud_rate)?;
        let handle = JsFuture::from(handle_promise).await?;
        Ok(Self {
            handle,
            rx_buffer: VecDeque::new(),
        })
    }

    /// Construct a transport from an existing JavaScript handle object.
    ///
    /// The handle must match the structure produced by
    /// [`WebSerialTransport::request_port`].
    pub fn from_handle(handle: JsValue) -> Self {
        Self {
            handle,
            rx_buffer: VecDeque::new(),
        }
    }

    /// Consume transport and return the wrapped JavaScript handle.
    pub fn into_handle(self) -> JsValue {
        self.handle
    }

    /// Write all bytes to the device.
    pub async fn write_all(&mut self, data: &[u8]) -> Result<(), JsValue> {
        let payload = js_sys::Uint8Array::from(data);
        let promise = webSerialWrite(&self.handle, &payload)?;
        let _ = JsFuture::from(promise).await?;
        Ok(())
    }

    /// Read exactly `buf.len()` bytes from the device.
    ///
    /// This waits for additional incoming chunks from the browser stream until
    /// the requested length has been filled.
    pub async fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), JsValue> {
        let mut offset = 0usize;

        while offset < buf.len() {
            while offset < buf.len() {
                if let Some(byte) = self.rx_buffer.pop_front() {
                    buf[offset] = byte;
                    offset += 1;
                } else {
                    break;
                }
            }

            if offset == buf.len() {
                break;
            }

            let promise = webSerialRead(&self.handle)?;
            let chunk = JsFuture::from(promise).await?;

            if chunk.is_null() || chunk.is_undefined() {
                return Err(JsValue::from_str(
                    "serial stream closed before enough bytes were read",
                ));
            }

            let bytes = js_sys::Uint8Array::new(&chunk);
            if bytes.length() == 0 {
                continue;
            }

            let mut temp = vec![0u8; bytes.length() as usize];
            bytes.copy_to(&mut temp);
            self.rx_buffer.extend(temp);
        }

        Ok(())
    }

    /// Cancel stream reader/writer locks and close the serial port.
    pub async fn close(&mut self) -> Result<(), JsValue> {
        let promise = webSerialClose(&self.handle)?;
        let _ = JsFuture::from(promise).await?;
        Ok(())
    }
}

impl ReaderTransport for WebSerialTransport {
    type Error = JsValue;

    async fn write_all(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        WebSerialTransport::write_all(self, data).await
    }

    async fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        WebSerialTransport::read_exact(self, buf).await
    }
}
