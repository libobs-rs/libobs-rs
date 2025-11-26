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
                let e = {
                    $(let $var = $var.0;)*
                    $operation
                };
                return e()
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
                    let r = $crate::run_with_obs!(self.runtime, ($($var),*), $operation);
                    if std::thread::panicking() {
                        return;
                    }

                    r.unwrap();
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
