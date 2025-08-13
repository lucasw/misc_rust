#![no_std]

use core::alloc::{GlobalAlloc, Layout};
use core::cell::UnsafeCell;
// use core::net::{IpAddr, Ipv4Addr};
use core::ptr::null_mut;
use core::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use cortex_m_semihosting::hprintln;

use embassy_executor::task;
use embassy_net::udp::{PacketMetadata, UdpMetadata, UdpSocket};
use embassy_net::{IpAddress, IpEndpoint, Ipv4Address, Stack};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Watch;
use embassy_time::{Instant, Timer};

use net_common::Epoch;

use sntpc::net::SocketAddr;
use sntpc::{
    NtpContext, NtpResult, NtpTimestampGenerator, NtpUdpSocket,
    Result,
};
use sntpc::sync::get_ntp_correction;

include!(concat!(env!("OUT_DIR"), "/constants.rs"));

// with more subscribers increase the '1' here
pub static NTP_WATCH: Watch<CriticalSectionRawMutex, NtpResult, 1> = Watch::new();

const ARENA_SIZE: usize = 128 * 1024;
const MAX_SUPPORTED_ALIGN: usize = 4096;
#[repr(C, align(4096))] // 4096 == MAX_SUPPORTED_ALIGN
struct SimpleAllocator {
    arena: UnsafeCell<[u8; ARENA_SIZE]>,
    remaining: AtomicUsize, // we allocate from the top, counting down
}

unsafe impl Sync for SimpleAllocator {}

unsafe impl GlobalAlloc for SimpleAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();

        // `Layout` contract forbids making a `Layout` with align=0, or align not power of 2.
        // So we can safely use a mask to ensure alignment without worrying about UB.
        let align_mask_to_round_down = !(align - 1);

        if align > MAX_SUPPORTED_ALIGN {
            return null_mut();
        }

        let mut allocated = 0;
        if self
            .remaining
            .fetch_update(Relaxed, Relaxed, |mut remaining| {
                if size > remaining {
                    return None;
                }
                remaining -= size;
                remaining &= align_mask_to_round_down;
                allocated = remaining;
                Some(remaining)
            })
            .is_err()
        {
            return null_mut();
        }

        unsafe { self.arena.get().cast::<u8>().add(allocated) }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        todo!()
    }

    unsafe fn realloc(&self, _ptr: *mut u8, _layout: Layout, _new_size: usize) -> *mut u8 {
        todo!()
    }
}

#[global_allocator]
static ALLOCATOR: SimpleAllocator = SimpleAllocator {
    arena: UnsafeCell::new([0x55; ARENA_SIZE]),
    remaining: AtomicUsize::new(ARENA_SIZE),
};

// nucleo-h7xx ethernet.rs says systick timer is 1ms
// adapted from sntpc examples simple-no-std + smoltcp
#[derive(Copy, Clone)]
pub struct TimestampGen {
    pub instant: Instant,
}

impl Default for TimestampGen {
    fn default() -> Self {
        Self {
            instant: Instant::from_ticks(0),
        }
    }
}

impl NtpTimestampGenerator for TimestampGen {
    fn init(&mut self) {
        self.instant = Instant::now();
    }

    fn timestamp_sec(&self) -> u64 {
        self.instant.as_secs()
    }

    fn timestamp_subsec_micros(&self) -> u32 {
        (self.instant - Instant::from_secs(self.timestamp_sec())).as_micros() as u32
    }
}

pub struct EmbassyUdpSocketWrapper<'a> {
    // TODO(lucasw) use something other than ref cell?
    // pub socket: RefCell<&'b mut UdpSocket<'a>>,
    pub socket: UdpSocket<'a>,
}

