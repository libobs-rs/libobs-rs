//! Opaque transport and native-handle types used at the OBS actor seam.
//!
//! Downstream safe Rust cannot manufacture a `Send`/`Sync` wrapper for arbitrary
//! values. Owned native handles carry their native address in a shared lifetime lease,
//! while the runtime registry tracks only opaque identity/liveness for diagnostics.

use std::{
    collections::HashSet,
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
mod transport {
    /// Audited payloads for the crate-private actor transport.
    ///
    /// # Safety
    /// Moving or sharing the payload must not itself access a native pointee or run
    /// thread-affine destruction. Call sites must separately retain the pointee owner
    /// through each synchronous call, or transfer ownership into an actor cleanup task.
    /// Native access must follow the relevant OBS/OS thread and locking rules.
    pub(super) unsafe trait ActorTransport {}

    // SAFETY: Raw pointers have no destructor and copying them never accesses their
    // pointees. This marker does not authorize dereferencing or extending a lifetime.
    unsafe impl<T> ActorTransport for *mut T {}
    // SAFETY: As above, transporting a const pointer does not read the pointee.
    unsafe impl<T> ActorTransport for *const T {}
    // SAFETY: gs_window contains only borrowed OS handles; it has no native destructor.
    // Their owner/lifetime obligations are imposed by ObsWindowHandle's constructors.
    unsafe impl ActorTransport for libobs::gs_window {}
    // SAFETY: This is an owned copy of display configuration and borrowed window handles.
    // obs_display_create consumes it synchronously; window lifetime is retained separately.
    unsafe impl ActorTransport for crate::display::CloneableGsInitData {}
    // SAFETY: The allocation owns plain C data; moving its Box keeps its address stable.
    // ObsVideoInfo retains the graphics-module CString; raw access is actor-serialized.
    unsafe impl ActorTransport for std::pin::Pin<Box<libobs::obs_video_info>> {}
    // SAFETY: The pinned C struct has no Rust destructor for its native stack. CalldataWrapper
    // transfers this entire allocation to free_owned_calldata for actor-thread native cleanup.
    unsafe impl ActorTransport for std::pin::Pin<Box<libobs::calldata_t>> {}
}

#[cfg(feature = "enable_runtime")]
use transport::ActorTransport;

// SAFETY: Only the explicitly audited inert payloads above can cross the actor seam.
// The wrapper is crate-private; all dereferences and native destruction remain explicit.
#[cfg(feature = "enable_runtime")]
unsafe impl<T: ActorTransport> Send for Sendable<T> {}
// SAFETY: Sharing these payloads cannot access their pointees by itself. Native reads
// and writes still require the ownership and synchronization specified by ActorTransport.
#[cfg(feature = "enable_runtime")]
unsafe impl<T: ActorTransport> Sync for Sendable<T> {}

impl<T: Copy> Sendable<T> {
    /// Returns the transported value while keeping access to the audited transport wrapper
    /// crate-private. Using a method also prevents closure-capture lowering from bypassing the
    /// wrapper and capturing a raw pointer field directly.
    pub(crate) fn get(&self) -> T {
        self.0
    }
}

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

/// Registry owned by the OBS runtime.
///
/// It intentionally stores only opaque identities. Native addresses belong to the shared
/// handle lease, so an already-live handle never performs a fallible registry lookup to
/// recover its own pointer.
#[derive(Debug)]
pub(crate) struct NativeObjectRegistry {
    runtime_id: u64,
    next_id: AtomicU64,
    entries: RwLock<HashSet<NativeObjectId>>,
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
            entries: RwLock::new(HashSet::new()),
        }
    }
}

impl NativeObjectRegistry {
    pub(crate) fn register(&self) -> NativeObjectId {
        let id = NativeObjectId {
            runtime: self.runtime_id,
            object: self.next_id.fetch_add(1, Ordering::Relaxed),
        };
        let mut entries = self
            .entries
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        entries.insert(id);
        id
    }

    fn unregister(&self, id: NativeObjectId) {
        let mut entries = self
            .entries
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        entries.remove(&id);
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
    address: usize,
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
/// The handle itself stores only an opaque ID and a lease. Calling `get_ptr()` recovers
/// the address retained by that lease; safe downstream code cannot construct a
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
        let id = registry.register();
        Self {
            lease: Arc::new(NativeHandleLease {
                id,
                address: ptr.into_addr(),
                registry,
                _drop_guard: drop_guard,
            }),
            _pointer_type: PhantomData,
        }
    }

