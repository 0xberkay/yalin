//! Owner of every native document handed to Java.
//!
//! Java never sees a pointer. It gets an opaque positive handle
//! `(generation << 32) | slot`. A stale, forged or already-closed handle
//! resolves to `None` instead of touching freed memory, because the slot's
//! generation changes on every release.
//!
//! Lookups hand out an `Arc`, so `close()` racing a query on another thread is
//! safe: the document is freed when the last in-flight query finishes.

use std::sync::{Arc, Mutex, MutexGuard};

use yalin_core::Document;

pub type OwnedDocument = Document<Vec<u8>>;

struct Slot {
    generation: u32,
    doc: Option<Arc<OwnedDocument>>,
}

#[derive(Default)]
pub struct Registry {
    slots: Vec<Slot>,
    free: Vec<u32>,
}

impl Registry {
    pub const fn new() -> Self {
        Self { slots: Vec::new(), free: Vec::new() }
    }

    /// Returns `None` only when all 2^32 slots are in use.
    pub fn insert(&mut self, doc: OwnedDocument) -> Option<i64> {
        let doc = Some(Arc::new(doc));
        let slot = match self.free.pop() {
            Some(slot) => {
                self.slots[slot as usize].doc = doc;
                slot
            }
            None => {
                let slot = u32::try_from(self.slots.len()).ok()?;
                self.slots.push(Slot { generation: 1, doc });
                slot
            }
        };
        Some(handle(self.slots[slot as usize].generation, slot))
    }

    pub fn get(&self, handle: i64) -> Option<Arc<OwnedDocument>> {
        self.live_slot(handle).and_then(|s| self.slots[s].doc.clone())
    }

    /// Unregisters a live handle, returning its document so the caller can
    /// drop it after releasing the lock.
    pub fn take(&mut self, handle: i64) -> Option<Arc<OwnedDocument>> {
        let s = self.live_slot(handle)?;
        let slot = &mut self.slots[s];
        let doc = slot.doc.take();
        // Generations stay in 1..=MAX_GENERATION so handles are always positive.
        slot.generation = if slot.generation >= MAX_GENERATION { 1 } else { slot.generation + 1 };
        self.free.push(s as u32);
        doc
    }

    fn live_slot(&self, handle: i64) -> Option<usize> {
        if handle <= 0 {
            return None;
        }
        let (generation, slot) = ((handle >> 32) as u32, handle as u32 as usize);
        let s = self.slots.get(slot)?;
        (s.generation == generation && s.doc.is_some()).then_some(slot)
    }
}

const MAX_GENERATION: u32 = i32::MAX as u32;

fn handle(generation: u32, slot: u32) -> i64 {
    (generation as i64) << 32 | slot as i64
}

static GLOBAL: Mutex<Registry> = Mutex::new(Registry::new());

/// The process-wide registry. A panic while holding the lock cannot leave it
/// inconsistent (every mutation is a single assignment), so poisoning is ignored.
pub fn global() -> MutexGuard<'static, Registry> {
    GLOBAL.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(s: &str) -> OwnedDocument {
        Document::parse(s.as_bytes().to_vec()).unwrap()
    }

    #[test]
    fn insert_get_remove() {
        let mut r = Registry::new();
        let a = r.insert(doc("1")).unwrap();
        let b = r.insert(doc("2")).unwrap();
        assert!(a > 0 && b > 0 && a != b);
        assert_eq!(r.get(a).unwrap().root().as_i64(), Ok(1));
        assert!(r.take(a).is_some());
        assert!(r.get(a).is_none(), "closed handle");
        assert!(r.take(a).is_none(), "double close");
        assert_eq!(r.get(b).unwrap().root().as_i64(), Ok(2));
    }

    #[test]
    fn reused_slot_rejects_stale_handle() {
        let mut r = Registry::new();
        let old = r.insert(doc("1")).unwrap();
        r.take(old);
        let new = r.insert(doc("2")).unwrap();
        assert_eq!(old as u32, new as u32, "same slot reused");
        assert!(r.get(old).is_none());
        assert_eq!(r.get(new).unwrap().root().as_i64(), Ok(2));
    }

    #[test]
    fn invalid_handles() {
        let mut r = Registry::new();
        r.insert(doc("1")).unwrap();
        for h in [0, -1, i64::MIN, 1, 1 << 32 | 5, i64::MAX] {
            assert!(r.get(h).is_none(), "{h}");
        }
    }

    #[test]
    fn in_flight_reference_outlives_close() {
        let mut r = Registry::new();
        let h = r.insert(doc(r#"{"a":"x"}"#)).unwrap();
        let held = r.get(h).unwrap();
        r.take(h);
        assert_eq!(held.get("a").unwrap().as_str().unwrap(), "x");
    }
}
