/*!
Receive packets from a sender

transmit on command line:

```
echo "my test message is much too long" | nc -u localhost 34201
```

or send with the node0 binary

*/

use clap::{Command, arg};
use net_common::Message;
use std::net::UdpSocket;

fn main() -> std::io::Result<()> {
    let matches = Command::new("message_rx")
        .args(&[
            arg!(
                -l --local_ip <LOCAL_IP> "ip of local computer running this program"
            )
            .default_value("127.0.0.1"),
            /*
            arg!(
                -r --remote_ip <REMOTE_IP> "ip of remote device"
            )
            .default_value("192.168.0.123"),
            */
        ])
        .get_matches();
    let local_ip = matches.get_one::<String>("local_ip").unwrap();
    let local_ip_port = format!("{local_ip}:34200");
    println!("local ip and port {local_ip_port:?}");
    let socket = UdpSocket::bind(local_ip_port)?;
    println!("{socket:?}");

    let crc = crc::Crc::<u32>::new(&crc::CRC_32_ISCSI);

    let mut buf = [0; 256];
    loop {
        match socket.recv_from(&mut buf) {
            Ok((rx_num, src)) => {
                let rx_stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("time went backwards");

                if rx_num >= buf.len() {
                    eprintln!(
                        "more bytes may have been sent but only received {rx_num} == {}",
                        buf.len()
                    );
                }
                let msg: Message = {
                    match Message::decode(&buf, crc.digest()) {
                        Ok(rx_data) => rx_data,
                        Err(err) => {
                            eprintln!("{err:?}");
                            continue;
                        }
                    }
                };
                if let Message::TimeStamp(timestamp) = msg {
                    println!(
                        "[{rx_stamp:.3?}], TimeStamp offset {:.3}s, roundtrip {}us",
                        timestamp.ntp_offset as f64 / 1e6,
                        timestamp.ntp_roundtrip,
                    );
                } else {
                    println!(
                        "[{rx_stamp:?}] {msg:?} decoded from {rx_num:?} bytes from {src:?}: {:X?}",
                        &buf[..rx_num]
                    );
                }
            }
            Err(err) => {
                eprintln!("{err:?}");
            }
        }
        // std::thread::sleep(std::time::Duration::from_secs(1));
    }

    // Ok(())
}
