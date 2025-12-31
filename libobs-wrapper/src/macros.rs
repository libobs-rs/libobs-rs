#[macro_export]
macro_rules! run_with_obs_impl {
    ($runtime:expr, $operation:expr) => {
        $crate::run_with_obs_impl!($runtime, (), $operation)
    };
    ($runtime:expr, ($($var:ident),* $(,)*), $operation:expr) => {
        {
            $(let $var = $var.clone();)*
            $runtime.run_with_obs_result(move || {
                $(let $var = $var;)*
                let inner_obs_run = {
                    $(let $var = $var.0;)*
                    $operation
                };
                return inner_obs_run()
            })
        }
    };
    (SEPARATE_THREAD, $runtime:expr, ($($var:ident),* $(,)*), $operation:expr) => {
        {
            $(let $var = $var.clone();)*

            tokio::task::spawn_blocking(move || {
                $runtime.run_with_obs_result(move || {
                    $(let $var = $var;)*
                    let e = {
                        $(let $var = $var.0;)*
                        $operation
                    };
                    return e()
                }).unwrap()
            })
        }
    };
}

#[macro_export]
macro_rules! run_with_obs {
    ($runtime:expr, $operation:expr) => {
        {
            $crate::run_with_obs_impl!($runtime, $operation)
                .map_err(|e| $crate::utils::ObsError::InvocationError(e.to_string()))
        }
    };
    ($runtime:expr, ($($var:ident),* $(,)*), $operation:expr) => {
        {
            $crate::run_with_obs_impl!($runtime, ($($var),*), $operation)
                .map_err(|e| $crate::utils::ObsError::InvocationError(e.to_string()))
        }
    };
}

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
                #[cfg(any(not(feature = "no_blocking_drops"), test, feature="__test_environment"))]
                {
                    let run_with_obs_result = $crate::run_with_obs!(self.runtime, ($($var),*), $operation);
                    if std::thread::panicking() {
                        return;
                    }

                    run_with_obs_result.unwrap();
                }

                #[cfg(all(feature = "no_blocking_drops", not(test), not(feature="__test_environment")))]
                {
                    let __runtime = self.runtime.clone();
                    $crate::run_with_obs_impl!(SEPARATE_THREAD, __runtime, ($($var),*), $operation);
                }
            }
        }
    };
}

macro_rules! impl_eq_of_ptr {
    ($struct: ty, $ptr: ident) => {
        impl PartialEq for $struct {
            fn eq(&self, other: &Self) -> bool {
                self.$ptr.0 == other.$ptr.0
            }
        }

        impl Eq for $struct {}

        impl Hash for $struct {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.$ptr.0.hash(state);
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

pub(crate) use enum_from_number;
pub(crate) use impl_eq_of_ptr;
