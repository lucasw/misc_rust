use net_common::Message;
use postcard::to_stdvec_crc32;

// TODO(lucasw) can't put this in net_common because not no_std (though could put a no_std Vec into
// net_common?)
pub fn encode(
    message: &Message,
    crc_digest: crc::Digest<'_, u32>,
) -> Result<Vec<u8>, postcard::Error> {
    let mut vec = Vec::new();
    match &message {
        Message::TimeStamp(data) => {
            for byte in &Message::DATA {
                vec.push(*byte);
            }
            vec.append(&mut to_stdvec_crc32(&data, crc_digest.clone())?);
        }
        Message::Array(small_array) => {
            for byte in &Message::ARRAY {
                vec.push(*byte);
            }
            vec.append(&mut to_stdvec_crc32(&small_array, crc_digest.clone())?);
        }
        Message::Error(()) => {
            // TODO(lucasw) return a more appropriate error than this?
            return Err(postcard::Error::WontImplement);
        }
    }
    Ok(vec)
}
