use std::convert::{TryFrom, TryInto};
use std::mem;
use std::panic::catch_unwind;
use std::sync::{Arc, Mutex};

use bytes::{Buf, BufMut, Bytes, BytesMut};

use crate::error::*;
use crate::serialization::*;
use crate::Value;

pub(crate) const MARKER_TINY: u8 = 0x90;
pub(crate) const MARKER_SMALL: u8 = 0xD4;
pub(crate) const MARKER_MEDIUM: u8 = 0xD5;
pub(crate) const MARKER_LARGE: u8 = 0xD6;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct List {
    pub(crate) value: Vec<Value>,
}

impl Marker for List {
    fn get_marker(&self) -> Result<u8> {
        match self.value.len() {
            0..=15 => Ok(MARKER_TINY | self.value.len() as u8),
            16..=255 => Ok(MARKER_SMALL),
            256..=65_535 => Ok(MARKER_MEDIUM),
            65_536..=4_294_967_295 => Ok(MARKER_LARGE),
            _ => Err(Error::ValueTooLarge(self.value.len())),
        }
    }
}

impl Serialize for List {}

impl TryInto<Bytes> for List {
    type Error = Error;

    fn try_into(self) -> Result<Bytes> {
        let marker = self.get_marker()?;
        let length = self.value.len();

        let mut total_value_bytes: usize = 0;
        let mut value_bytes_vec: Vec<Bytes> = Vec::with_capacity(length);
        for value in self.value {
            let value_bytes: Bytes = value.try_into()?;
            total_value_bytes += value_bytes.len();
            value_bytes_vec.push(value_bytes);
        }
        // Worst case is a large List, with marker byte, 32-bit size value, and all the
        // Value bytes
        let mut bytes = BytesMut::with_capacity(
            mem::size_of::<u8>() + mem::size_of::<u32>() + total_value_bytes,
        );
        bytes.put_u8(marker);
        match length {
            0..=15 => {}
            16..=255 => bytes.put_u8(length as u8),
            256..=65_535 => bytes.put_u16(length as u16),
            65_536..=4_294_967_295 => bytes.put_u32(length as u32),
            _ => return Err(Error::ValueTooLarge(length)),
        }
        for value_bytes in value_bytes_vec {
            bytes.put(value_bytes);
        }
        Ok(bytes.freeze())
    }
}

impl Deserialize for List {}

impl TryFrom<Arc<Mutex<Bytes>>> for List {
    type Error = Error;

    fn try_from(input_arc: Arc<Mutex<Bytes>>) -> Result<Self> {
        catch_unwind(move || {
            let marker = input_arc.lock().unwrap().get_u8();
            let size = match marker {
                marker if (MARKER_TINY..=(MARKER_TINY | 0x0F)).contains(&marker) => {
                    0x0F & marker as usize
                }
                MARKER_SMALL => input_arc.lock().unwrap().get_u8() as usize,
                MARKER_MEDIUM => input_arc.lock().unwrap().get_u16() as usize,
                MARKER_LARGE => input_arc.lock().unwrap().get_u32() as usize,
                _ => {
                    return Err(DeserializationError::InvalidMarkerByte(marker).into());
                }
            };
            let mut list: Vec<Value> = Vec::with_capacity(size);
            for _ in 0..size {
                list.push(Value::try_from(Arc::clone(&input_arc))?);
            }
            Ok(List::from(list))
        })
        .map_err(|_| DeserializationError::Panicked)?
    }
}

