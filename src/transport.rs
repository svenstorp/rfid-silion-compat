
/// Abstraction over byte-oriented transport used by the reader protocol.
pub trait ReaderTransport {
    /// Transport-specific error.
    type Error;

    /// Write all bytes to the transport.
    fn write_all(&mut self, data: &[u8]) -> Result<(), Self::Error>;

    /// Read exactly `buf.len()` bytes from the transport.
    ///
    /// Implementations should block/retry according to their timeout strategy
    /// until the exact byte count is produced or an error is returned.
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Self::Error>;
}

