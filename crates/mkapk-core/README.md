# mkapk-core

Platform-agnostic GUI core for [mkapk](https://github.com/mkaudio-company/mkapk): geometry, color, and math types, a paint command list, the retained widget tree, box/flex layout, input events, an animation engine, and accessibility metadata.

`#![no_std]` and `#![deny(unsafe_code)]` — this crate contains no platform-specific code and no unsafe code; platform crates (`mkapk-win32`, `mkapk-mac`) translate its `PaintCommand`s into native rendering calls.

See the [workspace README](https://github.com/mkaudio-company/mkapk#readme) for the full picture of how this fits into building VST3/AU/AAX/Standalone plugins.
