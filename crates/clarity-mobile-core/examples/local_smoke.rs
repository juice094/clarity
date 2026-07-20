//! Host-side diagnostic for the local DeepSeek device agent path in mobile-core.
//!
//! Run with:
//!
//! ```bash
//! DEEPSEEK_DEVICE_MOBILE=13626566112 \
//! DEEPSEEK_DEVICE_PASSWORD=zjx040507 \
//! cargo run -p clarity-mobile-core --example local_smoke
//! ```

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use clarity_mobile_core::{MobileConfig, MobileRuntime, ProviderProfile, ProviderType};

fn main() {
    let mobile = std::env::var("DEEPSEEK_DEVICE_MOBILE").expect("set DEEPSEEK_DEVICE_MOBILE");
    let password = std::env::var("DEEPSEEK_DEVICE_PASSWORD").expect("set DEEPSEEK_DEVICE_PASSWORD");
    let data_dir = std::env::var("DATA_DIR")
        .unwrap_or_else(|_| "target/tmp/clarity-mobile-core-local-smoke".into());

    let _ = std::fs::remove_dir_all(&data_dir);
    std::fs::create_dir_all(&data_dir).expect("create data dir");

    let profile = ProviderProfile {
        provider: ProviderType::DeepseekDevice,
        model: "deepseek-chat".into(),
        api_key: "".into(),
        base_url: None,
        mobile: Some(mobile),
        password: Some(password),
        search_enabled: false,
        reasoning_enabled: false,
    };
    let config = MobileConfig {
        data_dir,
        default_provider: profile,
        gateway_url: None,
        gateway_token: None,
    };

    let rt = Arc::new(MobileRuntime::new(config));
    let thread_id = rt.create_thread(None);
    println!("created thread_id={}", thread_id);

    let rt_poll = Arc::clone(&rt);
    let poll_handle = thread::spawn(move || {
        println!("event poller started");
        loop {
            match rt_poll.poll_event(1000) {
                Some(event) => println!("EVENT: {:?}", event),
                None => {}
            }
        }
    });

    println!("sending first message...");
    rt.send_message("Hi".into());
    thread::sleep(Duration::from_secs(45));

    println!("sending second message...");
    rt.send_message("What is 2+2?".into());
    thread::sleep(Duration::from_secs(45));

    println!("diagnostic complete");
    let _ = poll_handle.join();
}
