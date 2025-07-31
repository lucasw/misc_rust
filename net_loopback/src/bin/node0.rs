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

use net_loopback::{Message, SmallArray, SomeData};
use std::net::UdpSocket;

fn main() -> std::io::Result<()> {
    let addr0 = "127.0.0.1:34200";
    let socket = UdpSocket::bind(addr0)?;
    let addr1 = "127.0.0.1:34201";
    println!("{socket:?}");

    let crc = crc::Crc::<u32>::new(&crc::CRC_32_ISCSI);

    // It's a little bit of a pain to modify the contents of an enum, but it avoids cloning later
    let mut data = Message::Data(SomeData {
        value0: 2.52457892,
        ..Default::default()
    });

    let mut array = SmallArray::default();
    array.data[6] = 0x23;
    let mut array = Message::Array(array);

    loop {
        // alternate betwee message types
        std::thread::sleep(std::time::Duration::from_secs(1));
        let msg_bytes = {
            match Message::encode(&data, &crc) {
                Ok(msg_bytes) => msg_bytes,
                Err(err) => {
                    eprintln!("{err:?}");
                    continue;
                }
            }
        };
        let rv = socket.send_to(&msg_bytes, addr1);
        println!("sent {data:?} encoded as {msg_bytes:X?}, rv {rv:?}");
        // msg_bytes[2] += 1;
        if let Message::Data(ref mut data) = data {
            data.counter += 1;
            data.value0 += 0.1;
        }

        std::thread::sleep(std::time::Duration::from_secs(1));
        let msg_bytes = {
            match Message::encode(&array, &crc) {
                Ok(msg_bytes) => msg_bytes,
                Err(err) => {
                    eprintln!("{err:?}");
                    continue;
                }
            }
        };
        let rv = socket.send_to(&msg_bytes, addr1);
        println!("sent {array:?} encoded as {msg_bytes:X?}, rv {rv:?}");
        if let Message::Array(ref mut array) = array {
            array.data[6] *= 2;
            if array.data[6] > 2 {
                array.data[6] -= 1;
            }
        }

        std::thread::sleep(std::time::Duration::from_secs(1));
        let garbage = [0x12, 0x34, 0x45, 0x67, 0x89];
        let rv = socket.send_to(&garbage, addr1);
        println!("sent garbage {garbage:X?}, rv {rv:?}");

        // garbage with valid header
        let garbage = [
            Message::DATA[0],
            Message::DATA[1],
            Message::DATA[2],
            Message::DATA[3],
            0x89,
        ];
        let rv = socket.send_to(&garbage, addr1);
        println!("sent garbage {garbage:X?} with valid header, rv {rv:?}");

        // TODO(lucasw) generate garbage with valid header and valid crc32
    }

    // Ok(())
}
