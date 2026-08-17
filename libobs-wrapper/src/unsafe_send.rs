//! Opaque transport and native-handle types used at the OBS actor seam.
//!
//! Downstream safe Rust cannot manufacture a `Send`/`Sync` wrapper for arbitrary
//! values. Owned native handles are represented by a runtime-registry ID rather than
//! carrying the raw OBS pointer in every clone.

use std::{
    collections::HashMap,
    fmt,
    marker::PhantomData,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, RwLock,
    },
};

use crate::utils::ObsDropGuard;

static NEXT_RUNTIME_ID: AtomicU64 = AtomicU64::new(1);

/// An opaque value transported to the OBS actor.
///
/// The tuple field and constructor are crate-private, so downstream safe code cannot
/// use this type to manufacture `Send`/`Sync` for arbitrary values.
#[derive(Debug, Clone)]
pub(crate) struct Sendable<T>(pub(crate) T);

#[cfg(feature = "enable_runtime")]
unsafe impl<T> Send for Sendable<T> {}
#[cfg(feature = "enable_runtime")]
unsafe impl<T> Sync for Sendable<T> {}

mod native_pointer_sealed {
    pub trait Sealed {}
    impl<T> Sealed for *mut T {}
    impl<T> Sealed for *const T {}
}

/// Sealed marker for native pointer types accepted by [`SmartPointerSendable`].
#[doc(hidden)]
pub trait NativePointer: native_pointer_sealed::Sealed + Copy + fmt::Debug + 'static {
    #[doc(hidden)]
    fn into_addr(self) -> usize;
    #[doc(hidden)]
    unsafe fn from_addr(addr: usize) -> Self;
}

impl<T: 'static> NativePointer for *mut T {
    fn into_addr(self) -> usize {
        self as usize
    }

    unsafe fn from_addr(addr: usize) -> Self {
        addr as *mut T
    }
}

impl<T: 'static> NativePointer for *const T {
    fn into_addr(self) -> usize {
        self as usize
    }

    unsafe fn from_addr(addr: usize) -> Self {
        addr as *const T
    }
}

/// Stable identity assigned by the runtime to one owned native OBS object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NativeObjectId {
    runtime: u64,
    object: u64,
}

impl NativeObjectId {
    /// Returns the per-runtime object sequence number.
    ///
    /// The complete `NativeObjectId` is the identity. This sequence value alone is
    /// intentionally not globally unique and should only be used for diagnostics.
    pub fn sequence(self) -> u64 {
        self.object
    }
}

/// Registry owned by the OBS runtime. Native addresses live here rather than inside
/// every public wrapper clone.
#[derive(Debug)]
pub(crate) struct NativeObjectRegistry {
    runtime_id: u64,
    next_id: AtomicU64,
    entries: RwLock<HashMap<NativeObjectId, usize>>,
}

impl Default for NativeObjectRegistry {
    fn default() -> Self {
        // libobs is process-global, so only one registry can be active at a time.
        // A process-wide u64 epoch keeps copied/stale IDs from a previous context
        // from ever comparing equal to a later native object in realistic operation.
        let runtime_id = NEXT_RUNTIME_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            runtime_id,
            next_id: AtomicU64::new(1),
            entries: RwLock::new(HashMap::new()),
        }
    }
}

impl NativeObjectRegistry {
    pub(crate) fn register<P: NativePointer>(&self, pointer: P) -> NativeObjectId {
        let id = NativeObjectId {
            runtime: self.runtime_id,
            object: self.next_id.fetch_add(1, Ordering::Relaxed),
        };
        let mut entries = self
            .entries
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        entries.insert(id, pointer.into_addr());
        id
    }

    fn unregister(&self, id: NativeObjectId) {
        let mut entries = self
            .entries
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        entries.remove(&id);
    }

    fn resolve<P: NativePointer>(&self, id: NativeObjectId) -> P {
        let entries = self
            .entries
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let address = *entries
            .get(&id)
            .expect("native handle registry invariant violated");
        // Safety: registration and resolution use the same sealed pointer type at the
        // `SmartPointerSendable<P>` boundary, and the lease keeps the entry alive.
        unsafe { P::from_addr(address) }
    }

    pub(crate) fn len(&self) -> usize {
        self.entries
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .len()
    }

    pub(crate) fn runtime_id(&self) -> u64 {
        self.runtime_id
    }
}

#[derive(Debug)]
struct NativeHandleLease {
    id: NativeObjectId,
    registry: Arc<NativeObjectRegistry>,
    // Kept alive until after unregister. Dropping the guard schedules the actual
    // native release on the OBS actor.
    _drop_guard: Arc<dyn ObsDropGuard>,
}

