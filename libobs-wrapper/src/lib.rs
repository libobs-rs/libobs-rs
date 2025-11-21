#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

use getters0::Getters;

pub mod context;
pub mod crash_handler;
pub mod data;
pub mod display;
pub mod encoders;
pub mod logger;
pub mod runtime;
pub mod scenes;
pub mod signals;
pub mod sources;
pub mod unsafe_send;

pub mod enums;
pub mod utils;

// Add the macros module to the public exports
#[cfg_attr(coverage_nightly, coverage(off))]
mod macros;

#[derive(Debug, Clone, Copy, Getters)]
pub struct Vec2 {
    #[get_mut]
    x: f32,
    #[get_mut]
    y: f32,
}

impl From<libobs::vec2> for Vec2 {
    fn from(raw: libobs::vec2) -> Self {
        let inner = unsafe { raw.__bindgen_anon_1.__bindgen_anon_1 };
        Self {
            x: inner.x,
            y: inner.y,
        }
    }
}

impl From<Vec2> for libobs::vec2 {
    fn from(val: Vec2) -> Self {
        libobs::vec2 {
            __bindgen_anon_1: libobs::vec2__bindgen_ty_1 {
                __bindgen_anon_1: libobs::vec2__bindgen_ty_1__bindgen_ty_1 { x: val.x, y: val.y },
            },
        }
    }
}

#[test]
fn test_vec2() {
    let vec_val = Vec2::new(1.0, 2.0);
    let libobs_vec: libobs::vec2 = vec_val.into();

    let original = Vec2::from(libobs_vec);
    assert_eq!(original.x, 1.0);
    assert_eq!(original.y, 2.0);
    assert_ne!(original.x, 0.0);
    assert_ne!(original.y, 0.0);
}

#[test]
fn test_vec2_new() {
    let vec = Vec2::new(3.5, 4.5);
    assert_eq!(vec.x, 3.5);
    assert_eq!(vec.y, 4.5);
}

#[test]
fn test_vec2_clone() {
    let vec1 = Vec2::new(1.0, 2.0);
    #[allow(clippy::clone_on_copy)]
    let vec2 = vec1.clone();
    assert_eq!(vec1.x, vec2.x);
    assert_eq!(vec1.y, vec2.y);
}

#[test]
fn test_vec2_copy() {
    let vec1 = Vec2::new(1.0, 2.0);
    let vec2 = vec1; // Copy, not move
    assert_eq!(vec1.x, vec2.x);
    assert_eq!(vec1.y, vec2.y);
}

#[test]
fn test_vec2_debug() {
    let vec = Vec2::new(1.0, 2.0);
    let debug_str = format!("{:?}", vec);
    assert!(debug_str.contains("Vec2"));
}
