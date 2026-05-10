use crate::transport::ReaderTransport;
use serialport::SerialPort;
use std::io::{Read, Write};
use std::time::Duration;

/// Reader transport implementation backed by a serial port.
pub struct SerialPortTransport {
    port: Box<dyn SerialPort>,
}

impl SerialPortTransport {
    /// Open a serial-port-backed reader transport.
    ///
    /// `path` is the OS serial device path (for example `/dev/ttyUSB0` on
    /// Linux), `baud_rate` is reader UART speed, and `timeout` controls I/O
    /// read behavior.
    pub fn open(path: &str, baud_rate: u32, timeout: Duration) -> Result<Self, serialport::Error> {
        let port = serialport::new(path, baud_rate).timeout(timeout).open()?;
        Ok(Self { port })
    }

    /// Wrap an already-open serial port object.
    pub fn from_port(port: Box<dyn SerialPort>) -> Self {
        Self { port }
    }

    /// Consume transport and return the wrapped serial port.
    pub fn into_inner(self) -> Box<dyn SerialPort> {
        self.port
    }

    /// Change the read timeout on the underlying serial port.
    ///
    /// Call this with a long timeout (for example 30 s) before entering
    /// asynchronous inventory mode, where frames may arrive infrequently.
    pub fn set_timeout(&mut self, timeout: Duration) -> Result<(), serialport::Error> {
        self.port.set_timeout(timeout)
    }
}

impl ReaderTransport for SerialPortTransport {
    type Error = std::io::Error;

    fn write_all(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        self.port.write_all(data)
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        self.port.read_exact(buf)
    }
}
