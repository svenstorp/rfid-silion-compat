use crate::client::ReaderClient;
use crate::codes::CommandCode;
use crate::command::HostCommand;
use crate::host::{parse_async_frame_data, AsyncInventoryMessage, SilionHost};
use crate::transport::ReaderTransport;
use crate::ClientError;

/// An active asynchronous inventory session driven by awaited reads.
///
/// Created by [`SilionHost::into_async_session`]. The transport is moved into
/// this session for the duration of asynchronous inventory, and no other
/// commands can be sent until [`stop`][Self::stop] is called and the transport
/// is recovered as a [`SilionHost`].
pub struct AsyncInventorySession<T: ReaderTransport> {
    client: ReaderClient<T>,
}

impl<T: ReaderTransport> AsyncInventorySession<T> {
    pub(crate) fn new(client: ReaderClient<T>) -> Self {
        Self { client }
    }

    /// Receive one pushed asynchronous inventory message from the reader.
    pub async fn recv(&mut self) -> Result<AsyncInventoryMessage, ClientError<T::Error>> {
        let frame = self.client.read_frame().await?;
        if frame.command != CommandCode::AsynchronousInventory as u8 {
            return Err(ClientError::UnexpectedResponseCommand {
                expected: CommandCode::AsynchronousInventory as u8,
                actual: frame.command,
            });
        }
        if frame.status_raw != 0x0000 {
            return Err(ClientError::ReaderStatus {
                status_raw: frame.status_raw,
                status: frame.status,
            });
        }
        parse_async_frame_data(&frame.data).map_err(ClientError::Protocol)
    }

    /// Send `0xAA49`, wait for `StopAck`, and recover the host.
    pub async fn stop(mut self) -> Result<SilionHost<T>, ClientError<T::Error>> {
        let stop_packet = HostCommand::async_stop().map_err(ClientError::Protocol)?;
        self.client.write_frame(&stop_packet).await?;

        loop {
            let message = self.recv().await?;
            if matches!(message, AsyncInventoryMessage::StopAck) {
                break;
            }
        }

        Ok(SilionHost::from_client(self.client))
    }
}

#[cfg(test)]
mod tests {
    use super::AsyncInventorySession;
    use crate::client::ReaderClient;
    use crate::command::AsyncSubcommandCode;
    use crate::codes::CommandCode;
    use crate::test_support::{reply_frame, MockInteraction, MockTransport};
    use crate::{subcommand_crc, AsyncInventoryMessage, InventorySearchFlags, RegionCode};

    #[test]
    fn recv_heartbeat_message() {
        let mut data = b"XTSJ".to_vec();
        data.extend_from_slice(&0x8000u16.to_be_bytes());
        data.push(0x01);
        let packet = reply_frame(CommandCode::AsynchronousInventory as u8, 0x0000, &data);
        let transport = MockTransport::from_replies(vec![packet]);
        let client = ReaderClient::new(transport);
        let mut session = AsyncInventorySession::new(client);

        let message = futures::executor::block_on(session.recv()).expect("message should parse");
        match message {
            AsyncInventoryMessage::Heartbeat {
                search_flags,
                state_data,
            } => {
                assert_eq!(search_flags, InventorySearchFlags::from_raw(0x8000));
                assert_eq!(state_data, vec![0x01]);
            }
            other => panic!("unexpected async message: {other:?}"),
        }
    }

    #[test]
    fn stop_recovers_host() {
        let mut stop_ack = b"Moduletech".to_vec();
        stop_ack.extend_from_slice(&(AsyncSubcommandCode::Stop as u16).to_be_bytes());
        stop_ack.push(subcommand_crc(AsyncSubcommandCode::Stop as u16, &[]));
        stop_ack.push(0xBB);

        let transport = MockTransport::scripted(vec![
            MockInteraction {
                request_command: CommandCode::AsynchronousInventory as u8,
                response_status: 0x0000,
                response_data: stop_ack,
            },
            MockInteraction {
                request_command: CommandCode::GetCurrentRegion as u8,
                response_status: 0x0000,
                response_data: vec![0x01],
            },
        ]);
        let client = ReaderClient::new(transport);
        let session = AsyncInventorySession::new(client);

        let mut host = futures::executor::block_on(session.stop()).expect("stop should recover host");
        let region = futures::executor::block_on(host.get_current_region())
            .expect("recovered host should work");
        assert_eq!(region, RegionCode::NorthAmerica);
    }
}
