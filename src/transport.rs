/// Abstraction over asynchronous byte-oriented transport used by the reader protocol.
#[allow(async_fn_in_trait)]
pub trait ReaderTransport {
    /// Transport-specific error.
    type Error;

    /// Write all bytes to the transport.
    async fn write_all(&mut self, data: &[u8]) -> Result<(), Self::Error>;

    /// Read exactly `buf.len()` bytes from the transport.
    ///
    /// Implementations should await/retry according to their timeout strategy
    /// until the exact byte count is produced or an error is returned.
    async fn read_exact(&mut self, buf: &mut [u8]) -> Result<(), Self::Error>;
}
