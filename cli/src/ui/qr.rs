//! QR code generation and display.

use qrcode::QrCode;

/// Print a QR code to the terminal.
///
/// Uses Unicode block characters for compact display where
/// each character represents 2 vertical modules.
pub fn print_qr_code(data: &str) {
    let code = match QrCode::new(data.as_bytes()) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to generate QR code: {}", e);
            return;
        }
    };

    let colors = code.to_colors();
    let width = code.width();

    // Unicode block characters:
    // ▀ = top black, bottom white
    // ▄ = top white, bottom black
    // █ = both black
    // (space) = both white

    let quiet = "  ";

    // Top quiet zone
    println!("{}{}", quiet, " ".repeat(width + 4));

    for y in (0..colors.len()).step_by(width * 2) {
        print!("{}  ", quiet);
        for x in 0..width {
            let top = colors.get(y + x).map(|c| *c == qrcode::Color::Dark).unwrap_or(false);
            let bottom = colors.get(y + width + x).map(|c| *c == qrcode::Color::Dark).unwrap_or(false);

            let ch = match (top, bottom) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            };
            print!("{}", ch);
        }
        println!("  ");
    }

    // Bottom quiet zone
    println!("{}{}", quiet, " ".repeat(width + 4));
}
