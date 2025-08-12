/*!
nc -ul 34200 | hexdump
*/
#![no_main]
#![no_std]

use cortex_m_semihosting::hprintln;

use embassy_executor::{Spawner, main, task};
use embassy_net::udp::{PacketMetadata, RecvError, UdpMetadata, UdpSocket};
use embassy_net::{Ipv4Address, Ipv4Cidr, StackResources};
use embassy_stm32::eth::{Ethernet, GenericPhy, PacketQueue};
use embassy_stm32::gpio::{Level, Output, Speed};
use embassy_stm32::peripherals::ETH;
use embassy_stm32::{bind_interrupts, eth, peripherals, rng};
use embassy_time::Timer;

// use smoltcp::socket::udp::UdpMetadata};
use smoltcp::wire::{IpAddress, IpEndpoint};

use nucleo_embassy::{LOCAL_IP, REMOTE_IP};

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
    spawner.must_spawn(nucleo_embassy::time_sync(stack.clone(), REMOTE_IP));

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

#[task(pool_size = 3)]
async fn flash_led(mut gpio: Output<'static>, half_period: u64) -> ! {
    loop {
        gpio.set_high();
        Timer::after_millis(half_period).await;
        gpio.set_low();
        Timer::after_millis(half_period).await;
    }
}
