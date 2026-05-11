#[cfg(feature = "serial")]
use rfid_silion_compat::serial::SerialTransport;
#[cfg(feature = "serial")]
use rfid_silion_compat::{InventoryOption, MetadataFlags, SilionReader};
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
        .single_tag_inventory(
            timeout_ms,
            InventoryOption::default().with_single_tag_metadata(true),
            metadata,
            None,
        )
        .await?;

    println!("Tag found:");
    println!("  EPC: {}", to_hex(&tag.epc_id));
    println!("  EPC bits: {}", tag.epc_bit_length);
    println!("  PC: 0x{:04X}", tag.pc_word);
    println!("  CRC: 0x{:04X}", tag.tag_crc);
    println!("  RSSI dBm: {:?}", tag.rssi_dbm);
    println!("  Antenna ID: {:?}", tag.antenna_id);
    println!("  Timestamp ms: {:?}", tag.timestamp_ms);
    println!("  Protocol ID: {:?}", tag.protocol_id);

    Ok(())
}

#[cfg(not(feature = "serial"))]
fn main() {
    eprintln!("Enable the 'serial' feature to run this example.");
    eprintln!(
        "Example: cargo run --features serial --example single_tag_inventory_serial -- /dev/ttyUSB0 115200 5000"
    );
}
