//! A minimal insertion-order-preserving string-keyed map, standing in for Python
//! `dict`'s guaranteed insertion order. Needed because `yaml.safe_dump(sort_keys=False)`
//! emits keys in insertion order, and `bundle_store.save_manifest` round-trips a
//! `manifest.yaml` that must reproduce that order exactly (`okf_version` lands last,
//! after `exports`, per field-declaration order -- not alphabetically and not
//! matching the OKF_BUNDLE_SPEC's documented example order).

#[derive(Debug, Clone, PartialEq, Default)]
pub struct OrderedMap<V> {
    entries: Vec<(String, V)>,
}

impl<V> OrderedMap<V> {
    pub fn new() -> Self {
        Self { entries: Vec::new() }
    }

    /// Inserts, or overwrites in place (preserving the original position) if the
    /// key already exists -- matching Python `dict[key] = value` semantics.
    pub fn insert(&mut self, key: impl Into<String>, value: V) {
        let key = key.into();
        if let Some(entry) = self.entries.iter_mut().find(|(k, _)| *k == key) {
            entry.1 = value;
        } else {
            self.entries.push((key, value));
        }
    }

    pub fn get(&self, key: &str) -> Option<&V> {
        self.entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.entries.iter().any(|(k, _)| k == key)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &V)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v))
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl<V> FromIterator<(String, V)> for OrderedMap<V> {
    fn from_iter<T: IntoIterator<Item = (String, V)>>(iter: T) -> Self {
        let mut map = Self::new();
        for (k, v) in iter {
            map.insert(k, v);
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_insertion_order_including_overwrite() {
        let mut m: OrderedMap<i32> = OrderedMap::new();
        m.insert("id", 1);
        m.insert("version", 2);
        m.insert("id", 99); // overwrite in place, position unchanged
        let keys: Vec<&str> = m.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["id", "version"]);
        assert_eq!(m.get("id"), Some(&99));
    }
}
