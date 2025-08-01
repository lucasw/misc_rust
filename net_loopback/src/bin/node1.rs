/*!
Receive packets from a sender

transmit on command line:

```
echo "my test message is much too long" | nc -u localhost 34201
```

or send with the node0 binary

*/

use net_common::Message;
use std::net::UdpSocket;

fn main() -> std::io::Result<()> {
    // let addr0 = "127.0.0.1:34200";
    let addr1 = "127.0.0.1:34201";
    let socket = UdpSocket::bind(addr1)?;
    println!("{socket:?}");

    let crc = crc::Crc::<u32>::new(&crc::CRC_32_ISCSI);

    let mut buf = [0; 256];
    loop {
        match socket.recv_from(&mut buf) {
            Ok((rx_num, src)) => {
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
                println!(
                    "{msg:?} decoded from {rx_num:?} bytes from {src:?}: {:X?}",
                    &buf[..rx_num]
                );
            }
            Err(err) => {
                eprintln!("{err:?}");
            }
        }
        // std::thread::sleep(std::time::Duration::from_secs(1));
    }

    // Ok(())
}
