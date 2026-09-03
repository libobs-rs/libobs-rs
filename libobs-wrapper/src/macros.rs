#[macro_export]
macro_rules! run_with_obs_impl {
    ($runtime:expr, $operation:expr) => {
        $crate::run_with_obs_impl!($runtime, (), $operation)
    };
    ($runtime:expr, ($($var:ident),* $(,)*), $operation:expr) => {{
        $(let $var = $var.clone();)*
        $runtime.run_with_obs_result(move || {
            $(let $var = $var;)*
            let inner_obs_run = { $operation };
            inner_obs_run()
        })
    }};
}

/// Execute work on the OBS actor while preserving the runtime's structured error.
#[macro_export]
macro_rules! run_with_obs {
    ($runtime:expr, $operation:expr) => {{
        $crate::run_with_obs_impl!($runtime, $operation)
    }};
    ($runtime:expr, ($($var:ident),* $(,)*), $operation:expr) => {{
        $crate::run_with_obs_impl!($runtime, ($($var),*), $operation)
    }};
}

/// Implement a non-panicking native cleanup guard.
///
/// Cleanup is enqueued on the runtime's dedicated cleanup queue, so destructors do
/// not synchronously execute libobs calls or require a Tokio runtime.
#[macro_export]
macro_rules! impl_obs_drop {
    ($struct_name: ident, $operation:expr) => {
        $crate::impl_obs_drop!($struct_name, (), $operation);
    };
    ($struct_name: ident, ($($var:ident),* $(,)*), $operation:expr) => {
        impl Drop for $struct_name {
            fn drop(&mut self) {
                log::trace!("Dropping {}...", stringify!($struct_name));
                $(let $var = self.$var.clone();)*
                // Move one Send tuple rather than accessing wrapper fields across the
                // closure boundary. This defeats precise field capture of raw pointers
                // without introducing a generic "make anything Send" adapter.
                let cleanup_payload = ($($var,)*);
                let runtime = self.runtime.clone();
                let cleanup_runtime = runtime.clone();
                runtime.defer_obs_cleanup(move || {
                    // Keep the payload wrapped until it crosses the actor-call boundary so
                    // Rust's precise closure capture cannot extract a raw pointer field.
                    let result = cleanup_runtime.run_with_obs_result(move || {
                        let ($($var,)*) = cleanup_payload;
                        let inner_obs_drop = { $operation };
                        inner_obs_drop()
                    });
                    if let Err(err) = result {
                        log::error!(
                            "Failed to run native cleanup for {} on the OBS actor: {:?}",
                            stringify!($struct_name),
                            err
                        );
                    }
                });
            }
        }
    };
}

/// Implements PartialEq, Eq and Hash für a struct by comparing the inner pointer given by `as_ptr()`.
macro_rules! impl_eq_of_ptr {
    ($struct: ty) => {
        impl PartialEq for $struct {
            fn eq(&self, other: &Self) -> bool {
                self.as_ptr().native_id() == other.as_ptr().native_id()
            }
        }

        impl Eq for $struct {}

        impl std::hash::Hash for $struct {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.as_ptr().native_id().hash(state);
            }
        }
    };
}

/// Implements PartialEq, Eq and Hash for a managed OBS object using its opaque ID.
macro_rules! impl_eq_of_obs_object {
    ($struct: ty) => {
        impl PartialEq for $struct {
            fn eq(&self, other: &Self) -> bool {
                crate::data::object::ObsObjectTrait::object_id(self)
                    == crate::data::object::ObsObjectTrait::object_id(other)
            }
        }

        impl Eq for $struct {}

        impl std::hash::Hash for $struct {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                crate::data::object::ObsObjectTrait::object_id(self).hash(state);
            }
        }
    };
}

#[cfg(windows)]
macro_rules! enum_from_number {
    ($var: ident, $numb: expr) => {{
        use num_traits::FromPrimitive;
        $var::from_i32($numb)
    }};
}

#[cfg(not(windows))]
macro_rules! enum_from_number {
    ($var: ident, $numb: expr) => {{
        use num_traits::FromPrimitive;
        $var::from_u32($numb)
    }};
}

/// Defines a trait that conditionally includes Send + Sync bounds when the enable_runtime feature is enabled.
/// This avoids duplicating trait definitions for runtime vs non-runtime scenarios.
///
/// # Example
/// ```ignore
/// trait_with_optional_send_sync! {
///     #[doc(hidden)]
///     pub trait MyTrait: Debug {
///         fn my_method(&self);
///     }
/// }
/// ```
/// This expands to two trait definitions:
/// - With enable_runtime: `pub trait MyTrait: Debug + Send + Sync { ... }`
/// - Without enable_runtime: `pub trait MyTrait: Debug { ... }`
macro_rules! trait_with_optional_send_sync {
    (
        $(#[$meta:meta])*
        $vis:vis trait $trait_name:ident: $base_bound:path {
            $($body:tt)*
        }
    ) => {
        #[cfg(feature="enable_runtime")]
        $(#[$meta])*
        $vis trait $trait_name: $base_bound + Send + Sync {
            $($body)*
        }

        #[cfg(not(feature="enable_runtime"))]
        $(#[$meta])*
        $vis trait $trait_name: $base_bound {
            $($body)*
        }
    };
}

pub(crate) use enum_from_number;
pub(crate) use impl_eq_of_obs_object;
pub(crate) use impl_eq_of_ptr;
pub(crate) use trait_with_optional_send_sync;
