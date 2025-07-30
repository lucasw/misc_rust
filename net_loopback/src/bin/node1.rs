/*!
Receive packets from a sender

transmit on command line:

```
echo "my test message is much too long" | nc -u localhost 34201
```

or send with the node0 binary

*/

use std::net::UdpSocket;

fn main() -> std::io::Result<()> {
    // let addr0 = "127.0.0.1:34200";
    let addr1 = "127.0.0.1:34201";
    let socket = UdpSocket::bind(addr1)?;
    println!("{socket:?}");

    let mut buf = [0; 20];
    loop {
        match socket.recv_from(&mut buf) {
            Ok((rx_num, src)) => {
                if rx_num >= buf.len() {
                    eprintln!(
                        "more bytes may have been sent but only received {rx_num} == {}",
                        buf.len()
                    );
                }
                println!("{rx_num:?} bytes from {src:?}: {:X?}", &buf[..rx_num]);
            }
            Err(err) => {
                eprintln!("{err:?}");
            }
        }
        // std::thread::sleep(std::time::Duration::from_secs(1));
    }

    // Ok(())
}