impl NtpUdpSocket for EmbassyUdpSocketWrapper<'_> {
    async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> Result<usize> {
        let endpoint = match addr {
            SocketAddr::V4(v4_ip_and_port) => {
                let v4 = v4_ip_and_port.ip().octets();
                IpEndpoint::new(
                    Ipv4Address::new(v4[0], v4[1], v4[2], v4[3]).into(),
                    v4_ip_and_port.port(),
                )
            }
            SocketAddr::V6(_) => return Err(sntpc::Error::Network),
        };

        let local_ip_addr = IpAddress::Ipv4(Ipv4Address::new(
            LOCAL_IP[0],
            LOCAL_IP[1],
            LOCAL_IP[2],
            LOCAL_IP[3],
        ));
        let udp_endpoint = UdpMetadata {
            // TODO(lucasw) experiment with broadcast
            // 255 has the same behavior as with nucleo-h7xx- one packet is received then no more
            endpoint,
            local_address: Some(local_ip_addr),
            meta: smoltcp::phy::PacketMeta::default(),
        };

        let rv = self.socket.send_to(buf, udp_endpoint).await;
        if rv.is_ok() {
            return Ok(buf.len());
        }
        hprintln!("{:?}", rv);

        Err(sntpc::Error::Network)
    }

    async fn recv_from(
        &self,
        buf: &mut [u8],
        // ) -> Result<(usize, SocketAddr), sntpc::Error> {
    ) -> Result<(usize, SocketAddr)> {
        let result = self.socket.recv_from(&mut buf[..]).await;

        if let Ok((size, meta)) = result {
            let sockaddr = SocketAddr::new(meta.endpoint.addr.into(), meta.endpoint.port);

            return Ok((size, sockaddr));
        }

        Err(sntpc::Error::Network)
    }
}

#[task]
pub async fn time_sync(stack: Stack<'static>, ntp_server_ip: [u8; 4]) -> ! {
    hprintln!("setting up time sync");
    let sender = NTP_WATCH.sender();
    /*
    let ntp_result0 = NtpResult::default();
    // store something here
    // TODO(lucasw) or maybe shouldn't so later now() calls will fail
    sender.send(ntp_result0);
    */

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
    let context = NtpContext::new(timestamp_gen);

    // TODO(lucasw) do this before calling time sync, pass in the wrapper?
    let sock_wrapper = EmbassyUdpSocketWrapper {
        socket,
    };

    hprintln!("starting time sync with ntp server: {:?}", remote_sock_addr);
    loop {
        Timer::after_millis(1000).await;

        let rv = get_ntp_correction(&remote_sock_addr, &sock_wrapper, context).await;
        match rv {
            Ok(new_rx_result) => {
                // TODO(lucasw) store last rx_result and compare offsets
                sender.send(new_rx_result);
                /*
                hprintln!(
                    "[{:?}] sntp offset: {:.2}s",
                    now,
                    new_rx_result.offset as f64 / 1e6
                );
                */

                /*
                let timestamp_msg = TimeStamp {
                    counter,
                    // TODO(lucasw) these are for the timestamp received from the other
                    // computer via a TimeStamp message, maybe get rid of them?
                    seconds: 0,
                    nanoseconds: 0,
                    tick_ms: now,
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
    }
}

const MS_PER_S: u64 = 1_000;
const US_PER_S: u64 = 1_000_000;
const NS_PER_S: u64 = 1_000_000_000;

/// get unix epoch seconds
/// TODO(lucasw) return seconds and subsecond nanos in a tuple?
pub fn now(ntp_result: Option<NtpResult>) -> (Epoch, Instant) {
    let tick_instant = Instant::now();
    let mut epoch = Epoch {
        secs: tick_instant.as_secs(),
        nanos: ((tick_instant.as_millis() - tick_instant.as_secs() * MS_PER_S) * US_PER_S) as u32,
    };
    let Some(ntp_result) = ntp_result else {
        return (epoch, tick_instant);
    };
    let ntp_us_offset = ntp_result.offset;
    let offset_secs = ntp_us_offset / US_PER_S as i64;
    epoch.secs += offset_secs as u64;
    epoch.nanos += ((ntp_us_offset - (offset_secs * US_PER_S as i64)) * MS_PER_S as i64) as u32;
    let more_secs = epoch.nanos / NS_PER_S as u32;
    epoch.secs += more_secs as u64;
    epoch.nanos -= more_secs * NS_PER_S as u32;
    (epoch, tick_instant)
}
