# Creating Custom Sources

`libobs-rs` provides a powerful macro system, `libobs-simple-macro`, to simplify the creation of custom OBS sources. This system handles the boilerplate of property management, settings updates, and C-API interaction.

## The `obs_object_builder` Macro

This macro generates a builder struct for your source. It allows you to define properties that map directly to OBS settings.

### Supported Types (`type_t`)

- `string`: Maps to `ObsString`.
- `bool`: Maps to `bool`.
- `int`: Maps to `i64`.
- `enum`: Maps to a C-style enum (requires `num_derive`).
- `enum_string`: Maps to a string-based enum (requires `StringEnum` trait).

### Example: Defining a Source

```rust
use libobs_simple_macro::obs_object_builder;
use libobs_wrapper::data::ObsObjectBuilder;

#[derive(Debug)]
#[obs_object_builder("my_custom_source_id")]
pub struct MySourceBuilder {
    /// This doc comment will appear on the builder method.
    #[obs_property(type_t = "string")]
    pub url: String,

    #[obs_property(type_t = "bool", settings_key = "is_visible")]
    pub visible: bool,
    
    #[obs_property(type_t = "int")]
    pub width: i64,
}
```

This will generate a `MySourceBuilder` struct with methods like `set_url`, `set_visible`, and `set_width`.

## The `obs_object_updater` Macro

This macro generates an updater struct, allowing you to modify the settings of an existing source at runtime.

The macro takes three positional arguments: the OBS object id, the wrapper type being updated, and
the underlying native pointer type of that wrapper.

```rust
use libobs_simple_macro::obs_object_updater;
use libobs_wrapper::sources::ObsSourceRef;

#[obs_object_updater("my_custom_source_id", ObsSourceRef, *mut libobs::obs_source_t)]
pub struct MySourceUpdater {
    #[obs_property(type_t = "string")]
    pub url: String,
    
    // ... same properties as builder ...
}
```

## The `obs_object_impl` Macro

This macro ties everything together: the methods written in `impl MySource` are generated on both
`MySourceBuilder` and `MySourceUpdater` (the `MySource` name is only used as the shared prefix).

```rust
use libobs_simple_macro::obs_object_impl;

#[obs_object_impl]
impl MySource {
    // You can add custom helper methods here
}
```

## Real-World Example

For a complete, real-world example, look at the `WindowCaptureSource` implementation in `libobs-simple/src/sources/windows/sources/window_capture.rs`. It demonstrates:

- Using enums for properties (`ObsWindowCaptureMethod`).
- Handling complex settings updates.
- Integrating with the `ObsSourceBuilder` trait.
