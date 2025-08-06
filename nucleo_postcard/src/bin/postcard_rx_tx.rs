/*!
Adapted from git@github.com:lucasw/nucleo-h7xx.git examples/ethernet.rs

Simple ethernet example that will respond to icmp pings on
`IP_LOCAL` and periodically send a udp packet to
`REMOTE_IP:REMOTE_PORT`
You can start a simple listening server with netcat:

nc -u -l 34200
*/

#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_parens)]
#![allow(unused_variables)]
#![no_main]
#![no_std]

include!(concat!(env!("OUT_DIR"), "/constants.rs"));

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
use sntpc::NtpContext;
use sntpc::sync::{sntp_process_response, sntp_send_request};

use net_common::{Message, TimeStamp};
use nucleo_postcard::TimestampGen;

// use log::{debug, error, info};

const MAC_LOCAL: [u8; 6] = [0x02, 0x00, 0x11, 0x22, 0x33, 0x44];
// put this on the same ip address as your computer, make sure it isn't already in use
const LOCAL_IP: [u8; 4] = [192, 168, 0, 123];
const LOCAL_PORT: u16 = 34201;
// TODO(lucasw) this only works once?  Or nc only echoes it once?  I can restart nc and receive
// it again, and I see the openocd indicating the 1Hz sends aren't failing like when 192...255 is
// used
// const IP_REMOTE: [u8; 4] = [255, 255, 255, 255];

// these fail, only see one 'sent message' hprintln in openocd- the ethernet device is failing?
// const IP_REMOTE: [u8; 4] = [192, 255, 255, 255];
// const IP_REMOTE: [u8; 4] = [192, 168, 0, 255];

// match the port in net_loopback/src/bin/node0.rs
const REMOTE_PORT: u16 = 34200;

// mod utilities;

// need to disable semihosting to run outside of openocd + gdb, also remove it in the openocd.gdb
// monitor line
use cortex_m_semihosting::hprintln;
// TODO(lucasw) instead of hprintln use the (usb) serial port for debug messages?
// dummy hprintln
/*
#[macro_export]
macro_rules! hprintln {
    ( $( $x:expr ),* ) => {};
}
*/

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
                // hprintln!("sent message, {} bytes", msg_bytes.len());
                // hprintln!(msg);
            }
            Err(smoltcp::Error::Exhausted) => {
                hprintln!("exhausted");
            }
            Err(e) => {
                hprintln!("UdpSocket::send error: {:?}", e);
            }
        };
    });
}

