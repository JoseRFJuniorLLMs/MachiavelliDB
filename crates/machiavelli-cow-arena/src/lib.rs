//! Immutable anonymous file backed arena, private writable mappings per fork.
//! Forks use OS page CoW; sealing dirty contents into a new backing file is O(bytes).
//! Dropping a mapping asks the OS to unmap it; no constant-latency guarantee is made.
use arc_swap::ArcSwap;
use memmap2::{Mmap, MmapMut, MmapOptions};
use std::{
    fs::File,
    io::{self, Write},
    sync::Arc,
};

pub struct Arena {
    file: File,
    map: Mmap,
}
impl Arena {
    pub fn new(bytes: &[u8]) -> io::Result<Self> {
        if bytes.is_empty() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, "empty arena"));
        }
        let mut file = tempfile::tempfile()?;
        file.write_all(bytes)?;
        // SAFETY: anonymous private file, no path/handle exposed; after initialization
        // no writer can mutate/truncate the backing file while any mapping exists.
        let map = unsafe { MmapOptions::new().map(&file)? };
        Ok(Self { file, map })
    }
    pub fn bytes(&self) -> &[u8] {
        &self.map
    }
    pub fn fork(self: &Arc<Self>) -> io::Result<Branch> {
        // SAFETY: same immutable file lifetime; branch keeps the Arena alive.
        // map_copy never writes to the shared backing file.
        let map = unsafe { MmapOptions::new().map_copy(&self.file)? };
        Ok(Branch {
            base: self.clone(),
            map,
        })
    }
}
pub struct Branch {
    base: Arc<Arena>,
    map: MmapMut,
}
impl Branch {
    pub fn bytes(&self) -> &[u8] {
        &self.map
    }
    pub fn write(&mut self, offset: usize, bytes: &[u8]) -> io::Result<()> {
        let end = offset
            .checked_add(bytes.len())
            .filter(|&end| end <= self.map.len())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "write outside arena"))?;
        self.map[offset..end].copy_from_slice(bytes);
        Ok(())
    }
    pub fn base(&self) -> &Arc<Arena> {
        &self.base
    }
    /// Freeze a scenario for future forks. Copies bytes to a new immutable file.
    pub fn seal(&self) -> io::Result<Arc<Arena>> {
        Ok(Arc::new(Arena::new(&self.map)?))
    }
}
pub struct Head {
    current: ArcSwap<Arena>,
}
impl Head {
    pub fn new(arena: Arc<Arena>) -> Self {
        Self {
            current: ArcSwap::from(arena),
        }
    }
    pub fn snapshot(&self) -> Arc<Arena> {
        self.current.load_full()
    }
    /// Compare-and-swap publication avoids overwriting a concurrent promotion.
    pub fn promote(&self, branch: &Branch) -> io::Result<bool> {
        let next = branch.seal()?;
        let previous = self.current.compare_and_swap(branch.base(), next);
        Ok(Arc::ptr_eq(&previous, branch.base()))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn private_pages_and_atomic_promotion() {
        let base = Arc::new(Arena::new(&vec![0; 16384]).unwrap());
        let head = Head::new(base.clone());
        let old = head.snapshot();
        let mut first = base.fork().unwrap();
        let mut second = base.fork().unwrap();
        first.write(4096, &[1, 2, 3]).unwrap();
        second.write(4096, &[7, 8, 9]).unwrap();
        assert_eq!(&base.bytes()[4096..4099], &[0, 0, 0]);
        assert_eq!(&second.bytes()[4096..4099], &[7, 8, 9]);
        assert!(head.promote(&first).unwrap());
        assert!(!head.promote(&second).unwrap());
        assert_eq!(&head.snapshot().bytes()[4096..4099], &[1, 2, 3]);
        assert_eq!(&old.bytes()[4096..4099], &[0, 0, 0]);
        let next = head.snapshot().fork().unwrap();
        assert_eq!(&next.bytes()[4096..4099], &[1, 2, 3]);
    }
    #[test]
    fn writes_are_bounded() {
        let arena = Arc::new(Arena::new(&[0; 16]).unwrap());
        let mut branch = arena.fork().unwrap();
        assert!(branch.write(usize::MAX, &[1]).is_err());
        assert!(branch.write(16, &[1]).is_err());
        assert!(Arena::new(&[]).is_err());
    }
}
