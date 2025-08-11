/*!
nc -ul 34200 | hexdump
*/
#![no_main]
#![no_std]

use cortex_m_semihosting::hprintln;

use embassy_executor::{Spawner, main, task};
use embassy_net::udp::{PacketMetadata, RecvError, UdpMetadata, UdpSocket};
use embassy_net::{Ipv4Address, Ipv4Cidr, Stack, StackResources};
use embassy_stm32::eth::{Ethernet, GenericPhy, PacketQueue};
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::peripherals::ETH;
use embassy_stm32::{bind_interrupts, eth, peripherals, rng};
use embassy_time::{Instant, Timer};

// use smoltcp::socket::udp::UdpMetadata};
use smoltcp::wire::{IpAddress, IpEndpoint};

use nucleo_embassy::{LOCAL_IP, REMOTE_IP, TimestampGen};
use sntpc::sync::{sntp_process_response, sntp_send_request};
use sntpc::{NtpContext, NtpPacket, RawNtpPacket, get_ntp_timestamp};
use sntpc::{NtpTimestampGenerator, process_response};

use static_cell::StaticCell;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    hprintln!("{}", info);
    loop {}
}

type Device = Ethernet<'static, ETH, GenericPhy>;

#[embassy_executor::task]
async fn net_task(mut runner: embassy_net::Runner<'static, Device>) -> ! {
    runner.run().await
}

bind_interrupts!(struct Irqs {
    ETH => eth::InterruptHandler;
    HASH_RNG => rng::InterruptHandler<peripherals::RNG>;
});

#[main]
async fn main(spawner: Spawner) {
    hprintln!("embassy udp + led flashing start");

    let p = embassy_stm32::init(Default::default());
    /*
    let mut led_green = pb0.into_push_pull_output();
    let mut led_orange = gpioe.pe1.into_push_pull_output();
    let mut led_red = gpiob.pb14.into_push_pull_output();
    */
    let led_green = Output::new(p.PB0, Level::Low, Speed::Medium);
    let led_orange = Output::new(p.PE1, Level::Low, Speed::Medium);
    let led_red = Output::new(p.PB14, Level::High, Speed::Medium);

    // embassy/examples/stm32h7/src/bin/eth.rs
    let mac_addr = [0x00, 0x00, 0xDE, 0xAD, 0xBE, 0xEF];
    static PACKETS: StaticCell<PacketQueue<4, 4>> = StaticCell::new();

    let device = Ethernet::new(
        PACKETS.init(PacketQueue::<4, 4>::new()),
        p.ETH,
        Irqs,
        // these match nucleo-h7xx/src/ethernet.rs Pins
        p.PA1,  // ref_clk
        p.PA2,  // mdio
        p.PC1,  // eth_mdc
        p.PA7,  // CRS_DV: Carrier Sense
        p.PC4,  // RX_D0: Received Bit 0
        p.PC5,  // RX_D1: Received Bit 1
        p.PG13, // TX_D0: Transmit Bit 0
        p.PB13, // TX_D1: Transmit Bit 1
        p.PG11, // TX_EN: Transmit Enable
        GenericPhy::new_auto(),
        mac_addr,
    );

    // Generate random seed
    let mut rng = rng::Rng::new(p.RNG, Irqs);
    let mut seed = [0; 8];
    rng.fill_bytes(&mut seed);
    let seed = u64::from_le_bytes(seed);

    // from_bytes is in smoltcp, but not exposed through embassy_net?
    // let local_ip_addr = Ipv4Address::from_bytes(&[192, 168, 0, 123]);
    let local_ip_addr = Ipv4Address::new(LOCAL_IP[0], LOCAL_IP[1], LOCAL_IP[2], LOCAL_IP[3]);

    let config = embassy_net::Config::ipv4_static(embassy_net::StaticConfigV4 {
        address: Ipv4Cidr::new(local_ip_addr, 24),
        dns_servers: heapless::Vec::new(),
        gateway: Some(Ipv4Address::new(192, 168, 0, 1)),
    });

    // Init network stack
    static RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
    let (stack, runner) =
        embassy_net::new(device, config, RESOURCES.init(StackResources::new()), seed);
    spawner.spawn(net_task(runner)).unwrap();

    // TODO(lucasw) is it okay to clone the stack?
    spawner.must_spawn(time_sync(stack.clone(), REMOTE_IP));

    let mut rx_meta = [PacketMetadata::EMPTY; 16];
    let mut rx_buffer = [0; 4096];
    let mut tx_meta = [PacketMetadata::EMPTY; 16];
    let mut tx_buffer = [0; 4096];

    let mut socket = UdpSocket::new(
        stack,
        &mut rx_meta,
        &mut rx_buffer,
        &mut tx_meta,
        &mut tx_buffer,
    );
    let local_port = 34201;
    socket.bind(local_port).unwrap();

    let endpoint = UdpMetadata {
        // TODO(lucasw) use build.rs to set target ip address, also experiment with broadcast
        // 255 has the same behavior as with nucleo-h7xx- one packet is received then no more
        endpoint: IpEndpoint::new(
            Ipv4Address::new(REMOTE_IP[0], REMOTE_IP[1], REMOTE_IP[2], REMOTE_IP[3]).into(),
            34200,
        ),
        local_address: Some(IpAddress::Ipv4(local_ip_addr)),
        meta: smoltcp::phy::PacketMeta::default(),
    };

    spawner.must_spawn(flash_led(led_green, 500));
    spawner.must_spawn(flash_led(led_orange, 1100));
    spawner.must_spawn(flash_led(led_red, 2250));

    let mut rx_buf = [0; 4096];
    let mut tx_buf = [0; 4096];

    let mut count = 0;
    loop {
        let num = {
            hprintln!("wait for message on {:?} {}", local_ip_addr, local_port);
            match socket.recv_from(&mut rx_buf).await {
                Ok((num, _meta)) => {
                    // hprintln!("rx {}", num);
                    num
                }
                Err(RecvError::Truncated) => {
                    hprintln!("receive error truncated");
                    // continue;
                    rx_buf.len()
                }
            }
        };
        tx_buf[0] = 0xff;
        tx_buf[3] = (count / 256) as u8;
        tx_buf[2] = count as u8;
        tx_buf[6] = (num / 256) as u8;
        tx_buf[5] = num as u8;
        socket.send_to(&tx_buf[0..16], endpoint).await.unwrap();
        // Timer::after_millis(200).await;
        count += 1;
    }
}

