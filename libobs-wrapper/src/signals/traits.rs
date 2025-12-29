use crate::runtime::ObsRuntime;

pub trait ObsSignalManagerTrait {
    fn runtime(&self) -> &ObsRuntime;
    //TODO add common signal manager methods here, so users can specify their own signals to listen to
}
