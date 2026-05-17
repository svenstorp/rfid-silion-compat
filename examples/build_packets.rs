use rfid_silion_compat::{MemBank, RegionCode, command::HostCommand, command::SelectContent};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let get_version = HostCommand::get_version()?;
    println!("GetVersion packet: {:02X?}", get_version);

    let set_region = HostCommand::set_current_region(RegionCode::NorthAmerica)?;
    println!("SetCurrentRegion(NA) packet: {:02X?}", set_region);

    let select = SelectContent {
        address_bits: 0x0000_0020,
        bit_len: 0x60,
        data: vec![
            0x01, 0x23, 0x45, 0x67, 0x89, 0xAB, 0xCD, 0xEF, 0x01, 0x23, 0x45, 0x67,
        ],
    };

    let read_tag = HostCommand::read_tag_data(
        1000,
        0x01,
        None,
        MemBank::User,
        0x0000_0002,
        4,
        Some(0),
        Some(select),
    )?;
    println!("ReadTagData packet: {:02X?}", read_tag);

    let stop_async = HostCommand::async_stop()?;
    println!("StopAsyncInventory packet: {:02X?}", stop_async);

    Ok(())
}
