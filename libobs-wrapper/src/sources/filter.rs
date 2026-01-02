use libobs::obs_source_t;

use crate::{
    data::ImmutableObsData,
    forward_obs_object_impl, forward_obs_source_impl, impl_obs_drop,
    macros::impl_eq_of_ptr,
    runtime::ObsRuntime,
    sources::ObsSourceRef,
    unsafe_send::SmartPointerSendable,
    utils::{ObsDropGuard, ObsError, ObsString},
};

#[derive(Debug, Clone)]
/// This is essentially the same as the OsSourceRef but we want to make sure
/// the dev doesn't confuse a filter with a source.
pub struct ObsFilterRef {
    inner: ObsSourceRef,
}

impl ObsFilterRef {
    pub fn new<T: Into<ObsString> + Sync + Send, K: Into<ObsString> + Sync + Send>(
        id: T,
        name: K,
        settings: Option<ImmutableObsData>,
        hotkey_data: Option<ImmutableObsData>,
        runtime: ObsRuntime,
    ) -> Result<Self, ObsError> {
        let inner = ObsSourceRef::new(id, name, settings, hotkey_data, runtime)?;
        Ok(Self { inner })
    }
}

#[derive(Debug)]
pub(crate) struct _ObsRemoveFilterOnDrop<K> {
    source: SmartPointerSendable<*mut obs_source_t>,
    filter: SmartPointerSendable<*mut obs_source_t>,
    additional: K,
    runtime: ObsRuntime,
}

impl <K> _ObsRemoveFilterOnDrop<K> {
    pub fn new(
        source: SmartPointerSendable<*mut obs_source_t>,
        filter: SmartPointerSendable<*mut obs_source_t>,
        additional: K,
        runtime: ObsRuntime,
    ) -> Self {
        Self {
            source,
            filter,
            additional,
            runtime,
        }
    }
}

impl ObsDropGuard for _ObsRemoveFilterOnDrop {}
impl_obs_drop!(_ObsRemoveFilterOnDrop, (source, filter), move || unsafe {
    // Safety: This is safe because both pointers still exist because of the SmartPointers.
    libobs::obs_source_filter_remove(source.get_ptr(), filter.get_ptr());
});

forward_obs_object_impl!(ObsFilterRef, inner, *mut libobs::obs_source_t);
forward_obs_source_impl!(ObsFilterRef, inner);

impl_eq_of_ptr!(ObsFilterRef);
