#[cfg(feature = "serial")]
use rfid_silion_compat::{
    AntennaPortsConfiguration, AntennaPower, EpcValue, Giai96, MetadataFlags, SelectOption,
    SerialTransport, SilionReader,
};
#[cfg(feature = "serial")]
use std::env;

#[cfg(feature = "serial")]
const LOCAL_COMPANY_PREFIX: u64 = 999_999;
#[cfg(feature = "serial")]
const GIAI_PARTITION_FOR_6_DIGITS: u8 = 6;
#[cfg(feature = "serial")]
const MAX_BIB: u64 = 999_999;
#[cfg(feature = "serial")]
const TX_ANTENNA: u8 = 1;
#[cfg(feature = "serial")]
const TARGET_GAIN_DBM: u16 = 200; // 2.00 dBm (protocol unit is 0.01 dBm)

#[cfg(feature = "serial")]
fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02X}"));
    }
    out
}

#[cfg(feature = "serial")]
fn compose_asset_reference(race_id: u64, bib_number: u64) -> Result<u64, Box<dyn std::error::Error>> {
    if bib_number > MAX_BIB {
        return Err(format!("bib number must be <= {MAX_BIB}").into());
    }

    // Pack as: race_id * 1_000_000 + bib_number.
    // This keeps race_id and bib_number human-readable in decimal form.
    race_id
        .checked_mul(1_000_000)
        .and_then(|v| v.checked_add(bib_number))
        .ok_or_else(|| "race_id/bib combination overflows asset reference".into())
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
    let race_id = args
        .next()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(20260001);
    let bib_number = args
        .next()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(1001);

    println!(
        "Opening serial port {} at {} baud (timeout={} ms)...",
        port, baud, timeout_ms
    );

    let transport = SerialTransport::open(&port, baud)?;
    let mut reader = SilionReader::new(transport);

    println!(
        "Setting antenna {} read/write power to 2 dB (2.00 dBm)...",
        TX_ANTENNA
    );
    reader
        .set_antenna_ports(&AntennaPortsConfiguration::Power(vec![AntennaPower {
            tx: TX_ANTENNA,
            read_power: TARGET_GAIN_DBM,
            write_power: TARGET_GAIN_DBM,
        }]))
        .await?;

    println!("Running SingleTagInventory (0x21) to get one tag...");
    let metadata = MetadataFlags::default()
        .with_rssi(true)
        .with_antenna_id(true)
        .with_timestamp(true);
    let tag = reader
        .single_tag_inventory(timeout_ms, SelectOption::Disabled, Some(metadata))
        .await?;

    println!("Current tag EPC: {}", to_hex(&tag.epc_id));

    let asset_reference = compose_asset_reference(race_id, bib_number)?;

    // We use a local/private 6-digit company prefix when no global GS1 prefix exists.
    let giai = Giai96 {
        filter: 1, // Application-specific class marker (example: race bib)
        partition: GIAI_PARTITION_FOR_6_DIGITS,
        company_prefix: LOCAL_COMPANY_PREFIX,
        individual_asset_reference: asset_reference,
    };
    let new_epc = EpcValue::from_schema(giai)?;
    let new_epc_bytes = new_epc.as_bytes().to_vec();

    println!(
        "Writing new EPC as GIAI-96: company_prefix={}, race_id={}, bib_number={}",
        LOCAL_COMPANY_PREFIX, race_id, bib_number
    );
    println!("New EPC bytes: {}", to_hex(&new_epc_bytes));

    let select_current_tag = SelectOption::Epc {
        select_length_bits: (tag.epc_id.len() * 8) as u16,
        select_data: tag.epc_id.clone(),
        invert: false,
    };

    reader
        .write_tag_epc(
            timeout_ms,
            select_current_tag,
            None,
            &new_epc_bytes,
        )
        .await?;

    println!("WriteTagEpc (0x23) succeeded.");

    let updated = reader
        .single_tag_inventory(timeout_ms, SelectOption::Disabled, None)
        .await?;
    println!("Updated tag EPC: {}", to_hex(&updated.epc_id));

    Ok(())
}

#[cfg(not(feature = "serial"))]
fn main() {
    eprintln!("Enable the 'serial' feature to run this example.");
    eprintln!(
        "Example: cargo run --features serial --example write_race_bib_epc_serial -- /dev/ttyUSB0 115200 5000 20260001 1001"
    );
}