impl<T> From<Vec<T>> for List
where
    T: Into<Value>,
{
    fn from(value: Vec<T>) -> Self {
        Self {
            value: value.into_iter().map(|v| v.into()).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::value::*;

    use super::*;

    #[test]
    fn get_marker() {
        let empty_list: List = Vec::<i32>::new().into();
        let tiny_list: List = vec![0; 10].into();
        let small_list: List = vec![0; 100].into();
        let medium_list: List = vec![0; 1000].into();
        assert_eq!(empty_list.get_marker().unwrap(), MARKER_TINY);
        assert_eq!(
            tiny_list.get_marker().unwrap(),
            MARKER_TINY | tiny_list.value.len() as u8
        );
        assert_eq!(small_list.get_marker().unwrap(), MARKER_SMALL);
        assert_eq!(medium_list.get_marker().unwrap(), MARKER_MEDIUM);
    }

    #[test]
    #[ignore]
    fn get_marker_large() {
        let large_list: List = vec![0; 100_000].into();
        assert_eq!(large_list.get_marker().unwrap(), MARKER_LARGE);
    }

    #[test]
    fn try_into_bytes() {
        let empty_list: List = Vec::<i32>::new().into();
        let tiny_list: List = vec![100_000_000_000_i64; 10].into();
        let small_list: List = vec!["item"; 100].into();
        let medium_list: List = vec![false; 1000].into();
        assert_eq!(
            empty_list.try_into_bytes().unwrap(),
            Bytes::from_static(&[MARKER_TINY])
        );
        let tiny_list_item_bytes = Integer::from(100_000_000_000_i64).try_into_bytes().unwrap();
        let tiny_list_expected_bytes: Vec<u8> = vec![MARKER_TINY | 10]
            .into_iter()
            .chain(tiny_list_item_bytes.repeat(10).into_iter())
            .collect();
        assert_eq!(
            tiny_list.try_into_bytes().unwrap(),
            Bytes::from(tiny_list_expected_bytes)
        );
        let small_list_item_bytes = String::from("item").try_into_bytes().unwrap();
        let small_list_expected_bytes: Vec<u8> = vec![MARKER_SMALL, 0x64] // marker, size
            .into_iter()
            .chain(small_list_item_bytes.repeat(100).into_iter())
            .collect();
        assert_eq!(
            small_list.try_into_bytes().unwrap(),
            Bytes::from(small_list_expected_bytes)
        );
        let medium_list_item_bytes = Boolean::from(false).try_into_bytes().unwrap();
        let medium_list_expected_bytes: Vec<u8> = vec![MARKER_MEDIUM, 0x03, 0xE8] // marker, size
            .into_iter()
            .chain(medium_list_item_bytes.repeat(1000).into_iter())
            .collect();
        assert_eq!(
            medium_list.try_into_bytes().unwrap(),
            Bytes::from(medium_list_expected_bytes)
        );
    }

    #[test]
    #[ignore]
    fn try_into_large_bytes() {
        let large_list: List = vec![1_i8; 100_000].into();
        let large_list_item_bytes = Integer::from(1_i8).try_into_bytes().unwrap();
        let large_list_expected_bytes: Vec<u8> = vec![MARKER_LARGE, 0x00, 0x01, 0x86, 0xA0] // marker, size
            .into_iter()
            .chain(large_list_item_bytes.repeat(100_000).into_iter())
            .collect();
        assert_eq!(
            large_list.try_into_bytes().unwrap(),
            Bytes::from(large_list_expected_bytes)
        );
    }

    #[test]
    fn try_from_bytes() {
        let empty_list: List = Vec::<i32>::new().into();
        let empty_list_bytes = empty_list.clone().try_into_bytes().unwrap();
        let tiny_list: List = vec![100_000_000_000_i64; 10].into();
        let tiny_list_bytes = tiny_list.clone().try_into_bytes().unwrap();
        let small_list: List = vec!["item"; 100].into();
        let small_list_bytes = small_list.clone().try_into_bytes().unwrap();
        let medium_list: List = vec![false; 1000].into();
        let medium_list_bytes = medium_list.clone().try_into_bytes().unwrap();
        assert_eq!(
            List::try_from(Arc::new(Mutex::new(empty_list_bytes))).unwrap(),
            empty_list
        );
        assert_eq!(
            List::try_from(Arc::new(Mutex::new(tiny_list_bytes))).unwrap(),
            tiny_list
        );
        assert_eq!(
            List::try_from(Arc::new(Mutex::new(small_list_bytes))).unwrap(),
            small_list
        );
        assert_eq!(
            List::try_from(Arc::new(Mutex::new(medium_list_bytes))).unwrap(),
            medium_list
        );
    }

    #[test]
    #[ignore]
    fn try_from_large_bytes() {
        let large_list: List = vec![1_i8; 100_000].into();
        let large_list_bytes = large_list.clone().try_into_bytes().unwrap();
        assert_eq!(
            List::try_from(Arc::new(Mutex::new(large_list_bytes))).unwrap(),
            large_list
        );
    }
}
