/*!
Send timestamp message to a receiver which will fill it's own timestamp field and return
the message as quickly as possible, then measure the elapsed time here

receive on command line:

```
netcat -ul 34201
```

or in hex (but have to wait for a full line before seeing output)

```
netcat -ul 34201 | hexdump -C
```

*/

use net_common::{Message, SmallArray, TimeStamp};
use std::net::UdpSocket;
use std::time::Duration;

fn main() -> std::io::Result<()> {
    // TODO(lucasw) get this ip address from command line argument
    let addr0 = "192.168.0.100:34200";
    let socket = UdpSocket::bind(addr0)?;
    // 1 second timeout
    let recv_timeout = Duration::new(1, 0);
    let result = socket.set_read_timeout(Some(recv_timeout));
    println!("set socket recv timeout to {recv_timeout:?}, {result:?}");

    let addr1 = "192.168.0.123:34201";
    println!("this socket is {socket:?}");
    println!("sending to {addr1:?}");

    let crc = crc::Crc::<u32>::new(&crc::CRC_32_ISCSI);

    // It's a little bit of a pain to modify the contents of an enum, but it avoids cloning later
    let mut tx_timestamp = Message::Data(TimeStamp::default());

    /*
    let mut array = SmallArray::default();
    array.data[6] = 0x23;
    // let mut array = Message::Array(array);
    */

    let delay_secs = 1;

    loop {
        std::thread::sleep(std::time::Duration::from_secs(delay_secs));
        let tx_stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time went backwards");
        if let Message::Data(ref mut data) = tx_timestamp {
            data.counter += 1;
            data.seconds = tx_stamp.as_secs();
            data.nanoseconds = tx_stamp.subsec_nanos();
        }

        let msg_bytes = {
            match net_loopback::encode(&tx_timestamp, crc.digest()) {
                Ok(msg_bytes) => msg_bytes,
                Err(err) => {
                    eprintln!("{err:?}");
                    continue;
                }
            }
        };
        let tx_rv = socket.send_to(&msg_bytes, addr1);
        // println!("sent {data:?} encoded as {msg_bytes:X?}, rv {rv:?}");

        let mut rx_buffer = [0; 256];
        match socket.recv(&mut rx_buffer) {
            Ok(num_bytes) => {
                if num_bytes > 0 {
                    let rx_stamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .expect("time went backwards");
                    match Message::decode(&rx_buffer[..num_bytes], crc.digest()) {
                        Ok(Message::Data(data)) => {
                            let elapsed = (rx_stamp - tx_stamp).as_secs_f64();
                            println!(
                                "[{rx_stamp:?}], elapsed {:.3}ms, received data {data:?}, (sent {tx_timestamp:?})",
                                elapsed * 1e3
                            );
                        }
                        Ok(Message::Array(array)) => {
                            eprintln!("unexpected response {array:?}, tx rv was {tx_rv:?}");
                        }
                        Ok(Message::Error(err)) => {
                            eprintln!("unexpected response {err:?}, tx rv was {tx_rv:?}");
                        }
                        Err(err) => {
                            eprintln!("error {err:?}, tx rv was {tx_rv:?}");
                        }
                    }
                } else {
                    eprintln!("didn't receive anything");
                }
            }
            Err(err) => {
                eprintln!("recv err {err:?}, tx rv was {tx_rv:?}");
            }
        }

        /*
        // msg_bytes[2] += 1;
        if let Message::Data(ref mut data) = data {
            data.counter += 1;
            data.value0 += 0.1;
        }

        std::thread::sleep(std::time::Duration::from_secs(delay_secs));
        let msg_bytes = {
            match net_loopback::encode(&array, crc.digest()) {
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

        std::thread::sleep(std::time::Duration::from_secs(delay_secs));
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
        */
    }

    // Ok(())
}
