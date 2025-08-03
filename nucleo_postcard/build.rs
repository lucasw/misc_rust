use std::fs::File;
use std::io::Write;
use std::net::Ipv4Addr;
use std::path::Path;
use std::{env, option_env};

fn main() {
    let out_dir = env::var("OUT_DIR").expect("No out dir");
    let dest_path = Path::new(&out_dir).join("constants.rs");
    let mut f = File::create(&dest_path).expect("Could not create file");

    let ip_string = {
        // set environmental variable to your IP address
        let ip_string = option_env!("REMOTE_IP");
        ip_string.unwrap_or("192.168.0.100")
    };

    let ipv4_addr: Ipv4Addr = ip_string.parse().expect("Invalid IPv4 address");

    // Get the 4-byte octet representation
    let octets: [u8; 4] = ipv4_addr.octets();
    println!(
        "cargo:warning=remote ip (should be the computer linked to the target device): {octets:?}"
    );

    write!(&mut f, "const REMOTE_IP: [u8; 4] = {octets:?};").expect("Could not write file");
    println!("cargo:rerun-if-env-changed=REMOTE_IP");
}
