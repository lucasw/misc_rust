/*!
Adapted from git@github.com:lucasw/nucleo-h7xx.git examples/ethernet.rs

Simple ethernet example that will respond to icmp pings on
`IP_LOCAL` and periodically send a udp packet to
`IP_REMOTE:IP_REMOTE_PORT`
You can start a simple listening server with netcat:

nc -u -l 34254
*/

#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(unused_variables)]
#![no_main]
#![no_std]

use cortex_m_rt::entry;

use nucleo::loggit;
use nucleo_h7xx as nucleo;

use hal::gpio::Speed::*;
use hal::hal::digital::v2::OutputPin;
use hal::hal::digital::v2::ToggleableOutputPin;
use hal::prelude::*;
use hal::rcc::CoreClocks;
use hal::{ethernet, ethernet::PHY};
use nucleo::hal;

use hal::pac;
use pac::interrupt;

use smoltcp::iface::{
    Interface, InterfaceBuilder, Neighbor, NeighborCache, Route, Routes, SocketHandle,
};
use smoltcp::socket::{UdpPacketMetadata, UdpSocket, UdpSocketBuffer};
use smoltcp::storage::PacketMetadata;
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr, IpEndpoint, Ipv4Address, Ipv6Cidr};

use net_common::{Message, SomeData};

// use log::{debug, error, info};

const MAC_LOCAL: [u8; 6] = [0x02, 0x00, 0x11, 0x22, 0x33, 0x44];
// put this on the same ip address as your computer, make sure it isn't already in use
const IP_LOCAL: [u8; 4] = [192, 168, 0, 123];
// TODO(lucasw) this only works once?  Or nc only echoes it once?  I can restart nc and receive
// it again, and I see the openocd indicating the 1Hz sends aren't failing like when 192...255 is
// used
// const IP_REMOTE: [u8; 4] = [255, 255, 255, 255];

// these fail, only see one 'sent message' hprintln in openocd- the ethernet device is failing?
// const IP_REMOTE: [u8; 4] = [192, 255, 255, 255];
// const IP_REMOTE: [u8; 4] = [192, 168, 0, 255];

// TODO(lucasw) pass this in via env!- four separate ones, and it won't be const here
const IP_REMOTE: [u8; 4] = [192, 168, 0, 100];
// match the port in net_loopback/src/bin/node0.rs
const IP_REMOTE_PORT: u16 = 34201;

// mod utilities;

// need to disable semihosting to run outside of openocd + gdb
// use cortex_m_semihosting::hprintln;
// TODO(lucasw) instead of hprintln use the (usb) serial port for debug messages?
// dummy hprintln
#[macro_export]
macro_rules! hprintln {
    ( $( $x:expr ),* ) => {};
}

// TODO(lucasw) return result
fn send_message(
    data: &Message,
    crc: &crc::Crc<u32>,
    socket_handle: SocketHandle,
    remote_endpoint: IpEndpoint,
) {
    let msg_bytes = {
        match data.encode::<128>(crc.digest()) {
            Ok(msg_bytes) => msg_bytes,
            Err(err) => {
                hprintln!("encoding error");
                return;
            }
        }
    };

    // send something
    nucleo::ethernet::EthernetInterface::interrupt_free(|ethernet_interface| {
        let socket = ethernet_interface
            .interface
            .as_mut()
            .unwrap()
            .get_socket::<UdpSocket>(socket_handle);
        match socket.send_slice(&msg_bytes, remote_endpoint) {
            Ok(()) => {
                hprintln!("sent message");
                // hprintln!(msg);
            }
            Err(smoltcp::Error::Exhausted) => (),
            Err(e) => {
                hprintln!("UdpSocket::send error: {:?}", e);
            }
        };
    });
}

