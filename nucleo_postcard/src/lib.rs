#![no_std]

use core::alloc::{GlobalAlloc, Layout};
use core::cell::RefCell;
use core::cell::UnsafeCell;
use core::net::{IpAddr, Ipv4Addr};
use core::ptr::null_mut;
use core::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use smoltcp::socket::UdpSocket;
use smoltcp::wire::{IpAddress, IpEndpoint, Ipv4Address};
use sntpc::net::SocketAddr;
use sntpc::{NtpTimestampGenerator, NtpUdpSocket, Result};

pub mod logger;

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
const TICKS_PER_SEC: u32 = 1_000;

// adapted from sntpc examples simple-no-std + smoltcp
#[derive(Copy, Clone, Default)]
pub struct TimestampGen {
    pub duration_ticks: u32,
}

impl NtpTimestampGenerator for TimestampGen {
    fn init(&mut self) {
        // TODO(lucasw) does this need to be interrupt free?
        // self.duration_ticks += 3000;
        self.duration_ticks = nucleo_h7xx::ethernet::ATOMIC_TIME.load(Relaxed);
    }

    fn timestamp_sec(&self) -> u64 {
        (self.duration_ticks / TICKS_PER_SEC) as u64
    }

    fn timestamp_subsec_micros(&self) -> u32 {
        (self.duration_ticks - self.timestamp_sec() as u32 * TICKS_PER_SEC) * 1_000
    }
}

pub struct SmoltcpUdpSocketWrapper<'a, 'b> {
    // pub socket: UdpSocket<'a>,
    pub socket: RefCell<&'b mut UdpSocket<'a>>,
}

impl NtpUdpSocket for SmoltcpUdpSocketWrapper<'_, '_> {
    async fn send_to(&self, buf: &[u8], addr: SocketAddr) -> Result<usize> {
        // , sntpc::Error> {
        let endpoint = match addr {
            SocketAddr::V4(v4) => IpEndpoint::new(
                IpAddress::Ipv4(Ipv4Address::from_bytes(&v4.ip().octets())),
                v4.port(),
            ),
            SocketAddr::V6(_) => return Err(sntpc::Error::Network),
        };

        if self.socket.borrow_mut().send_slice(buf, endpoint).is_ok() {
            return Ok(buf.len());
        }

        Err(sntpc::Error::Network)
    }

    async fn recv_from(
        &self,
        buf: &mut [u8],
        // ) -> Result<(usize, SocketAddr), sntpc::Error> {
    ) -> Result<(usize, SocketAddr)> {
        let result = self.socket.borrow_mut().recv_slice(&mut buf[..]);

        if let Ok((size, endpoint)) = result {
            // make compiler and clippy happy as without the else branch clippy complains
            // that not all variants covered for some reason
            #[allow(irrefutable_let_patterns)]
            let IpAddress::Ipv4(v4) = endpoint.addr else {
                todo!()
            };
            // TODO(lucasw) more compact conversion to SocketAddr?
            let ip = v4.as_bytes();
            let sockaddr = SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(ip[0], ip[1], ip[2], ip[3])),
                endpoint.port,
            );

            return Ok((size, sockaddr));
        }

        Err(sntpc::Error::Network)
    }
}
