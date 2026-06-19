//! Build script for `clarity-mobile-core`.
//!
//! Generates UniFFI scaffolding from the UDL file at compile time.

fn main() {
    if let Err(e) = uniffi::generate_scaffolding("./src/clarity_mobile_core.udl") {
        eprintln!("Failed to generate UniFFI scaffolding: {e}");
        std::process::exit(1);
    }
}
