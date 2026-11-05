# libOBS simple macro

This is a helper macro for the [libobs-simple crate](https://crates.io/crates/libobs-simple).

## Usage
This is an example for the window_capture source:
```rust
#[obs_object_builder("window_capture")]
pub struct WindowCaptureSourceBuilder {
    #[obs_property(type_t = "enum")]
    capture_method: ObsWindowCaptureMethod,

    // This attribute has to be on each field that should change any obs data setting.
    // notice the `settings_key` attribute, which sets the key that should be used when setting obs data (so data.set_string("window", 'your_window') would be called). Otherwise defaults to the field name.
    // The `type_t` attribute is the type that should be used in the obs data. This is used to generate the correct obs data setter.
    // Can be enum,enum_string,int,bool,string
    #[obs_property(type_t = "string", settings_key = "window")]
    window_raw: String,

    #[obs_property(type_t = "bool")]
    cursor: bool,
}
```

For more examples look at
the [libobs-simple crate](https://github.com/libobs-rs/libobs-rs/tree/main/libobs-simple).