mod getters;
mod setters;

pub use getters::*;
pub use setters::*;

use crate::{runtime::ObsRuntime, unsafe_send::SmartPointerSendable};

pub trait ObsDataPointers {
    fn runtime(&self) -> &ObsRuntime;
    fn as_ptr(&self) -> SmartPointerSendable<*mut libobs::obs_data_t>;
}