#[task]
async fn time_sync(stack: Stack<'static>, ntp_server_ip: [u8; 4]) -> ! {
    hprintln!("setting up time sync");
    // TODO(lucasw) this needs to be fixed
    // TODO(lucasw) don't want to pass stack into the time_sync function,
    // but the above buffers need to persist forever
    let mut rx_meta0: [PacketMetadata; 16] = [PacketMetadata::EMPTY; 16];
    let mut rx_buffer0: [u8; 4096] = [0; 4096];
    let mut tx_meta0: [PacketMetadata; 16] = [PacketMetadata::EMPTY; 16];
    let mut tx_buffer0: [u8; 4096] = [0; 4096];

    let mut socket = UdpSocket::new(
        stack,
        &mut rx_meta0,
        &mut rx_buffer0,
        &mut tx_meta0,
        &mut tx_buffer0,
    );

    // TODO(lucasw) this port is never used (?), but the socket needs to be bound to something
    let local_port = 35201;
    socket.bind(local_port).unwrap();

    let ntp_port = 123;
    let remote_sock_addr = sntpc::net::SocketAddr::new(
        core::net::IpAddr::V4(core::net::Ipv4Addr::new(
            ntp_server_ip[0],
            ntp_server_ip[1],
            ntp_server_ip[2],
            ntp_server_ip[3],
        )),
        ntp_port,
    );

    let timestamp_gen = TimestampGen::default();
    let mut context = NtpContext::new(timestamp_gen);

    // TODO(lucasw) do this before calling time sync, pass in the wrapper?
    let sock_wrapper = nucleo_embassy::EmbassyUdpSocketWrapper {
        socket: (&mut socket).into(),
    };

    hprintln!("starting time sync with ntp server: {:?}", remote_sock_addr);
    loop {
        Timer::after_millis(1000).await;

        // TODO(lucasw) if any unhandled ntp results are sitting in the buffer this
        // fouls up, flush them above
        // this is working
        // { originate_timestamp: 9487534653230284800, version: 35 }
        let send_req_result = sntp_send_request(remote_sock_addr, &sock_wrapper, context);

        let now = Instant::now().as_millis() as f64 / 1e3;
        hprintln!("[{:.3}] {:?}", now, send_req_result);

        let send_req_result = {
            match send_req_result {
                Ok(send_req_result) => {
                    // TODO(lucasw) measure how long it took to get a response
                    // hprintln!("[{}] tx success: {:?}, now wait for response", now, send_req_result);
                    send_req_result
                }
                Err(e) => {
                    hprintln!("[{:.3}] sntp send error: {:?}", now, e);
                    continue;
                }
            }
        };

        // TODO(lucasw) this is a local version of sntp_process_response(), it appears to be working
        // where-as the sntpc version is locking up because it isn't meant to be used in the
        // embassy task environment?
        // I next need to clean up the hprintlns and report out the offset and see that it is stable,
        // then could make a fork of sntpc that provides this version of the function, but for
        // now it will exist here and be paired with a fork of sntpc that makes the needed structs
        // and functions public.
        // TODO(lucasw) lots of cargo clippy warnings to clean up
        let mut response_buf = RawNtpPacket::default();
        let rv = sock_wrapper
            .socket
            .borrow()
            .recv_from(response_buf.0.as_mut())
            .await;
        let (response, src) = {
            match rv {
                Ok((response, src)) => (response, src),
                Err(err) => {
                    hprintln!("sntp recv from error {:?}", err);
                    continue;
                }
            }
        };

        /*
        if dest != src {
            return Err(Error::ResponseAddressMismatch);
        }
        */

        if response != size_of::<NtpPacket>() {
            // Err(Error::IncorrectPayload);
            hprintln!("bad ntp rx size {} != {}", response, size_of::<NtpPacket>());
            continue;
        }

        context.timestamp_gen.init();
        let recv_timestamp = get_ntp_timestamp(&context.timestamp_gen);

        // hprintln!("{:?}", recv_timestamp);
        // let (response, src) i

        let result = process_response(send_req_result, response_buf, recv_timestamp);
        hprintln!("ntp process result: {:?}", result);

        /*
        // TODO(lucasw) this is locking up, maybe the socket recv_from within it is never returning
        let rv = sntp_process_response(
            remote_sock_addr,
            &sock_wrapper,
            context,
            ntp_tx_result,
        );

        let now = Instant::now().as_millis() as f64 / 1e3;
        hprintln!("[{:.3}] sntp process rv: {:?}", now, rv);

        match rv {
            Ok(new_rx_result) => {
                // TODO(lucasw) store last rx_result and compare offsets
                hprintln!(
                    "[{:?}] sntp offset: {:.2}s",
                    now.as_millis(),
                    new_rx_result.offset as f64 / 1e6
                );

                /*
                let timestamp_msg = TimeStamp {
                    counter,
                    // TODO(lucasw) these are for the timestamp received from the other
                    // computer via a TimeStamp message, maybe get rid of them?
                    seconds: 0,
                    nanoseconds: 0,
                    stamp_ms: now,
                    ntp_offset: new_rx_result.offset,
                    ntp_seconds: new_rx_result.seconds,
                    ntp_seconds_fraction: new_rx_result.seconds_fraction,
                    ntp_roundtrip: new_rx_result.roundtrip,
                };
                let tx_rv = send_message(
                    &Message::TimeStamp(timestamp_msg),
                    &crc,
                    socket_handle,
                    remote_endpoint,
                );
                match tx_rv {
                    Ok(num_bytes) => {}
                    Err(Error::Smoltcp(smoltcp::Error::Exhausted)) => {
                        hprintln!("exhausted");
                    }
                    Err(e) => {
                        hprintln!("UdpSocket::send error: {:?}", e);
                    }
                }
                */
            }
            Err(e) => {
                hprintln!("sntp response error {:?}", e);
                // TODO(lucasw) how to recover, just wait and try again?
                continue;
            }
        }

        // TODO(lucasw) broadcast latest ntp corrections out via a channel
        */
    }
}

#[task(pool_size = 3)]
async fn flash_led(mut gpio: Output<'static>, half_period: u64) -> ! {
    loop {
        gpio.set_high();
        Timer::after_millis(half_period).await;
        gpio.set_low();
        Timer::after_millis(half_period).await;
    }
}
