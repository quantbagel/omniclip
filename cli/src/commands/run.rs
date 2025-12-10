//! Run command implementation.

use omniclip_core::{ClipboardContent, OmniclipService, ServiceEvent};

use crate::process::kill_previous_instances;
use crate::ui::{print_banner, print_qr_code};

/// Run the omniclip service.
pub async fn run_service(device_name: String) -> anyhow::Result<()> {
    kill_previous_instances();
    print_banner();

    let mut service = OmniclipService::new(device_name);

    println!("\x1b[1mDevice:\x1b[0m {}", service.device_name());
    println!("\x1b[1mID:\x1b[0m     {}", service.device_id());
    println!("\x1b[1mKey:\x1b[0m    {}", service.fingerprint());

    // Start pairing session and show QR
    let pairing_url = service.start_pairing().await?;

    println!("\n\x1b[1;33mScan this QR code with the Omniclip iOS app to pair:\x1b[0m\n");
    print_qr_code(&pairing_url);
    println!("\n\x1b[2mOr enter manually: {}\x1b[0m\n", pairing_url);

    // Start the service
    let mut events = service.start().await?;

    println!("\x1b[1;32m✓\x1b[0m Listening for devices and clipboard changes...");
    println!("\x1b[2mPress Ctrl+C to stop.\x1b[0m\n");

    // Handle Ctrl+C gracefully
    let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(1);
    ctrlc::set_handler(move || {
        let _ = tx.blocking_send(());
    })?;

    loop {
        tokio::select! {
            Some(event) = events.recv() => {
                handle_event(event);
            }
            _ = rx.recv() => {
                println!("\n\x1b[1;33mShutting down...\x1b[0m");
                break;
            }
        }
    }

    Ok(())
}

/// Handle a service event and print appropriate output.
fn handle_event(event: ServiceEvent) {
    match event {
        ServiceEvent::DeviceDiscovered(peer) => {
            println!("\x1b[1;32m⬤\x1b[0m Found: \x1b[1m{}\x1b[0m", peer.device_name);
            for addr in &peer.addresses {
                println!("    {}:{}", addr, peer.port);
            }
        }
        ServiceEvent::DeviceLost(id) => {
            println!("\x1b[1;31m⬤\x1b[0m Lost: {}", id);
        }
        ServiceEvent::PairingRequest { device_id, device_name } => {
            println!(
                "\x1b[1;35m⚡\x1b[0m Pairing request from: \x1b[1m{}\x1b[0m ({})",
                device_name, device_id
            );
        }
        ServiceEvent::ClipboardReceived { from_device, content } => {
            let preview = format_preview(&content);
            println!("\x1b[1;34m📋\x1b[0m Received from {}: \"{}\"", from_device, preview);
        }
        ServiceEvent::ClipboardSent { to_devices } => {
            println!("\x1b[1;34m📤\x1b[0m Sent to {} device(s)", to_devices.len());
        }
        ServiceEvent::Error(e) => {
            eprintln!("\x1b[1;31m✗\x1b[0m Error: {}", e);
        }
    }
}

/// Format clipboard content for preview display.
fn format_preview(content: &ClipboardContent) -> String {
    const MAX_PREVIEW_LEN: usize = 50;

    let text = match content {
        ClipboardContent::Text(t) => t,
        ClipboardContent::RichText { plain, .. } => plain,
    };

    if text.len() > MAX_PREVIEW_LEN {
        format!("{}...", &text[..MAX_PREVIEW_LEN])
    } else {
        text.clone()
    }
}
