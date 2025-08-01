use net_common::{SmallArray, SomeData};
use postcard::{from_bytes_crc32, to_stdvec_crc32};

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
    Data(SomeData),
    Array(SmallArray),
    Error(()),
}

impl Message {
    pub const DATA: [u8; 4] = [0x5E, 0xA7, 0x00, 0x01];
    pub const ARRAY: [u8; 4] = [0x5E, 0xA7, 0x00, 0x02];

    pub fn decode(msg_bytes: &[u8], crc_digest: crc::Digest<'_, u32>) -> Result<Self, postcard::Error> {
        // TODO(lucasw) return an error instead of unwrap
        let header: [u8; 4] = msg_bytes[..4].try_into().unwrap();

        match header {
            Self::DATA => {
                let data: SomeData = from_bytes_crc32(&msg_bytes[4..], crc_digest)?;
                Ok(Message::Data(data))
            }
            Self::ARRAY => {
                let array: SmallArray = from_bytes_crc32(&msg_bytes[4..], crc_digest)?;
                Ok(Message::Array(array))
            }
            _ => Ok(Message::Error(())),
        }
    }

    pub fn encode(&self, crc_digest: crc::Digest<'_, u32>) -> Result<Vec<u8>, postcard::Error> {
        let mut vec = Vec::new();
        match &self {
            Self::Data(data) => {
                for byte in &Self::DATA {
                    vec.push(*byte);
                }
                vec.append(&mut to_stdvec_crc32(&data, crc_digest.clone())?);
            }
            Self::Array(small_array) => {
                for byte in &Self::ARRAY {
                    vec.push(*byte);
                }
                vec.append(&mut to_stdvec_crc32(&small_array, crc_digest.clone())?);
            }
            Self::Error(()) => {
                // TODO(lucasw) return an error instead of empty vec
            }
        }
        Ok(vec)
    }
}
