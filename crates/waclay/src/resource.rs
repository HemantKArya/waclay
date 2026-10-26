//! Component resource tables: handle allocation for `canon resource.new`,
//! `canon resource.drop` and `canon resource.rep` intrinsics.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::{Error, Result};

/// A single resource type's handle table (handle index → representation).
#[derive(Default)]
pub struct ResourceTable {
    slots: Vec<Option<u64>>,
    free: Vec<u32>,
}

impl ResourceTable {
    fn alloc(&mut self, rep: u64) -> u32 {
        if let Some(idx) = self.free.pop() {
            self.slots[idx as usize] = Some(rep);
            idx
        } else {
            let idx = self.slots.len() as u32;
            self.slots.push(Some(rep));
            idx
        }
    }

    fn drop(&mut self, idx: u32) -> Result<u64> {
        let slot = self
            .slots
            .get_mut(idx as usize)
            .and_then(|s| s.take())
            .ok_or_else(|| Error::Resource(format!("invalid resource handle {idx}")))?;
        self.free.push(idx);
        Ok(slot)
    }

    fn rep(&self, idx: u32) -> Result<u64> {
        self.slots
            .get(idx as usize)
            .and_then(|s| *s)
            .ok_or_else(|| Error::Resource(format!("invalid resource handle {idx}")))
    }

    fn len(&self) -> usize {
        self.slots.iter().filter(|s| s.is_some()).count()
    }
}

/// All resource tables of one component instance, keyed by resource type id.
#[derive(Default)]
pub struct ResourceTables {
    tables: HashMap<u32, ResourceTable>,
}

impl ResourceTables {
    /// Create the handle for `canon resource.new`.
    pub fn resource_new(&mut self, resource: u32, rep: u64) -> u32 {
        self.tables.entry(resource).or_default().alloc(rep)
    }

    /// Drop a handle for `canon resource.drop`, returning its rep.
    pub fn resource_drop(&mut self, resource: u32, index: u32) -> Result<u64> {
        self.tables
            .get_mut(&resource)
            .ok_or_else(|| Error::Resource("no table for resource type".into()))?
            .drop(index)
    }

    /// Read the representation for `canon resource.rep`.
    pub fn resource_rep(&self, resource: u32, index: u32) -> Result<u64> {
        self.tables
            .get(&resource)
            .ok_or_else(|| Error::Resource("no table for resource type".into()))?
            .rep(index)
    }

    /// Number of live resources of one type (diagnostics/tests).
    pub fn live(&self, resource: u32) -> usize {
        self.tables.get(&resource).map(|t| t.len()).unwrap_or(0)
    }
}

/// Shared, lock-protected handle used by host callbacks.
pub type SharedTables = Arc<Mutex<ResourceTables>>;

/// Create a new shared table set.
pub fn shared() -> SharedTables {
    Arc::new(Mutex::new(ResourceTables::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alloc_drop_realloc() {
        let mut t = ResourceTables::default();
        let h1 = t.resource_new(7, 100);
        let h2 = t.resource_new(7, 200);
        assert_eq!(t.resource_rep(7, h1).unwrap(), 100);
        assert_eq!(t.resource_rep(7, h2).unwrap(), 200);
        assert_eq!(t.resource_drop(7, h1).unwrap(), 100);
        assert!(t.resource_rep(7, h1).is_err());
        let h3 = t.resource_new(7, 300);
        assert_eq!(h3, h1, "freelist reuses the dropped slot");
        assert_eq!(t.live(7), 2);
    }
}
