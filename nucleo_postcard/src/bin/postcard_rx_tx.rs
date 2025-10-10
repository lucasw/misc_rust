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

use smoltcp;
use smoltcp::iface::{
    Interface, InterfaceBuilder, Neighbor, NeighborCache, Route, Routes, SocketHandle,
};
use smoltcp::socket::{UdpPacketMetadata, UdpSocket, UdpSocketBuffer};
use smoltcp::storage::PacketMetadata;
use smoltcp::time::Instant;
use smoltcp::wire::{EthernetAddress, IpAddress, IpCidr, IpEndpoint, Ipv4Address, Ipv6Cidr};
use sntpc::NtpContext;
use sntpc::NtpTimestampGenerator;
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

#[derive(Debug)]
pub enum Error {
    Postcard(postcard::Error),
    Smoltcp(smoltcp::Error),
}
pub type Result<T> = core::result::Result<T, Error>;

// TODO(lucasw) return result
fn send_message(
    data: &Message,
    crc: &crc::Crc<u32>,
    socket_handle: SocketHandle,
    remote_endpoint: IpEndpoint,
) -> Result<usize> {
    let msg_bytes = {
        match data.encode::<128>(crc.digest()) {
            Ok(msg_bytes) => msg_bytes,
            // Err(postcard::Error(err)) => {
            Err(err) => {
                return Err(Error::Postcard(err));
            }
        }
    };

    // send something
    let rv = nucleo::ethernet::EthernetInterface::interrupt_free(|ethernet_interface| {
        let socket = ethernet_interface
            .interface
            .as_mut()
            .unwrap()
            .get_socket::<UdpSocket>(socket_handle);
        socket.send_slice(&msg_bytes, remote_endpoint)
    });
    match rv {
        Ok(()) => {
            return Ok((msg_bytes.len()));
            // hprintln!("sent message, {} bytes", msg_bytes.len());
            // hprintln!(msg);
        }
        // Err(smoltcp::Error(err)) => {
        Err(err) => {
            // Err(Error::Smoltcp(err)) => {
            return Err(Error::Smoltcp(err));
        }
    };
}

