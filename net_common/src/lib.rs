#![no_std]

use postcard::{from_bytes_crc32, to_vec_crc32};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct TimeStamp {
    pub counter: u64,
    pub seconds: u64,
    pub nanoseconds: u32,
    pub stamp_ms: i64,
    pub ntp_offset: i64,
    pub ntp_seconds: u32,
    pub ntp_seconds_fraction: u32,
    pub ntp_roundtrip: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug, Default)]
pub struct SmallArray {
    pub data: [u8; 32],
}

// need nightly only feature generic_const_exprs
// for here W * H needs to equal SZ
// could use use serde_big_array::BigArray;
/*
#[derive(Serialize, Deserialize, Debug)]
// pub struct Image<const W: usize, const H: usize, const SZ: usize> {
pub struct Image {
    // const width: u16 = W,
    // const height: u16 = H,
    #[derive(Deserialize, Serialize)]
    pub data: [u8; 160 * 120],
}
*/

// TODO(lucasw) probably the enum needs to move into net_common also
#[derive(Debug)]
pub enum Message {
    TimeStamp(TimeStamp),
    Array(SmallArray),
    Error(()),
}

impl Message {
    pub const DATA: [u8; 4] = [0x5E, 0xA7, 0x00, 0x01];
    pub const ARRAY: [u8; 4] = [0x5E, 0xA7, 0x00, 0x02];

    // TODO(lucasw) make Message have a const to define the return message size
    pub fn encode<const SZ: usize>(
        &self,
        crc_digest: crc::Digest<'_, u32>,
    ) -> Result<heapless::Vec<u8, SZ>, postcard::Error> {
        let mut vec = heapless::Vec::<u8, SZ>::new();
        match self {
            Self::TimeStamp(some_data) => {
                for byte in &Message::DATA {
                    if vec.push(*byte).is_err() {
                        return Err(postcard::Error::SerializeBufferFull);
                    }
                }
                vec.extend(to_vec_crc32::<TimeStamp, SZ>(some_data, crc_digest)?);
            }
            Self::Array(small_array) => {
                for byte in &Message::ARRAY {
                    if vec.push(*byte).is_err() {
                        return Err(postcard::Error::SerializeBufferFull);
                    }
                }
                vec.extend(to_vec_crc32::<SmallArray, SZ>(small_array, crc_digest)?);
            }
            Self::Error(()) => {
                // TODO(lucasw) need a different error for this?
                return Err(postcard::Error::WontImplement);
            }
        }

        Ok(vec)
    }

    pub fn decode(
        msg_bytes: &[u8],
        crc_digest: crc::Digest<'_, u32>,
    ) -> Result<Self, postcard::Error> {
        // TODO(lucasw) return an error instead of unwrap
        let header: [u8; 4] = msg_bytes[..4].try_into().unwrap();

        match header {
            Self::DATA => {
                let data: TimeStamp = from_bytes_crc32(&msg_bytes[4..], crc_digest)?;
                Ok(Message::TimeStamp(data))
            }
            Self::ARRAY => {
                let array: SmallArray = from_bytes_crc32(&msg_bytes[4..], crc_digest)?;
                Ok(Message::Array(array))
            }
            _ => Ok(Message::Error(())),
        }
    }
}
