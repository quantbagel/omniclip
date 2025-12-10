//! Info command implementation.

use omniclip_core::OmniclipService;

/// Display device information.
pub fn show_info(device_name: String) {
    let service = OmniclipService::new(device_name);

    println!("\n\x1b[1mOmniclip Device Info\x1b[0m");
    println!("═══════════════════════════════════════");
    println!("\x1b[1mName:\x1b[0m        {}", service.device_name());
    println!("\x1b[1mID:\x1b[0m          {}", service.device_id());
    println!("\x1b[1mFingerprint:\x1b[0m {}", service.fingerprint());

    println!("\n\x1b[1mLocal IPs:\x1b[0m");
    for ip in omniclip_core::discovery::get_local_ips() {
        println!("  • {}", ip);
    }
    println!();
}
