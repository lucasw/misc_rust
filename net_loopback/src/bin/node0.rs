/*!
Send packets to a receiver

recieve on command line:

```
netcat -ul 34201
```

or in hex (but have to wait for a full line before seeing output)

```
netcat -ul 34201 | hexdump -C
```

*/

use std::net::UdpSocket;

fn main() -> std::io::Result<()> {
    let addr0 = "127.0.0.1:34200";
    let socket = UdpSocket::bind(addr0)?;
    let addr1 = "127.0.0.1:34201";
    println!("{socket:?}");

    let mut buf = [0x33, 0xBE, 0x0, 0x4, 0x6, 0x9];
    loop {
        buf[2] += 1;
        let rv = socket.send_to(&buf, &addr1);
        println!("sent {buf:X?}, {rv:?}");
        std::thread::sleep(std::time::Duration::from_secs(1));
    }

    // Ok(())
}
