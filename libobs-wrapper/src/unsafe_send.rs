//! Sendable wrapper types for non-Send types
//!
//! This module provides wrapper types that allow non-Send types to be sent
//! across thread boundaries. Use with caution - these are unsafe by design.

use std::{hash::Hash, sync::Arc};

use crate::utils::ObsDropGuard;

#[derive(Debug, Clone)]
pub struct Sendable<T>(pub T);

unsafe impl<T> Send for Sendable<T> {}
unsafe impl<T> Sync for Sendable<T> {}

#[derive(Debug, Hash, Clone, PartialEq, Eq)]
pub struct SendableComp<T>(pub T);

unsafe impl<T: PartialEq> Send for SendableComp<T> {}
unsafe impl<T: PartialEq> Sync for SendableComp<T> {}

#[derive(Debug, Clone)]
/// Most of the time, this smart pointer will need to be the last field of a struct to ensure proper drop order.
pub struct SmartPointerSendable<T: Clone> {
    ptr: T,
    drop_guard: Arc<dyn ObsDropGuard>,
}

impl<T: Clone> SmartPointerSendable<T> {
    pub fn new(ptr: T, drop_guard: Arc<dyn ObsDropGuard>) -> Self {
        Self { ptr, drop_guard }
    }

    pub fn get_ptr(&self) -> T {
        self.ptr.clone()
    }

    pub fn into_comp(self) -> SmartPointerSendableComp<T> {
        SmartPointerSendableComp::new(self.ptr, self.drop_guard)
    }

    /// # Safety
    /// This exposes the drop guard, which may lead to misuse. Make sure to only use it when you need it.
    pub unsafe fn drop_guard(&self) -> Arc<dyn ObsDropGuard> {
        self.drop_guard.clone()
    }
}

unsafe impl<T: Clone> Send for SmartPointerSendable<T> {}
unsafe impl<T: Clone> Sync for SmartPointerSendable<T> {}

#[derive(Debug, Clone)]
pub struct SmartPointerSendableComp<T: Clone> {
    ptr: T,
    _drop_guard: Arc<dyn ObsDropGuard>,
}

impl<T> Hash for SmartPointerSendableComp<T>
where
    T: Clone + Hash,
{
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ptr.hash(state);
    }
}

impl<T> PartialEq for SmartPointerSendableComp<T>
where
    T: Clone + PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.ptr == other.ptr
    }
}

impl<T> Eq for SmartPointerSendableComp<T> where T: Clone + Eq {}

impl<T: Clone> SmartPointerSendableComp<T> {
    pub fn new(ptr: T, drop_guard: Arc<dyn ObsDropGuard>) -> Self {
        Self {
            ptr,
            _drop_guard: drop_guard,
        }
    }

    pub fn get_ptr(&self) -> T {
        self.ptr.clone()
    }
}

unsafe impl<T: Clone> Send for SmartPointerSendableComp<T> {}
unsafe impl<T: Clone> Sync for SmartPointerSendableComp<T> {}
