//! Typed OBS streaming-service handles.
//!
//! Services are ordinary runtime-affine native objects: creation and mutation happen on
//! the OBS actor, clones share one native lifetime lease, and final release is deferred
//! back to the actor.

use std::{
    ffi::CStr,
    sync::{Arc, RwLock},
};

use crate::{
    data::{
        object::{inner_fn_update_settings, ObsObjectTrait, ObsObjectTraitPrivate},
        ImmutableObsData, ObsDataPointers,
    },
    impl_obs_drop,
    macros::impl_eq_of_obs_object,
    run_with_obs,
    runtime::ObsRuntime,
    unsafe_send::{Sendable, SmartPointerSendable},
    utils::{ObjectInfo, ObsDropGuard, ObsError, ObsString},
};

#[derive(Debug)]
struct _ObsServiceDropGuard {
    service: Sendable<*mut libobs::obs_service_t>,
    runtime: ObsRuntime,
}

impl ObsDropGuard for _ObsServiceDropGuard {}

impl_obs_drop!(_ObsServiceDropGuard, (service), move || unsafe {
    // Safety: The guard owns the native service reference and cleanup runs on the OBS actor.
    libobs::obs_service_release(service.0);
});

/// A managed OBS service, such as an RTMP streaming-service configuration.
#[derive(Clone, Debug)]
pub struct ObsServiceRef {
    id: ObsString,
    name: ObsString,
    settings: Arc<RwLock<ImmutableObsData>>,
    hotkey_data: Arc<RwLock<ImmutableObsData>>,
    runtime: ObsRuntime,
    service: SmartPointerSendable<*mut libobs::obs_service_t>,
}

impl ObsServiceRef {
    /// Returns the protocol reported by this service instance (for example `RTMP`).
    pub fn protocol(&self) -> Result<Option<String>, ObsError> {
        let service = self.__native_handle();
        run_with_obs!(self.runtime, (service), move || {
            // Safety: the managed service handle remains alive for the actor call.
            let protocol = unsafe { libobs::obs_service_get_protocol(service.get_ptr()) };
            if protocol.is_null() {
                None
            } else {
                // Safety: libobs returns a borrowed NUL-terminated string for the service lifetime.
                Some(
                    unsafe { CStr::from_ptr(protocol) }
                        .to_string_lossy()
                        .into_owned(),
                )
            }
        })
    }

    pub(crate) fn new_from_info(
        info: ObjectInfo,
        runtime: ObsRuntime,
    ) -> Result<Arc<Self>, ObsError> {
        let ObjectInfo {
            id,
            name,
            settings,
            hotkey_data,
        } = info;

        let settings_ptr = settings.as_ref().map(ObsDataPointers::as_ptr);
        let hotkey_data_ptr = hotkey_data.as_ref().map(ObsDataPointers::as_ptr);

        let service = run_with_obs!(
            runtime,
            (id, name, settings_ptr, hotkey_data_ptr),
            move || {
                let settings_raw = settings_ptr
                    .as_ref()
                    .map_or(std::ptr::null_mut(), SmartPointerSendable::get_ptr);
                let hotkey_raw = hotkey_data_ptr
                    .as_ref()
                    .map_or(std::ptr::null_mut(), SmartPointerSendable::get_ptr);

                // Safety: IDs/names stay alive for the call and data handles keep their native
                // pointers alive while creation executes on the OBS actor.
                let ptr = unsafe {
                    libobs::obs_service_create(
                        id.as_ptr().0,
                        name.as_ptr().0,
                        settings_raw,
                        hotkey_raw,
                    )
                };
                if ptr.is_null() {
                    Err(ObsError::NullPointer(Some("OBS service creation".into())))
                } else {
                    Ok(Sendable(ptr))
                }
            }
        )??;

        let service = SmartPointerSendable::new(
            service.0,
            Arc::new(_ObsServiceDropGuard {
                service: service.clone(),
                runtime: runtime.clone(),
            }),
            runtime.native_registry(),
        );

        let current_settings = run_with_obs!(runtime, (service), move || {
            // Safety: the managed service handle remains alive for this actor call.
            let ptr = unsafe { libobs::obs_service_get_settings(service.get_ptr()) };
            if ptr.is_null() {
                Err(ObsError::NullPointer(Some("OBS service settings".into())))
            } else {
                Ok(Sendable(ptr))
            }
        })??;
        let settings = ImmutableObsData::from_raw_pointer(current_settings, runtime.clone());

        let hotkey_data = match hotkey_data {
            Some(data) => data,
            None => ImmutableObsData::new(&runtime)?,
        };

        Ok(Arc::new(Self {
            id,
            name,
            settings: Arc::new(RwLock::new(settings)),
            hotkey_data: Arc::new(RwLock::new(hotkey_data)),
            runtime,
            service,
        }))
    }
}

impl ObsObjectTraitPrivate for ObsServiceRef {
    fn __internal_replace_settings(&self, settings: ImmutableObsData) -> Result<(), ObsError> {
        *self
            .settings
            .write()
            .map_err(|_| ObsError::LockError("Failed to acquire service settings lock".into()))? =
            settings;
        Ok(())
    }

    fn __internal_replace_hotkey_data(
        &self,
        hotkey_data: ImmutableObsData,
    ) -> Result<(), ObsError> {
        *self
            .hotkey_data
            .write()
            .map_err(|_| ObsError::LockError("Failed to acquire service hotkey lock".into()))? =
            hotkey_data;
        Ok(())
    }
}

impl ObsObjectTrait for ObsServiceRef {
    type Native = *mut libobs::obs_service_t;

    fn runtime(&self) -> &ObsRuntime {
        &self.runtime
    }

    fn settings(&self) -> Result<ImmutableObsData, ObsError> {
        self.settings
            .read()
            .map_err(|_| ObsError::LockError("Failed to acquire service settings lock".into()))
            .map(|settings| settings.clone())
    }

    fn hotkey_data(&self) -> Result<ImmutableObsData, ObsError> {
        self.hotkey_data
            .read()
            .map_err(|_| ObsError::LockError("Failed to acquire service hotkey lock".into()))
            .map(|data| data.clone())
    }

    fn id(&self) -> ObsString {
        self.id.clone()
    }

    fn name(&self) -> ObsString {
        self.name.clone()
    }

    fn update_settings(&self, settings: crate::data::ObsData) -> Result<(), ObsError> {
        inner_fn_update_settings!(self, libobs::obs_service_update, settings)
    }

    fn __native_handle(&self) -> SmartPointerSendable<Self::Native> {
        self.service.clone()
    }
}

impl_eq_of_obs_object!(ObsServiceRef);
