#[cfg(feature = "serial")]
use rfid_silion_compat::{MemBank, MetadataFlags, SelectOption, SerialTransport, SilionReader};
#[cfg(feature = "serial")]
use std::env;

#[cfg(feature = "serial")]
fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02X}"));
    }
    out
}

#[cfg(feature = "serial")]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let port = args.next().unwrap_or_else(|| "/dev/ttyUSB0".to_string());
    let baud = args
        .next()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(115_200);
    let timeout_ms = args
        .next()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(5_000);

    println!(
        "Opening serial port {} at {} baud (timeout={} ms)...",
        port, baud, timeout_ms
    );

    let transport = SerialTransport::open(&port, baud)?;
    let mut reader = SilionReader::new(transport);

    let metadata = MetadataFlags::default()
        .with_rssi(true)
        .with_antenna_id(true)
        .with_timestamp(true)
        .with_protocol_id(true);

    println!("Running SingleTagInventory (0x21)...");
    let tag = reader
        .single_tag_inventory(timeout_ms, SelectOption::Disabled, Some(metadata))
        .await?;

    println!("Tag found:");
    println!("  EPC: {}", to_hex(&tag.epc_id));
    println!("  EPC bits: {}", tag.epc_bit_length.unwrap_or(0));
    println!("  PC: 0x{:04X}", tag.pc_word.unwrap_or(0));
    println!("  CRC: 0x{:04X}", tag.tag_crc);
    println!("  RSSI dBm: {:?}", tag.rssi_dbm);
    println!("  Antenna ID: {:?}", tag.antenna_id);
    println!("  Timestamp ms: {:?}", tag.timestamp_ms);
    println!("  Protocol ID: {:?}", tag.protocol_id);

    // Read out TID data as well
    let tid = reader
        .read_tag_data(
            5000,
            SelectOption::Epc {
                select_length_bits: (tag.epc_id.len() * 8) as u16,
                select_data: tag.epc_id.clone(),
                invert: false,
            },
            None,
            MemBank::Tid,
            0,
            6,
        )
        .await?;
    if let Some(tag_data) = tid.tag_data {
        println!("TID: {}", to_hex(&tag_data));
    } else {
        println!("TID read failed or returned no data.");
    }

    Ok(())
}

#[cfg(not(feature = "serial"))]
fn main() {
    eprintln!("Enable the 'serial' feature to run this example.");
    eprintln!(
        "Example: cargo run --features serial --example single_tag_inventory_serial -- /dev/ttyUSB0 115200 5000"
    );
}
