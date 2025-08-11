#![no_std]

use core::alloc::{GlobalAlloc, Layout};
use core::cell::RefCell;
use core::cell::UnsafeCell;
// use core::net::{IpAddr, Ipv4Addr};
use core::ptr::null_mut;
use core::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use cortex_m_semihosting::hprintln;

use embassy_time::Instant;

use embassy_net::udp::{UdpMetadata, UdpSocket};
use embassy_net::{IpAddress, IpEndpoint, Ipv4Address};

use sntpc::net::SocketAddr;
use sntpc::{NtpTimestampGenerator, NtpUdpSocket, Result};

include!(concat!(env!("OUT_DIR"), "/constants.rs"));

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

pub struct EmbassyUdpSocketWrapper<'a, 'b> {
    pub socket: RefCell<&'b mut UdpSocket<'a>>,
}

impl NtpUdpSocket for EmbassyUdpSocketWrapper<'_, '_> {
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

        let rv = self.socket.borrow_mut().send_to(buf, udp_endpoint).await;
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
        let result = self.socket.borrow_mut().recv_from(&mut buf[..]).await;

        if let Ok((size, meta)) = result {
            let sockaddr = SocketAddr::new(meta.endpoint.addr.into(), meta.endpoint.port);

            return Ok((size, sockaddr));
        }

        Err(sntpc::Error::Network)
    }
}