    pub fn native_id(&self) -> NativeObjectId {
        self.lease.id
    }

    /// Returns the native pointer retained by this handle's lease.
    ///
    /// The registry tracks identity/liveness for diagnostics, but an already-live handle
    /// does not re-resolve its own pointer through the registry. That makes the lease
    /// itself the source of truth for pointer validity and removes an impossible-state
    /// panic from every FFI call site.
    pub(crate) fn get_ptr(&self) -> P {
        // Safety: `address` was produced from a `P` when this lease was constructed.
        // `P` is sealed to raw pointer types, and the lease retains the native drop guard
        // until after its final handle is gone.
        unsafe { P::from_addr(self.lease.address) }
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
    use std::sync::atomic::AtomicUsize;

    #[derive(Debug)]
    struct NoopGuard;
    impl ObsDropGuard for NoopGuard {}

    #[cfg(feature = "enable_runtime")]
    #[test]
    fn transport_does_not_make_arbitrary_values_send_or_sync() {
        // If either auto-trait is implemented for this non-transport payload, inference
        // becomes ambiguous and this regression test fails at compile time.
        trait AmbiguousIfSend<A> {
            fn check() {}
        }
        impl<T: ?Sized> AmbiguousIfSend<()> for T {}
        impl<T: ?Sized + Send> AmbiguousIfSend<u8> for T {}
        trait AmbiguousIfSync<A> {
            fn check() {}
        }
        impl<T: ?Sized> AmbiguousIfSync<()> for T {}
        impl<T: ?Sized + Sync> AmbiguousIfSync<u8> for T {}
        let _ = <Sendable<std::rc::Rc<()>> as AmbiguousIfSend<_>>::check;
        let _ = <Sendable<std::rc::Rc<()>> as AmbiguousIfSync<_>>::check;
    }

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
    fn live_handle_pointer_does_not_depend_on_registry_lookup() {
        let registry = Arc::new(NativeObjectRegistry::default());
        let raw = Box::into_raw(Box::new(7_u32));
        let handle = SmartPointerSendable::new(raw, Arc::new(NoopGuard), registry.clone());

        // Simulate diagnostic registry state being unavailable. Pointer validity is a
        // property of the lease, not of a second map lookup.
        registry.unregister(handle.native_id());
        assert_eq!(registry.len(), 0);
        assert_eq!(handle.get_ptr(), raw);

        drop(handle);
        unsafe { drop(Box::from_raw(raw)) };
    }

    #[test]
    fn final_lease_unregisters_before_releasing_native_guard_exactly_once() {
        #[derive(Debug)]
        struct ObservingGuard {
            registry: Arc<NativeObjectRegistry>,
            drops: Arc<AtomicUsize>,
        }

        impl ObsDropGuard for ObservingGuard {}

        impl Drop for ObservingGuard {
            fn drop(&mut self) {
                assert_eq!(
                    self.registry.len(),
                    0,
                    "native identity must be unregistered before native cleanup begins"
                );
                self.drops.fetch_add(1, Ordering::SeqCst);
            }
        }

        let registry = Arc::new(NativeObjectRegistry::default());
        let drops = Arc::new(AtomicUsize::new(0));
        let raw = Box::into_raw(Box::new(99_u32));
        let handle = SmartPointerSendable::new(
            raw,
            Arc::new(ObservingGuard {
                registry: registry.clone(),
                drops: drops.clone(),
            }),
            registry.clone(),
        );
        let clones = (0..16).map(|_| handle.clone()).collect::<Vec<_>>();

        drop(handle);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        drop(clones);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(registry.len(), 0);

        unsafe { drop(Box::from_raw(raw)) };
    }

    #[test]
    fn registry_returns_to_baseline_after_clone_drop_stress() {
        let registry = Arc::new(NativeObjectRegistry::default());
        let mut allocations = Vec::new();

        for value in 0..512_u32 {
            let raw = Box::into_raw(Box::new(value));
            allocations.push(raw);
            let handle = SmartPointerSendable::new(raw, Arc::new(NoopGuard), registry.clone());
            let clones = (0..8).map(|_| handle.clone()).collect::<Vec<_>>();
            assert_eq!(registry.len(), 1);
            drop(handle);
            assert_eq!(registry.len(), 1);
            drop(clones);
            assert_eq!(registry.len(), 0);
        }

        for raw in allocations {
            unsafe { drop(Box::from_raw(raw)) };
        }
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
