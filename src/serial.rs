use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_serial::{SerialPortBuilderExt, SerialStream};

use crate::transport::ReaderTransport;

/// Reader transport implementation backed by a native serial port.
pub struct SerialTransport {
    port: SerialStream,
}

impl SerialTransport {
    /// Open a serial-port-backed reader transport.
    ///
    /// `path` is the OS serial device path (for example `/dev/ttyUSB0` on
    /// Linux) and `baud_rate` is the reader UART speed.
    pub fn open(path: &str, baud_rate: u32) -> Result<Self, tokio_serial::Error> {
        let port = tokio_serial::new(path, baud_rate).open_native_async()?;
        Ok(Self { port })
    }

    /// Wrap an already-open serial stream object.
    pub fn from_stream(port: SerialStream) -> Self {
        Self { port }
    }

    /// Consume transport and return the wrapped serial stream.
    pub fn into_inner(self) -> SerialStream {
        self.port
    }
}

impl ReaderTransport for SerialTransport {
    type Error = std::io::Error;

    async fn write_all(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.port.write_all(data).await
    }

    async fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        self.port.read_exact(buf).await.map(|_| ())
    }
}
