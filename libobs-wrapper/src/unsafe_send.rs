//! Sendable wrapper types for non-Send types
//!
//! This module provides wrapper types that allow non-Send types to be sent
//! across thread boundaries. Use with caution - these are unsafe by design.

use std::sync::Arc;

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
}

unsafe impl<T> Send for SmartPointerSendable<T> {}
unsafe impl<T> Sync for SmartPointerSendable<T> {}
