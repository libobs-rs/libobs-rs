# libobs-bootstrapper

`libobs-bootstrapper` keeps local OBS-installation inspection helpers and the
legacy bootstrap API surface, but **runtime OBS download/install is disabled**.
Every `ObsBootstrapper::bootstrap*` entry point returns
`ObsBootstrapError::RuntimeBootstrapDisabled` before network, extraction, or
process-spawn work can begin.

This is intentional provenance hardening. An application must authenticate and
package its OBS runtime before process startup instead of downloading a mutable
"latest compatible" release and executing it in-process.

Use [`cargo-obs-build`](../cargo-obs-build/README.md) at build/package time on
Windows and macOS. On Linux, use the distro/system `libobs` installation or a
source integration appropriate for the target distribution.

`ObsBootstrapperOptions::set_install_dir` remains useful with the local
inspection helpers:

```rust
use libobs_bootstrapper::{ObsBootstrapper, ObsBootstrapperOptions};

let options = ObsBootstrapperOptions::new().set_install_dir("/opt/my-app/obs");
let valid = ObsBootstrapper::is_valid_installation_with_options(&options)?;
let needs_update = ObsBootstrapper::is_update_available_with_options(&options)?;
# Ok::<(), libobs_bootstrapper::ObsBootstrapError>(())
```

`is_update_available_with_options` is a local comparison against the libobs ABI
version used to generate this crate. It does not contact GitHub or another
release server.

The repository/update/restart option setters are retained temporarily for source
compatibility, but they cannot re-enable runtime network installation.