impl Drop for NativeHandleLease {
    fn drop(&mut self) {
        self.registry.unregister(self.id);
    }
}

/// A lifetime-carrying native handle backed by the runtime's object registry.
///
/// The handle itself stores only an opaque ID and a lease. Calling `get_ptr()` resolves
/// the address from the runtime registry; safe downstream code cannot construct a
/// handle for arbitrary memory.
pub struct SmartPointerSendable<P: NativePointer> {
    lease: Arc<NativeHandleLease>,
    _pointer_type: PhantomData<fn() -> P>,
}

impl<P: NativePointer> SmartPointerSendable<P> {
    pub(crate) fn new(
        ptr: P,
        drop_guard: Arc<dyn ObsDropGuard>,
        registry: Arc<NativeObjectRegistry>,
    ) -> Self {
        let id = registry.register(ptr);
        Self {
            lease: Arc::new(NativeHandleLease {
                id,
                registry,
                _drop_guard: drop_guard,
            }),
            _pointer_type: PhantomData,
        }
    }

    pub fn native_id(&self) -> NativeObjectId {
        self.lease.id
    }

    /// Resolve the native pointer while this handle keeps its runtime registry entry
    /// alive. Dereferencing or retaining the pointer beyond the handle lifetime remains
    /// subject to the usual raw-pointer safety rules.
    pub(crate) fn get_ptr(&self) -> P {
        self.lease.registry.resolve(self.lease.id)
    }

    /// Returns the underlying native pointer for advanced FFI integrations.
    ///
    /// # Safety
    /// The pointer is only valid while this handle and its owning runtime remain alive.
    /// The caller must obey libobs thread-affinity, ownership, and reference-count rules,
    /// must not release a borrowed pointer, and must not retain callback-borrowed pointers
    /// past their documented C lifetime.
    #[doc(hidden)]
    pub unsafe fn raw_ptr_unchecked(&self) -> P {
        self.get_ptr()
    }
}

impl<P: NativePointer> Clone for SmartPointerSendable<P> {
    fn clone(&self) -> Self {
        Self {
            lease: self.lease.clone(),
            _pointer_type: PhantomData,
        }
    }
}

impl<P: NativePointer> fmt::Debug for SmartPointerSendable<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SmartPointerSendable")
            .field("native_id", &self.native_id())
            .finish()
    }
}

impl<P: NativePointer> PartialEq for SmartPointerSendable<P> {
    fn eq(&self, other: &Self) -> bool {
        self.native_id() == other.native_id()
    }
}
impl<P: NativePointer> Eq for SmartPointerSendable<P> {}

impl<P: NativePointer> std::hash::Hash for SmartPointerSendable<P> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.native_id().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct NoopGuard;
    impl ObsDropGuard for NoopGuard {}

    #[test]
    fn native_handle_registry_lives_until_last_clone() {
        let registry = Arc::new(NativeObjectRegistry::default());
        let raw = Box::into_raw(Box::new(42_u32));
        let handle = SmartPointerSendable::new(raw, Arc::new(NoopGuard), registry.clone());
        assert_eq!(registry.len(), 1);
        assert_eq!(handle.get_ptr(), raw);

        let clone = handle.clone();
        assert_eq!(handle.native_id(), clone.native_id());
        drop(handle);
        assert_eq!(registry.len(), 1);
        drop(clone);
        assert_eq!(registry.len(), 0);

        // The no-op test guard intentionally does not own the allocation.
        unsafe { drop(Box::from_raw(raw)) };
    }

    #[test]
    fn native_ids_do_not_alias_across_runtime_registries() {
        let first = Arc::new(NativeObjectRegistry::default());
        let second = Arc::new(NativeObjectRegistry::default());
        let a = Box::into_raw(Box::new(1_u8));
        let b = Box::into_raw(Box::new(2_u8));
        let first_handle = SmartPointerSendable::new(a, Arc::new(NoopGuard), first);
        let second_handle = SmartPointerSendable::new(b, Arc::new(NoopGuard), second);

        assert_ne!(first_handle.native_id(), second_handle.native_id());
        assert_eq!(first_handle.native_id().sequence(), 1);
        assert_eq!(second_handle.native_id().sequence(), 1);

        drop(first_handle);
        drop(second_handle);
        unsafe {
            drop(Box::from_raw(a));
            drop(Box::from_raw(b));
        }
    }
}