#[entry]
fn main() -> ! {
    // TODO(lucasw) option_env
    let local_endpoint = IpEndpoint::new(Ipv4Address::from_bytes(&LOCAL_IP).into(), LOCAL_PORT);
    // let ip_remote = IpAddress::BROADCAST;
    let remote_endpoint = IpEndpoint::new(Ipv4Address::from_bytes(&REMOTE_IP).into(), REMOTE_PORT);
    let ntp_endpoint = IpEndpoint::new(Ipv4Address::from_bytes(&REMOTE_IP).into(), 123);
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

    let ntp_port = 123;
    let remote_sock_addr = sntpc::net::SocketAddr::new(
        core::net::IpAddr::V4(core::net::Ipv4Addr::new(
            REMOTE_IP[0],
            REMOTE_IP[1],
            REMOTE_IP[2],
            REMOTE_IP[3],
        )),
        ntp_port,
    );

    // let mut rx_buffer: [u8; 128] = [0; 128];

    let mut counter = 0;
    let mut last = 0;
    let mut ntp_tx_result = None;
    let mut ntp_rx_result = None;

    let crc = crc::Crc::<u32>::new(&crc::CRC_32_ISCSI);

    let timestamp_gen = TimestampGen::default();
    let context = NtpContext::new(timestamp_gen);

    // TODO(lucasw) don't do hprintln inside interrupt_free unless error
    // (does it screw up the timer counter?
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

        /*
        // receive something, and then send a response
        let (msg, now) =
            nucleo::ethernet::EthernetInterface::interrupt_free(|ethernet_interface| {
                let socket = ethernet_interface
                    .interface
                    .as_mut()
                    .unwrap()
                    .get_socket::<UdpSocket>(socket_handle);
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
                        if counter % 2000 == 0 {
                            // hprintln!("nothing received: {:?}", e);
                        }
                        (None, now)
                    }
                }
            });
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

        // TODO(lucasw) async awaits would simplify this
        if let Some(last_tx_result) = ntp_tx_result {
            let (now, new_rx_result) =
                nucleo::ethernet::EthernetInterface::interrupt_free(|ethernet_interface| {
                    let socket = ethernet_interface
                        .interface
                        .as_mut()
                        .unwrap()
                        .get_socket::<UdpSocket>(socket_handle);

                    // TODO(lucasw) I can send a message and receive it within 1 ms, but the sntp
                    // sync seems to be jittering by much more than that unless I'm interpreting it
                    // wrong.
                    // How do I apply the NtpResult to the current time?  I can compare that result
                    // with receiving a packet with a timestamp from the ntp server in the
                    // (currently commented out code) below
                    let sock_wrapper = nucleo_postcard::SmoltcpUdpSocketWrapper {
                        socket: socket.into(),
                    };
                    let rv = sntp_process_response(
                        remote_sock_addr,
                        &sock_wrapper,
                        context,
                        last_tx_result,
                    );
                    let now = ethernet_interface.now();
                    (now, rv)
                });

            match new_rx_result {
                Ok(new_rx_result) => {
                    // TODO(lucasw) the hprintln may be fouling up timing
                    // TODO(lucasw) store last rx_result and compare offsets
                    /*
                    hprintln!(
                        "[{}] sntp offset: {:.2}s",
                        now,
                        // last_tx_result.originate_timestamp,
                        rx_result.offset as f64 / 1e6
                    );
                    */

                    let timestamp_msg = TimeStamp {
                        // TODO(lucasw) these are for the timestamp received from the other
                        // computer via a TimeStamp message, maybe get rid of them?
                        epoch: net_common::Epoch { secs: 0, nanos: 0 },
                        counter,
                        tick_ms: now as u64,
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

                    ntp_rx_result = Some(new_rx_result);
                    ntp_tx_result = None;
                }
                Err(sntpc::Error::Network) => {
                    // do nothing, this is the receive timing out, get the response in a later loop
                }
                Err(e) => {
                    hprintln!("sntp response error {:?}", e);
                    // TODO(lucasw) how to recover, just wait and try again?
                    ntp_tx_result = None;
                }
            }
        }

        /*
        // do a recv on the ntp connection in order to flush any incoming messages before doing an ntp sync
        {
            nucleo::ethernet::EthernetInterface::interrupt_free(|ethernet_interface| {
                let socket = ethernet_interface
                    .interface
                    .as_mut()
                    .unwrap()
                    .get_socket::<UdpSocket>(socket_handle);
                match socket.recv_slice(&mut rx_buffer) {
                    Ok((num_bytes, ntp_endpoint)) => {
                        let now = ethernet_interface.now();
                        // hprintln!("received old message?: {:?}", rx_buffer);
                        hprintln!("received old message?  Flushing");
                    }
                    Err(e) => {
                    }
                }
            });
        }
        */

        // check if it has been 1 second since we last sent something
        if (now - last) < 1000 {
            continue;
        }
        last = now;

        /*
        if count % 5000 == 0 {
            timestamp_gen.init();
            hprintln!("now {}ms, timestamp gen {} seconds, {}", now, timestamp_gen.timestamp_sec(), timestamp_gen.timestamp_subsec_micros());
        }
        */

        /*
        timestamp_gen.init();
        let t0 = timestamp_gen.duration_ticks;
        */

        nucleo::ethernet::EthernetInterface::interrupt_free(|ethernet_interface| {
            let socket = ethernet_interface
                .interface
                .as_mut()
                .unwrap()
                .get_socket::<UdpSocket>(socket_handle);

            let sock_wrapper = nucleo_postcard::SmoltcpUdpSocketWrapper {
                socket: socket.into(),
            };

            // TODO(lucasw) if any unhandled ntp results are sitting in the buffer this
            // fouls up, flush them above
            // this is working
            // { originate_timestamp: 9487534653230284800, version: 35 }
            let new_tx_result = sntp_send_request(remote_sock_addr, &sock_wrapper, context);

            match new_tx_result {
                Ok(new_tx_result) => {
                    // TODO(lucasw) measure how long it took to get a response
                    // hprintln!("[{}] tx success: {:?}, now wait for response", now, new_tx_result);
                    ntp_tx_result = Some(new_tx_result);
                }
                Err(e) => {
                    hprintln!("send error: {:?}", e);
                }
            }
        });

        /*
        timestamp_gen.init();
        let t1 = timestamp_gen.duration_ticks;
        let val = {
            ((timestamp_gen.timestamp_sec() + (u64::from(2_208_988_800u32))) << 32)
                + u64::from(timestamp_gen.timestamp_subsec_micros()) * u64::from(u32::MAX)
                    / u64::from(1_000_000u32)
        };
        hprintln!("t0 {}, t1 {}, {}", t0, t1, val);
        */

        counter += 1;
    }
}
