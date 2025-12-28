mod getters;
mod setters;

use libobs::obs_data;
pub use setters::*;
pub use getters::*;

use crate::{runtime::ObsRuntime, unsafe_send::Sendable};


pub trait ObsDataPointers {
    fn runtime(&self) -> &ObsRuntime;
    fn as_ptr(&self) -> Sendable<*mut obs_data>;
}