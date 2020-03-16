use bolt_proto_derive::*;

pub(crate) const MARKER: u8 = 0xB0;
pub(crate) const SIGNATURE: u8 = 0x2F;

#[derive(Debug, Eq, PartialEq, Signature, Marker, Serialize, Deserialize)]
pub struct DiscardAll;

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;
    use std::sync::{Arc, Mutex};

    use bytes::Bytes;

    use super::*;

    #[test]
    fn try_from_bytes() {
        // No data needed!
        let bytes = Bytes::from_static(&[]);
        let discard_all = DiscardAll::try_from(Arc::new(Mutex::new(bytes)));
        assert!(discard_all.is_ok());
    }
}
