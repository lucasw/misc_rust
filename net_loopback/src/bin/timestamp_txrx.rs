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

use clap::{Command, arg};
use net_common::{Epoch, Message, TimeStamp};
use std::net::UdpSocket;
use std::time::Duration;

fn main() -> std::io::Result<()> {
    let matches = Command::new("timestamp_txrx")
        .args(&[
            arg!(
                -l --local_ip <LOCAL_IP> "ip of local computer running this program"
            )
            .default_value("127.0.0.1"),
            arg!(
                -r --remote_ip <REMOTE_IP> "ip of remote device"
            )
            .default_value("192.168.0.123"),
        ])
        .get_matches();
    let local_ip = matches.get_one::<String>("local_ip").unwrap();
    let local_ip_port = format!("{local_ip}:34200");
    println!(
        "ip and port of this device {local_ip_port:?} (note 127.0.0.1 may not work with remote device)"
    );
    let socket = UdpSocket::bind(local_ip_port)?;
    // 1 second timeout on receiving
    let recv_timeout = Duration::new(1, 0);
    let result = socket.set_read_timeout(Some(recv_timeout));
    println!("set socket {socket:?} recv timeout to {recv_timeout:?}, {result:?}");

    let remote_ip = matches.get_one::<String>("remote_ip").unwrap();
    let remote_ip_port = format!("{remote_ip}:34201");
    println!("this socket is {socket:?}");
    println!("sending to {remote_ip_port:?}");

    let crc = crc::Crc::<u32>::new(&crc::CRC_32_ISCSI);

    let mut counter = 0;
    /*
    let mut array = SmallArray::default();
    array.data[6] = 0x23;
    // let mut array = Message::Array(array);
    */

    let delay_ms = 500;
    let accum_num = 1000 / delay_ms;
    let mut elapsed_accum = 0.0;

    loop {
        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        let tx_stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time went backwards");
        // if let Message::TimeStamp(ref mut data) = tx_timestamp
        let tx_timestamp = Message::TimeStamp(TimeStamp {
            epoch: Epoch {
                secs: tx_stamp.as_secs(),
                nanos: tx_stamp.subsec_nanos(),
            },
            counter,
            tick_ms: 0,
            ..Default::default()
        });
        counter += 1;

        let msg_bytes = {
            match net_loopback::encode(&tx_timestamp, crc.digest()) {
                Ok(msg_bytes) => msg_bytes,
                Err(err) => {
                    eprintln!("{err:?}");
                    continue;
                }
            }
        };
        let tx_rv = socket.send_to(&msg_bytes, &remote_ip_port);
        // println!("sent {data:?} encoded as {msg_bytes:X?}, rv {rv:?}");

        let mut rx_buffer = [0; 256];
        match socket.recv(&mut rx_buffer) {
            Ok(num_bytes) => {
                if num_bytes > 0 {
                    let rx_stamp = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .expect("time went backwards");
                    match Message::decode(&rx_buffer[..num_bytes], crc.digest()) {
                        Ok(Message::TimeStamp(data)) => {
                            let elapsed = (rx_stamp - tx_stamp).as_secs_f64();
                            elapsed_accum += elapsed;
                            if counter % accum_num == 0 {
                                println!(
                                    "[{rx_stamp:.03?}], elapsed avg {:.2}ms, cur {:.3}ms, received data {data:?}, (sent {tx_timestamp:?})",
                                    (elapsed_accum / accum_num as f64) * 1e3,
                                    elapsed * 1e3
                                );
                                elapsed_accum = 0.0;
                            }
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
        if let Message::TimeStamp(ref mut data) = data {
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
