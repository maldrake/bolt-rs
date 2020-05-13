use std::collections::hash_map::RandomState;
use std::collections::HashMap;

use bolt_proto::Value;

#[derive(Default)]
pub struct Metadata {
    pub(crate) value: HashMap<String, Value>,
}

impl<K, V> From<HashMap<K, V>> for Metadata
where
    K: Into<String>,
    V: Into<Value>,
{
    fn from(map: HashMap<K, V, RandomState>) -> Self {
        Self {
            value: map.into_iter().map(|(k, v)| (k.into(), v.into())).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::iter::FromIterator;

    #[test]
    fn create_metadata() {
        let empty_metadata: Metadata = Default::default();
        assert!(empty_metadata.value.is_empty());
        let metadata = Metadata::from(HashMap::from_iter(vec![("key", "value")]));
        assert_eq!(
            HashMap::from_iter(vec![(String::from("key"), Value::from("value"))]),
            metadata.value
        );
    }
}
