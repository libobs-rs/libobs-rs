#![cfg_attr(doc, feature(doc_cfg))]
//! A simplified interface for recording and streaming with libobs

pub mod error;
pub mod output;
pub mod sources;

pub use error::ObsSimpleError;
pub use libobs_wrapper as wrapper;
