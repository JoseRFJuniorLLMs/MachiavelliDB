//! Immutable AVL map. Updates copy only the search path and share all other nodes.
use std::cmp::Ordering;
use std::sync::Arc;

type Link<K, V> = Option<Arc<Node<K, V>>>;
#[derive(Clone)]
struct Node<K, V> {
    key: K,
    value: V,
    left: Link<K, V>,
    right: Link<K, V>,
    height: u32,
    len: usize,
}
#[derive(Clone)]
pub(crate) struct Map<K, V> {
    root: Link<K, V>,
}
impl<K, V> Default for Map<K, V> {
    fn default() -> Self {
        Self { root: None }
    }
}
fn height<K, V>(n: &Link<K, V>) -> u32 {
    n.as_ref().map_or(0, |n| n.height)
}
fn len<K, V>(n: &Link<K, V>) -> usize {
    n.as_ref().map_or(0, |n| n.len)
}
impl<K: Ord + Clone, V: Clone> Map<K, V> {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn len(&self) -> usize {
        len(&self.root)
    }
    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }
    pub fn get(&self, key: &K) -> Option<&V> {
        let mut link = self.root.as_deref();
        while let Some(n) = link {
            match key.cmp(&n.key) {
                Ordering::Less => link = n.left.as_deref(),
                Ordering::Greater => link = n.right.as_deref(),
                Ordering::Equal => return Some(&n.value),
            }
        }
        None
    }
    fn make(key: K, value: V, left: Link<K, V>, right: Link<K, V>) -> Arc<Node<K, V>> {
        Arc::new(Node {
            key,
            value,
            height: 1 + height(&left).max(height(&right)),
            len: 1 + len(&left) + len(&right),
            left,
            right,
        })
    }
    fn balance(key: K, value: V, mut left: Link<K, V>, mut right: Link<K, V>) -> Arc<Node<K, V>> {
        if height(&left) > height(&right) + 1 {
            let l = left.as_ref().unwrap();
            if height(&l.right) > height(&l.left) {
                let r = l.right.as_ref().unwrap();
                left = Some(Self::make(
                    r.key.clone(),
                    r.value.clone(),
                    Some(Self::make(
                        l.key.clone(),
                        l.value.clone(),
                        l.left.clone(),
                        r.left.clone(),
                    )),
                    r.right.clone(),
                ));
            }
            let l = left.unwrap();
            return Self::make(
                l.key.clone(),
                l.value.clone(),
                l.left.clone(),
                Some(Self::make(key, value, l.right.clone(), right)),
            );
        }
        if height(&right) > height(&left) + 1 {
            let r = right.as_ref().unwrap();
            if height(&r.left) > height(&r.right) {
                let l = r.left.as_ref().unwrap();
                right = Some(Self::make(
                    l.key.clone(),
                    l.value.clone(),
                    l.left.clone(),
                    Some(Self::make(
                        r.key.clone(),
                        r.value.clone(),
                        l.right.clone(),
                        r.right.clone(),
                    )),
                ));
            }
            let r = right.unwrap();
            return Self::make(
                r.key.clone(),
                r.value.clone(),
                Some(Self::make(key, value, left, r.left.clone())),
                r.right.clone(),
            );
        }
        Self::make(key, value, left, right)
    }
    fn put(link: &Link<K, V>, key: K, value: V) -> Arc<Node<K, V>> {
        let Some(n) = link else {
            return Self::make(key, value, None, None);
        };
        match key.cmp(&n.key) {
            Ordering::Less => Self::balance(
                n.key.clone(),
                n.value.clone(),
                Some(Self::put(&n.left, key, value)),
                n.right.clone(),
            ),
            Ordering::Greater => Self::balance(
                n.key.clone(),
                n.value.clone(),
                n.left.clone(),
                Some(Self::put(&n.right, key, value)),
            ),
            Ordering::Equal => Self::make(key, value, n.left.clone(), n.right.clone()),
        }
    }
    pub fn insert(&mut self, key: K, value: V) {
        self.root = Some(Self::put(&self.root, key, value));
    }
    fn delete(link: &Link<K, V>, key: &K) -> Link<K, V> {
        let n = link.as_ref()?;
        match key.cmp(&n.key) {
            Ordering::Less => Some(Self::balance(
                n.key.clone(),
                n.value.clone(),
                Self::delete(&n.left, key),
                n.right.clone(),
            )),
            Ordering::Greater => Some(Self::balance(
                n.key.clone(),
                n.value.clone(),
                n.left.clone(),
                Self::delete(&n.right, key),
            )),
            Ordering::Equal => {
                if n.left.is_none() {
                    return n.right.clone();
                }
                if n.right.is_none() {
                    return n.left.clone();
                }
                let mut successor = n.right.as_ref().unwrap();
                while let Some(l) = &successor.left {
                    successor = l;
                }
                Some(Self::balance(
                    successor.key.clone(),
                    successor.value.clone(),
                    n.left.clone(),
                    Self::delete(&n.right, &successor.key),
                ))
            }
        }
    }
    pub fn remove(&mut self, key: &K) {
        self.root = Self::delete(&self.root, key);
    }
    pub fn iter(&self) -> Iter<'_, K, V> {
        let mut it = Iter { stack: Vec::new() };
        it.push_left(self.root.as_deref());
        it
    }
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, v)| v)
    }
}
pub(crate) struct Iter<'a, K, V> {
    stack: Vec<&'a Node<K, V>>,
}
impl<'a, K, V> Iter<'a, K, V> {
    fn push_left(&mut self, mut node: Option<&'a Node<K, V>>) {
        while let Some(n) = node {
            self.stack.push(n);
            node = n.left.as_deref();
        }
    }
}
impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);
    fn next(&mut self) -> Option<Self::Item> {
        let n = self.stack.pop()?;
        self.push_left(n.right.as_deref());
        Some((&n.key, &n.value))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn check<K: Ord, V>(link: &Link<K, V>) -> (u32, usize) {
        let Some(n) = link else { return (0, 0) };
        let (lh, ll) = check(&n.left);
        let (rh, rl) = check(&n.right);
        assert!(lh.abs_diff(rh) <= 1);
        assert_eq!(n.height, 1 + lh.max(rh));
        assert_eq!(n.len, 1 + ll + rl);
        (n.height, n.len)
    }
    #[test]
    fn ordered_updates_deletes_and_snapshots_match_btree() {
        let mut map = Map::new();
        let mut reference = std::collections::BTreeMap::new();
        let mut random = 17u64;
        for i in 0..10000 {
            random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
            let key = (random >> 32) % 501;
            if i % 3 == 0 {
                map.remove(&key);
                reference.remove(&key);
            } else {
                map.insert(key, i);
                reference.insert(key, i);
            }
            if i % 100 == 0 {
                check(&map.root);
                assert_eq!(
                    map.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>(),
                    reference.iter().map(|(k, v)| (*k, *v)).collect::<Vec<_>>()
                );
            }
        }
        let old = map.clone();
        for key in reference.keys() {
            map.remove(key);
        }
        assert!(map.is_empty());
        assert_eq!(old.len(), reference.len());
        check(&old.root);
    }
    #[test]
    fn updating_one_entry_does_not_clone_all_values() {
        #[derive(Debug)]
        struct Counted(Arc<std::sync::atomic::AtomicUsize>);
        impl Clone for Counted {
            fn clone(&self) -> Self {
                self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                Self(self.0.clone())
            }
        }
        let count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let mut map = Map::new();
        for i in 0..10000 {
            map.insert(i, Counted(count.clone()));
        }
        let old = map.clone();
        count.store(0, std::sync::atomic::Ordering::Relaxed);
        map.insert(5000, Counted(count.clone()));
        assert!(count.load(std::sync::atomic::Ordering::Relaxed) < 100);
        assert_eq!(old.len(), 10000);
    }
}
