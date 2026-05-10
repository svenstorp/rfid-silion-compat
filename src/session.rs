use std::sync::mpsc;
use std::thread;

use crate::client::{ClientError, ReaderClient};
use crate::codes::{CommandCode, StatusCode};
use crate::command::HostCommand;
use crate::host::{parse_async_frame_data, AsyncInventoryMessage, SilionHost};
use crate::transport::ReaderTransport;

/// An active asynchronous inventory session driven by a background reader thread.
///
/// Created by [`SilionHost::into_async_session`]. The transport is moved into
/// the background thread for the duration of the session; no other commands
/// can be sent until [`stop`][Self::stop] is called and the transport is
/// returned.
///
/// # Lifecycle
///
/// ```text
/// let mut host = SilionHost::new(transport);
/// // configure...
/// host.enable_async_inventory(&start_data)?;
///
/// let session = host.into_async_session();
///
/// // Drain messages on the current thread or hand session.message_rx to a
/// // dedicated receiver thread.
/// for result in &session.message_rx {
///     match result? {
///         AsyncInventoryMessage::TagInformation { tag, .. } => {
///             let _ = tag.epc_id;
///         }
///         AsyncInventoryMessage::StopAck => break,
///         _ => {}
///     }
/// }
///
/// let host = session.stop()?;  // recovered for future synchronous commands
/// ```
///
/// # Stop behaviour
///
/// When [`stop`][Self::stop] is called the background thread sends the
/// `0xAA49` stop command on the next opportunity (after the current
/// [`read_exact`][crate::ReaderTransport::read_exact] call returns).
/// Any frames that arrive between the stop command being written and the
/// `StopAck` being received are delivered through [`message_rx`] before the
/// channel closes.
///
/// If the session is **dropped without calling `stop`** the background thread
/// detects that the channel is disconnected and sends the stop command on its
/// own; the transport cannot be recovered in that case.
pub struct AsyncInventorySession<T>
where
    T: ReaderTransport + Send + 'static,
    T::Error: Send + 'static,
{
    /// Pushed asynchronous inventory messages from the reader.
    ///
    /// The channel closes after the `StopAck` frame is received or after an
    /// unrecoverable transport error.
    pub message_rx: mpsc::Receiver<Result<AsyncInventoryMessage, ClientError<T::Error>>>,
    stop_tx: mpsc::SyncSender<()>,
    transport_rx: mpsc::Receiver<T>,
    _thread: thread::JoinHandle<()>,
}

impl<T> AsyncInventorySession<T>
where
    T: ReaderTransport + Send + 'static,
    T::Error: Send + 'static,
{
    pub(crate) fn spawn(client: ReaderClient<T>) -> Self {
        // Capacity-1 so the bg thread can deposit the transport and exit
        // without blocking even when stop() is not called promptly.
        let (stop_tx, stop_rx) = mpsc::sync_channel::<()>(1);
        let (msg_tx, msg_rx) =
            mpsc::channel::<Result<AsyncInventoryMessage, ClientError<T::Error>>>();
        let (transport_tx, transport_rx) = mpsc::sync_channel::<T>(1);

        let thread = thread::spawn(move || {
            let mut client = client;
            'reader: loop {
                // Check for a pending stop signal (or a dropped session) before
                // blocking on the next read.
                match stop_rx.try_recv() {
                    Ok(()) => {
                        write_stop_then_drain(&mut client, &msg_tx);
                        break 'reader;
                    }
                    Err(mpsc::TryRecvError::Disconnected) => {
                        // Session was dropped without stop() — clean up anyway.
                        write_stop_then_drain(&mut client, &msg_tx);
                        break 'reader;
                    }
                    Err(mpsc::TryRecvError::Empty) => {}
                }

                let frame = match client.read_frame() {
                    Ok(f) => f,
                    Err(e) => {
                        let _ = msg_tx.send(Err(e));
                        break 'reader;
                    }
                };

                if frame.command != CommandCode::AsynchronousInventory as u8 {
                    // Non-async frame while in async mode — report and keep reading.
                    let _ = msg_tx.send(Err(ClientError::UnexpectedResponseCommand {
                        expected: CommandCode::AsynchronousInventory as u8,
                        actual: frame.command,
                    }));
                    continue 'reader;
                }

                if frame.status_raw != StatusCode::Success as u16 {
                    let _ = msg_tx.send(Err(ClientError::ReaderStatus {
                        status_raw: frame.status_raw,
                        status: frame.status,
                    }));
                    break 'reader;
                }

                match parse_async_frame_data(&frame.data) {
                    Err(e) => {
                        let _ = msg_tx.send(Err(ClientError::Protocol(e)));
                    }
                    Ok(msg) => {
                        let is_stop_ack = matches!(msg, AsyncInventoryMessage::StopAck);
                        let _ = msg_tx.send(Ok(msg));
                        if is_stop_ack {
                            break 'reader;
                        }
                    }
                }
            }

            // Always return the transport so stop() can recover it.
            let _ = transport_tx.send(client.into_inner());
        });

        Self {
            message_rx: msg_rx,
            stop_tx,
            transport_rx,
            _thread: thread,
        }
    }

    /// Signal the background thread to send the `0xAA49` stop command, wait
    /// for the session to end, and return a [`SilionHost`] wrapping the
    /// recovered transport.
    ///
    /// Any messages already queued in [`message_rx`][Self::message_rx] —
    /// including the final `StopAck` — can still be drained after this
    /// returns.
    ///
    /// Returns `Err(())` if the background thread panicked before it could
    /// return the transport.
    pub fn stop(self) -> Result<SilionHost<T>, ()> {
        // Ignore the send error: the bg thread may have already exited.
        let _ = self.stop_tx.send(());
        self.transport_rx
            .recv()
            .map(SilionHost::new)
            .map_err(|_| ())
    }
}

/// Write the async stop command and drain all remaining frames until `StopAck`.
fn write_stop_then_drain<T>(
    client: &mut ReaderClient<T>,
    msg_tx: &mpsc::Sender<Result<AsyncInventoryMessage, ClientError<T::Error>>>,
) where
    T: ReaderTransport,
{
    let stop_packet = match HostCommand::async_stop() {
        Ok(p) => p,
        Err(e) => {
            let _ = msg_tx.send(Err(ClientError::Protocol(e)));
            return;
        }
    };

    if let Err(e) = client.write_frame(&stop_packet) {
        let _ = msg_tx.send(Err(e));
        return;
    }

    // Keep reading until StopAck confirms the reader has stopped streaming.
    loop {
        let frame = match client.read_frame() {
            Ok(f) => f,
            Err(e) => {
                let _ = msg_tx.send(Err(e));
                return;
            }
        };
        match parse_async_frame_data(&frame.data) {
            Err(e) => {
                let _ = msg_tx.send(Err(ClientError::Protocol(e)));
            }
            Ok(msg) => {
                let is_stop_ack = matches!(msg, AsyncInventoryMessage::StopAck);
                let _ = msg_tx.send(Ok(msg));
                if is_stop_ack {
                    return;
                }
            }
        }
    }
}
