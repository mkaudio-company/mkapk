# Rust Plugin GUI Framework - Learnings

## Task 2: gui-core primitives

- Implemented `geometry`, `transform`, `color`, and `units` modules in `crates/gui-core`.
- Crate remains `#![no_std]` and `#![deny(unsafe_code)]` with no external dependencies.
- Key no_std considerations:
  - `f32::round`, `f32::sin`, `f32::cos` are not available in `core`; implemented manual rounding and a Taylor-series sin/cos approximation for `Transform::rotate`.
  - `f32::clamp` clippy lint had to be allowed because the method is also unavailable in no_std.
- Added 22 unit tests covering geometry operations, transform composition, color conversion, and Dp/Px roundtrip.
- Verification: `cargo test -p gui-core` passes, `cargo clippy -p gui-core -- -D warnings` passes.

