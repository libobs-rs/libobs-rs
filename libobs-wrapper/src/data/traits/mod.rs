mod getters;
mod setters;

pub use getters::*;
use libobs::obs_data;
pub use setters::*;

use crate::{runtime::ObsRuntime, unsafe_send::SmartPointerSendable};

pub trait ObsDataPointers {
    fn runtime(&self) -> &ObsRuntime;
    fn as_ptr(&self) -> SmartPointerSendable<*mut obs_data>;
}