#[entry]
fn main() -> ! {
    // - endpoints ------------------------------------------------------------

    let local_endpoint = IpEndpoint::new(Ipv4Address::from_bytes(&IP_LOCAL).into(), 1234);
    // let ip_remote = IpAddress::BROADCAST;
    let remote_endpoint =
        IpEndpoint::new(Ipv4Address::from_bytes(&IP_REMOTE).into(), IP_REMOTE_PORT);
    //  IpEndpoint::new(ip_remote, IP_REMOTE_PORT);

    // - board setup ----------------------------------------------------------

    hprintln!("Setting up board");

    let board = nucleo::Board::take().unwrap();

    let dp = pac::Peripherals::take().unwrap();

    let ccdr = board.freeze_clocks(dp.PWR.constrain(), dp.RCC.constrain(), &dp.SYSCFG);

    let pins = board.split_gpios(
        dp.GPIOA.split(ccdr.peripheral.GPIOA),
        dp.GPIOB.split(ccdr.peripheral.GPIOB),
        dp.GPIOC.split(ccdr.peripheral.GPIOC),
        dp.GPIOD.split(ccdr.peripheral.GPIOD),
        dp.GPIOE.split(ccdr.peripheral.GPIOE),
        dp.GPIOF.split(ccdr.peripheral.GPIOF),
        dp.GPIOG.split(ccdr.peripheral.GPIOG),
    );

    nucleo_postcard::logger::init();

    // - ethernet interface ---------------------------------------------------

    hprintln!("Bringing up ethernet interface");

    let timeout_timer = dp
        .TIM17
        .timer(100.Hz(), ccdr.peripheral.TIM17, &ccdr.clocks);
    let timeout_timer = nucleo::timer::CountDownTimer::new(timeout_timer);
    let timeout_timer = match nucleo::ethernet::EthernetInterface::start(
        pins.ethernet,
        &MAC_LOCAL,
        &IP_LOCAL,
        ccdr.peripheral.ETH1MAC,
        &ccdr.clocks,
        timeout_timer,
    ) {
        Ok(tim17) => tim17,
        Err(e) => {
            hprintln!("Failed to start ethernet interface: {:?}", e);
            loop {}
        }
    };

    // wait for link to come up
    hprintln!("Waiting for link to come up");
    nucleo::ethernet::EthernetInterface::interrupt_free(
        |ethernet_interface| {
            while !ethernet_interface.poll_link() {}
        },
    );

    // create and bind socket
    let socket_handle = nucleo::ethernet::EthernetInterface::interrupt_free(|ethernet_interface| {
        let socket_handle = ethernet_interface.new_udp_socket();
        let socket = ethernet_interface
            .interface
            .as_mut()
            .unwrap()
            .get_socket::<UdpSocket>(socket_handle);
        match socket.bind(local_endpoint) {
            Ok(()) => socket_handle,
            Err(e) => {
                hprintln!("Failed to bind socket to endpoint: {:?}", local_endpoint);
                loop {}
            }
        }
    });

    // let msg = "nucleo says hello!\n";

    hprintln!("Entering main loop");
    // hprintln!(format!("{IP_REMOTE:?} {IP_REMOTE_PORT:?}"));

    let mut rx_buffer: [u8; 128] = [0; 128];

    // let mut last = 0;

    let crc = crc::Crc::<u32>::new(&crc::CRC_32_ISCSI);
    let mut data = Message::Data(SomeData {
        value0: 2.52457892,
        ..Default::default()
    });
    loop {
        cortex_m::asm::wfi();

        // poll ethernet interface
        let now = nucleo::ethernet::EthernetInterface::interrupt_free(|ethernet_interface| {
            match ethernet_interface.poll() {
                Ok(result) => {} // packets were processed or emitted
                Err(smoltcp::Error::Exhausted) => (),
                Err(smoltcp::Error::Unrecognized) => (),
                Err(e) => {
                    hprintln!("ethernet::EthernetInterface.poll() -> {:?}", e);
                }
            }
            ethernet_interface.now()
        });

        /*
        // check if it has been 1 second since we last sent something
        if (now - last) < 1000 {
            continue;
        } else {
            last = now;
        }
        */

        // receive something, and then send a response
        let (do_send, now) =
            nucleo::ethernet::EthernetInterface::interrupt_free(|ethernet_interface| {
                let socket = ethernet_interface
                    .interface
                    .as_mut()
                    .unwrap()
                    .get_socket::<UdpSocket>(socket_handle);
                let do_send = {
                    match socket.recv_slice(&mut rx_buffer) {
                        Ok((num_bytes, ip_endpoint)) => {
                            // hprintln!("received message");
                            // hprintln!(msg);
                            true
                        }
                        Err(e) => {
                            // hprintln!(": {:?}", e);
                            false
                        }
                    }
                };

                (do_send, ethernet_interface.now())
            });

        if !do_send {
            continue;
        }

        // last = now;

        if let Message::Data(ref mut data) = data {
            data.counter += 1;
            data.stamp_ms = now;
            data.value0 *= 1.05;
        }
        send_message(&data, &crc, socket_handle, remote_endpoint);
    }
}
