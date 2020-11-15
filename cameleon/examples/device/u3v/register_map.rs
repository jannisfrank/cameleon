extern crate cameleon;
extern crate cameleon_device;

use cameleon::device::u3v::*;

fn main() {
    // Build emulator in case libusb is not supported.
    #[cfg(not(feature = "libusb"))]
    cameleon_device::u3v::EmulatorBuilder::new()
        .user_defined_name("cameleon-emulator")
        .unwrap()
        .build();

    // Enumerate devices.
    let mut devices = enumerate_devices().unwrap();
    if devices.is_empty() {
        println!("no device found");
        return;
    }

    // Open the first device.
    let mut device = devices.pop().unwrap();
    device.open().unwrap();

    println!("\n### Technology Agnostic Boot Register Map ###\n");
    let abrm = device.abrm().unwrap();

    println!("gencp_version: {}", abrm.gencp_version());
    println!("manufacturer_name: {}", abrm.manufacturer_name());
    println!("model_name: {}", abrm.model_name());
    println!("family_name: {:?}", abrm.family_name());
    println!("device_version: {}", abrm.device_version());
    println!("manufacturer_name: {}", abrm.manufacturer_name());
    println!("serial_number: {}", abrm.serial_number());
    println!("manifest_table_address: {}", abrm.manifest_table_address());
    println!("sbrm_address: {}", abrm.sbrm_address());
    println!(
        "device_software_interface_version: {:?}",
        abrm.device_software_interface_version()
    );
    println!(
        "maximum_device_response_time: {:?}",
        abrm.maximum_device_response_time()
    );
    println!(
        "is_user_defined_name_supported: {}",
        abrm.is_user_defined_name_supported()
    );
    println!("user_defined_name: {:?}", abrm.user_defined_name().unwrap());
    println!(
        "is_multi_event_supported: {}",
        abrm.is_multi_event_supported()
    );
    println!(
        "is_multi_event_enabled: {}",
        abrm.is_multi_event_enabled().unwrap()
    );
    println!(
        "is_stacked_commands_supported: {}",
        abrm.is_stacked_commands_supported()
    );

    // Write to writable registers.
    // NOTE. These oeprations will cause non-volatile changes to the register.
    //
    // abrm.set_user_defined_name("Cameleon").unwrap();
    // println!(
    //     "changed user_defined_name: {:?}",
    //     abrm.user_defined_name().unwrap()
    // );

    // if abrm.is_multi_event_supported() {
    //     abrm.enable_multi_event().unwrap();
    // }
    //
    println!("\n### Technology Specifig Boot Register Map ###\n");

    let sbrm = device.sbrm().unwrap();
    println!("u3v_version: {}", sbrm.u3v_version());
    println!(
        "maximum_command_transfer_length: {}",
        sbrm.maximum_command_transfer_length()
    );
    println!(
        "maximum_acknowledge_transfer_length: {}",
        sbrm.maximum_acknowledge_trasfer_length()
    );
    println!(
        "number_of_stream_channel: {}",
        sbrm.number_of_stream_channel()
    );
    println!("sirm_address: {:?}", sbrm.sirm_address());
    println!("sirm_length: {:?}", sbrm.sirm_length());

    println!("eirm_address: {:?}", sbrm.eirm_address());
    println!("eirm_length: {:?}", sbrm.eirm_length());
    println!("iidc2_address: {:?}", sbrm.iidc2_address());
    println!("current_speed: {:?}", sbrm.current_speed());
}
