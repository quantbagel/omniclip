//! Banner and header printing.

/// Print the application banner.
pub fn print_banner() {
    println!("\n\x1b[1;36m╔══════════════════════════════════════╗\x1b[0m");
    println!("\x1b[1;36m║\x1b[0m         \x1b[1mOmniclip\x1b[0m                     \x1b[1;36m║\x1b[0m");
    println!("\x1b[1;36m║\x1b[0m    Cross-platform clipboard sync     \x1b[1;36m║\x1b[0m");
    println!("\x1b[1;36m╚══════════════════════════════════════╝\x1b[0m\n");
}
