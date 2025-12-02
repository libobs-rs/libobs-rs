# libOBS Window Helper

This is just a helper crate for the `libobs-simple` crate. It provides a way to get a list of all windows that OBS can
capture (eiter `window_capture` or `game_capture`). If you want to use this crate nevertheless, here's an example.

## Example

```rust
use libobs_window_helper::{get_all_windows, WindowSearchMode};

fn main() {
    let res = get_all_windows(WindowSearchMode::ExcludeMinimized, false).unwrap();
        for i in res {
            /// This struct contains all crucial information about the window like title, class name, obs_id etc.
            println!("{:?}", i);
        }
}
```

