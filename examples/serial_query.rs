#[cfg(feature = "serialport")]
use rfidlibrs::serial::SerialPortTransport;
#[cfg(feature = "serialport")]
use rfidlibrs::{
    AsyncInventoryMessage, AsyncInventoryStartData, EmbeddedReadTagData,
    InventoryEmbeddedCommandContent, InventoryOption, InventorySearchFlags, MetadataFlags,
    MemBank, SilionHost,
};
#[cfg(feature = "serialport")]
use std::env;
#[cfg(feature = "serialport")]
use std::time::Duration;

#[cfg(feature = "serialport")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let port = args.next().unwrap_or_else(|| "/dev/ttyUSB0".to_string());
    let baud = args
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(115_200);

    let transport = SerialPortTransport::open(&port, baud, Duration::from_millis(500))?;
    let mut host = SilionHost::new(transport);

    let version = host.get_version()?;
    println!(
        "Firmware version: {:02X?}, date: {:02X?}",
        version.firmware_version, version.firmware_date
    );
    println!("Bootloader version: {:02X?}", version.bootloader_version);
    println!("Hardware Version: {:02X?}", version.hardware_version);

    let serial_number = host.get_serial_number(0x00, 0x00)?;
    println!(
        "Serial number year: {:02X?}, bytes: {:02X?}",
        serial_number.year, serial_number.serial_number
    );

    let phase = host.get_run_phase()?;
    println!("Run phase: {phase:?}");

    if phase == rfidlibrs::RunPhase::Bootloader {
        host.boot_firmware()?;
    }

    let region = host.get_current_region()?;
    println!("Current region: {region}");

    if region != rfidlibrs::RegionCode::Europe {
        host.set_current_region(rfidlibrs::RegionCode::Europe)?;
    }

    // --- Async inventory: heartbeats (pings) enabled, 2-minute window ---

    let search_flags = InventorySearchFlags::new()
        .with_async_heartbeat(true)
        .with_async_auto_stop(false)
        .with_embedded_command(true)
        .with_async_rest_ratio_steps(4)?;

    let start_data = AsyncInventoryStartData {
        metadata_flags: MetadataFlags::default()
            .with_read_count(true)
            .with_rssi(true)
            .with_antenna_id(true)
            .with_frequency(true)
            .with_timestamp(true)
            .with_data_length(true)
            .with_protocol_id(true),
        option: InventoryOption::default(),
        search_flags,
        access_password: None,
        select_content: None,
        embedded_command_content: Some(InventoryEmbeddedCommandContent::ReadTagData(
            EmbeddedReadTagData {
                read_membank: MemBank::Tid,
                read_address_words: 0,
                word_count: 6,
            },
        )),
    };

    println!("Starting async inventory (2 minutes, heartbeats enabled)…");
    // Switch to a long read timeout before going async: the reader only pushes
    // frames when it has data (tags or heartbeats), so the normal 500 ms
    // command timeout would cause spurious TimedOut errors between frames.
    host.transport_mut().set_timeout(Duration::from_secs(30))?;
    host.enable_async_inventory(&start_data)?;

    let session = host.into_async_session();

    let deadline = std::time::Instant::now() + Duration::from_secs(120);

    for result in &session.message_rx {
        match result? {
            AsyncInventoryMessage::TagInformation { metadata_flags, tag } => {
                println!(
                    "  Tag\n    metadata_flags: 0x{flags:04X}\n    read_count: {read_count:?}\n    rssi_dbm: {rssi:?}\n    antenna_id: {antenna_id:?}\n    frequency_khz: {frequency:?}\n    timestamp_ms: {timestamp:?}\n    rfu: {rfu:?}\n    protocol_id: {protocol_id:?}\n    tag_data_bit_length: {tag_data_bits:?}\n    tag_data: {tag_data:02X?}\n    epc_bit_length: {epc_bits}\n    pc_word: 0x{pc:04X}\n    epc_id: {epc:02X?}\n    tag_crc: 0x{crc:04X}",
                    flags = metadata_flags.raw(),
                    read_count = tag.read_count,
                    rssi = tag.rssi_dbm,
                    antenna_id = tag.antenna_id,
                    frequency = tag.frequency_khz,
                    timestamp = tag.timestamp_ms,
                    rfu = tag.rfu,
                    protocol_id = tag.protocol_id,
                    tag_data_bits = tag.tag_data_bit_length,
                    tag_data = tag.tag_data,
                    epc_bits = tag.epc_bit_length,
                    pc = tag.pc_word,
                    epc = tag.epc_id,
                    crc = tag.tag_crc,
                );
            }
            AsyncInventoryMessage::Heartbeat {
                search_flags,
                state_data,
            } => {
                println!(
                    "  Heartbeat  flags=0x{:04X}  state={state_data:02X?}",
                    search_flags.raw()
                );
            }
            AsyncInventoryMessage::StartAck => println!("  Async inventory started."),
            AsyncInventoryMessage::StopAck => {
                println!("  Async inventory stopped.");
                break;
            }
            AsyncInventoryMessage::Subcommand {
                subcommand,
                subcommand_data,
            } => {
                println!("  Subcommand 0x{subcommand:04X}: {subcommand_data:02X?}");
            }
        }

        if std::time::Instant::now() >= deadline {
            println!("2-minute window elapsed, stopping…");
            break;
        }
    }

    let mut host = session.stop().map_err(|()| "background thread panicked")?;

    // Drain any messages still queued after stop (including the StopAck if
    // the loop broke before seeing it), so the log is complete.

    println!("Resetting module to bootloader mode…");
    host.boot_bootloader()?;

    Ok(())
}

#[cfg(not(feature = "serialport"))]
fn main() {
    eprintln!("Enable the 'serialport' feature to run this example.");
    eprintln!(
        "Example: cargo run --features serialport --example serial_query -- /dev/ttyUSB0 115200"
    );
}
