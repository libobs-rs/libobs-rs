fn main() {
    // On Windows this lets the executable reach main() without obs.dll so the
    // explicit bootstrap call can provision/update OBS before the first symbol
    // from the DLL is used. It is a no-op on other platforms.
    libobs_bootstrapper::build::emit_windows_obs_delay_load();
}
