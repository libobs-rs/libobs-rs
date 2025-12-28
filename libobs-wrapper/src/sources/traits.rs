use std::fmt::Debug;

use crate::data::object::ObsObjectTrait;

pub(crate) trait ObsSourceTraitSealed: Debug + Send + Sync {}

#[allow(private_bounds)]
pub trait ObsSourceTrait: ObsObjectTrait + ObsSourceTraitSealed {

}