#[entry]
fn main() -> ! {
    // TODO(lucasw) option_env
    let local_endpoint = IpEndpoint::new(Ipv4Address::from_bytes(&LOCAL_IP).into(), LOCAL_PORT);
    // let ip_remote = IpAddress::BROADCAST;
    let remote_endpoint = IpEndpoint::new(Ipv4Address::from_bytes(&REMOTE_IP).into(), REMOTE_PORT);
    //  IpEndpoint::new(ip_remote, REMOTE_IP_PORT);

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

    hprintln!(
        "Bringing up ethernet interface with local ip {:?} {}",
        LOCAL_IP,
        LOCAL_PORT
    );

    let timeout_timer = dp
        .TIM17
        .timer(100.Hz(), ccdr.peripheral.TIM17, &ccdr.clocks);
    let timeout_timer = nucleo::timer::CountDownTimer::new(timeout_timer);
    let timeout_timer = match nucleo::ethernet::EthernetInterface::start(
        pins.ethernet,
        &MAC_LOCAL,
        &LOCAL_IP,
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

    // can't embed variable in string with hprintln
    hprintln!(
        "Entering main loop, will send messages to {:?} {}",
        REMOTE_IP,
        REMOTE_PORT
    );

    let udp_port = 123;
    let remote_sock_addr = sntpc::net::SocketAddr::new(
        core::net::IpAddr::V4(core::net::Ipv4Addr::new(
            REMOTE_IP[0],
            REMOTE_IP[1],
            REMOTE_IP[2],
            REMOTE_IP[3],
        )),
        123,
    );

    // let mut rx_buffer: [u8; 128] = [0; 128];

    let mut count = 0;
    let mut last = 0;

    let crc = crc::Crc::<u32>::new(&crc::CRC_32_ISCSI);

    let timestamp_gen = TimestampGen::default();
    let context = NtpContext::new(timestamp_gen);

    loop {
        cortex_m::asm::wfi();

        // poll ethernet interface
        let now = nucleo::ethernet::EthernetInterface::interrupt_free(|ethernet_interface| {
            match ethernet_interface.poll() {
                Ok(result) => {
                    // hprintln!("ethernet poll result '{:?}'", result);
                } // packets were processed or emitted
                Err(smoltcp::Error::Exhausted) => (),
                Err(smoltcp::Error::Unrecognized) => (),
                Err(e) => {
                    hprintln!("ethernet::EthernetInterface.poll() -> {:?}", e);
                }
            }
            ethernet_interface.now()
        });

        // check if it has been 1 second since we last sent something
        if (now - last) < 3000 {
            continue;
        } else {
            last = now;
        }

        nucleo::ethernet::EthernetInterface::interrupt_free(|ethernet_interface| {
            let socket = ethernet_interface
                .interface
                .as_mut()
                .unwrap()
                .get_socket::<UdpSocket>(socket_handle);

            let sock_wrapper = nucleo_postcard::SmoltcpUdpSocketWrapper {
                socket: socket.into(),
            };

            // this is working
            // { originate_timestamp: 9487534653230284800, version: 35 }
            let tx_result = sntp_send_request(remote_sock_addr, &sock_wrapper, context);

            match tx_result {
                Ok(tx_result) => {
                    // this is failing with Err(Network) on first try, but then later succeeds:
                    // sntp results tx: SendRequestResult { originate_timestamp: 9487534653230284800, version: 35 } -> rx: Ok(NtpResult { seconds: 1754512190, seconds_fraction: 786663844, roundtrip: 0, offset: 1754512190183130, stratum: 3, precision: -26 })
                    // offset is in us
                    // TODO(lucasw) I can send a message and receive it within 1 ms, but the sntp
                    // sync seems to be jittering by much more than that unless I'm interpreting it
                    // wrong.
                    // How do I apply the NtpResult to the current time?  I can compare that result
                    // with receiving a packet with a timestamp from the ntp server in the
                    // (currently commented out code) below
                    let rx_result =
                        sntp_process_response(remote_sock_addr, &sock_wrapper, context, tx_result);
                    hprintln!("sntp results tx: {:?} -> rx: {:?}", tx_result, rx_result);
                }
                Err(e) => {
                    hprintln!("send error: {:?}", e);
                    // once_tx = true;
                }
            }
        });

        // just testing sntp currently
        continue;

        /*
        // receive something, and then send a response
        let (msg, now) =
            nucleo::ethernet::EthernetInterface::interrupt_free(|ethernet_interface| {
                let socket = ethernet_interface
                    .interface
                    .as_mut()
                    .unwrap()
                    .get_socket::<UdpSocket>(socket_handle);
                match socket.recv_slice(&mut rx_buffer) {
                    Ok((num_bytes, ip_endpoint)) => {
                        // want this stamp as close to reception as possible
                        let now = ethernet_interface.now();
                        // this prints so slow
                        // hprintln!("received message: {:?}", rx_buffer);
                        // hprintln!("received message: {} {:?}", num_bytes, ip_endpoint);
                        (Some(Message::decode(&rx_buffer, crc.digest())), now)
                    }
                    Err(e) => {
                        let now = ethernet_interface.now();
                        if count % 2000 == 0 {
                            // hprintln!("nothing received: {:?}", e);
                        }
                        (None, now)
                    }
                }
            });

        count += 1;
        last = now;

        if let Some(Ok(msg)) = msg {
            if let Message::Data(mut data) = msg {
                // send the received message back but with the local timestamp
                data.stamp_ms = now;
                send_message(&Message::Data(data), &crc, socket_handle, remote_endpoint);
            } else {
                todo!()
            }
        }
        */
    }
}
