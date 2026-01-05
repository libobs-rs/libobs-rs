use libobs_wrapper::{
    data::{object::ObsObjectTrait, ImmutableObsData, ObsData},
    sources::{ObsFilterGuardPair, ObsFilterRef, ObsSourceSignals, ObsSourceTrait},
    unsafe_send::SmartPointerSendable,
    utils::ObsError,
};

#[derive(Debug, Clone)]
pub enum ObsEitherSource<A: ObsSourceTrait + Clone + 'static, B: ObsSourceTrait + Clone + 'static> {
    Left(A),
    Right(B),
}

impl<A, B> libobs_wrapper::data::object::ObsObjectTraitPrivate for ObsEitherSource<A, B>
where
    A: ObsSourceTrait + Clone + 'static,
    B: ObsSourceTrait + Clone + 'static,
{
    fn __internal_replace_settings(
        &self,
        settings: libobs_wrapper::data::ImmutableObsData,
    ) -> Result<(), ObsError> {
        match self {
            ObsEitherSource::Left(a) => a.__internal_replace_settings(settings),
            ObsEitherSource::Right(b) => b.__internal_replace_settings(settings),
        }
    }

    fn __internal_replace_hotkey_data(
        &self,
        hotkey_data: libobs_wrapper::data::ImmutableObsData,
    ) -> Result<(), ObsError> {
        match self {
            ObsEitherSource::Left(a) => a.__internal_replace_hotkey_data(hotkey_data),
            ObsEitherSource::Right(b) => b.__internal_replace_hotkey_data(hotkey_data),
        }
    }
}

impl<A, B> ObsObjectTrait<*mut libobs::obs_source> for ObsEitherSource<A, B>
where
    A: ObsSourceTrait + Clone + 'static,
    B: ObsSourceTrait + Clone + 'static,
{
    fn runtime(&self) -> &libobs_wrapper::runtime::ObsRuntime {
        match self {
            ObsEitherSource::Left(a) => a.runtime(),
            ObsEitherSource::Right(b) => b.runtime(),
        }
    }

    fn settings(&self) -> Result<ImmutableObsData, ObsError> {
        match self {
            ObsEitherSource::Left(a) => a.settings(),
            ObsEitherSource::Right(b) => b.settings(),
        }
    }

    fn hotkey_data(&self) -> Result<ImmutableObsData, ObsError> {
        match self {
            ObsEitherSource::Left(a) => a.hotkey_data(),
            ObsEitherSource::Right(b) => b.hotkey_data(),
        }
    }

    fn id(&self) -> libobs_wrapper::utils::ObsString {
        match self {
            ObsEitherSource::Left(a) => a.id(),
            ObsEitherSource::Right(b) => b.id(),
        }
    }

    fn name(&self) -> libobs_wrapper::utils::ObsString {
        match self {
            ObsEitherSource::Left(a) => a.name(),
            ObsEitherSource::Right(b) => b.name(),
        }
    }

    fn update_settings(&self, settings: ObsData) -> Result<(), ObsError> {
        match self {
            ObsEitherSource::Left(a) => a.update_settings(settings),
            ObsEitherSource::Right(b) => b.update_settings(settings),
        }
    }

    fn as_ptr(&self) -> SmartPointerSendable<*mut libobs::obs_source> {
        match self {
            ObsEitherSource::Left(a) => a.as_ptr(),
            ObsEitherSource::Right(b) => b.as_ptr(),
        }
    }
}

impl<A, B> ObsSourceTrait for ObsEitherSource<A, B>
where
    A: ObsSourceTrait + Clone + 'static,
    B: ObsSourceTrait + Clone + 'static,
{
    fn signals(&self) -> &std::sync::Arc<ObsSourceSignals> {
        match self {
            ObsEitherSource::Left(a) => a.signals(),
            ObsEitherSource::Right(b) => b.signals(),
        }
    }

    fn get_active_filters(&self) -> Result<Vec<ObsFilterGuardPair>, ObsError> {
        match self {
            ObsEitherSource::Left(a) => a.get_active_filters(),
            ObsEitherSource::Right(b) => b.get_active_filters(),
        }
    }

    fn apply_filter(&self, filter: &ObsFilterRef) -> Result<(), ObsError> {
        match self {
            ObsEitherSource::Left(a) => a.apply_filter(filter),
            ObsEitherSource::Right(b) => b.apply_filter(filter),
        }
    }
}

pub enum ObsEither<A, B> {
    Left(A),
    Right(B),
}
