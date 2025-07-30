/*!
Send packets to a receiver

receive on command line:

```
netcat -ul 34201
```

or in hex (but have to wait for a full line before seeing output)

```
netcat -ul 34201 | hexdump -C
```

*/

use postcard::{from_bytes_crc32, to_stdvec_crc32};
use serde::{Deserialize, Serialize};
use std::net::UdpSocket;

#[derive(Serialize, Deserialize, Debug, Default)]
struct SomeData {
    counter: u64,
    value0: f64,
    value1: u32,
    value2: u8,
}

fn main() -> std::io::Result<()> {
    let addr0 = "127.0.0.1:34200";
    let socket = UdpSocket::bind(addr0)?;
    let addr1 = "127.0.0.1:34201";
    println!("{socket:?}");

    let crc = crc::Crc::<u32>::new(&crc::CRC_32_ISCSI);
    // let mut msg_bytes = [0x33, 0xBE, 0x0, 0x4, 0x6, 0x9];
    let mut data = SomeData::default();

    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
        let msg_bytes = {
            match to_stdvec_crc32(&data, crc.digest()) {
                Ok(msg_bytes) => msg_bytes,
                Err(err) => {
                    eprintln!("{err:?}");
                    continue;
                }
            }
        };
        let rv = socket.send_to(&msg_bytes, &addr1);
        println!("sent {data:?} encoded as {msg_bytes:X?}, rv {rv:?}");
        // msg_bytes[2] += 1;
        data.counter += 1;
    }

    // Ok(())
}
