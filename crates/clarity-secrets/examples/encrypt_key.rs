#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    unsafe_code
)]
use clarity_secrets::SecretStore;
use std::io::Read;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: encrypt_key <key-file>");
        eprintln!("Plaintext is read from stdin.");
        std::process::exit(1);
    }
    let key_path = &args[1];

    let mut plaintext = String::new();
    std::io::stdin()
        .read_to_string(&mut plaintext)
        .expect("failed to read plaintext from stdin");
    let plaintext = plaintext.trim();

    let store = SecretStore::load_or_create(key_path).expect("failed to load/create secret store");
    let encrypted = store.encrypt(plaintext).expect("failed to encrypt");
    println!("{}", encrypted);
}
