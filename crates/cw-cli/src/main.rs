fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("doctor") => println!(
            "Corporate Warfare bootstrap: engine={}, interface={}; economy not implemented",
            cw_core::ENGINE_VERSION,
            cw_core::INTERFACE_VERSION
        ),
        _ => {
            eprintln!("Usage: cw-cli doctor");
            std::process::exit(2);
        }
    }
}
