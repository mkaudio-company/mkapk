# Rust Plugin GUI Framework - Learnings

## Task 2: gui-core primitives

- Implemented `geometry`, `transform`, `color`, and `units` modules in `crates/gui-core`.
- Crate remains `#![no_std]` and `#![deny(unsafe_code)]` with no external dependencies.
- Key no_std considerations:
  - `f32::round`, `f32::sin`, `f32::cos` are not available in `core`; implemented manual rounding and a Taylor-series sin/cos approximation for `Transform::rotate`.
  - `f32::clamp` clippy lint had to be allowed because the method is also unavailable in no_std.
- Added 22 unit tests covering geometry operations, transform composition, color conversion, and Dp/Px roundtrip.
- Verification: `cargo test -p gui-core` passes, `cargo clippy -p gui-core -- -D warnings` passes.

## Task 6: xtask helpers and CI

- Implemented `xtask/src/main.rs` with subcommands: `test`, `check`, `bundle-vst3`, `bundle-au`, `bundle-aax`, `validate`.
- Added `.cargo/config.toml` alias so `cargo xtask` dispatches to the `xtask` crate.
- Created `.github/workflows/ci.yml` running on `windows-latest` and `macos-latest`, installing stable Rust and running `cargo test`, `cargo clippy`, and `cargo fmt --check`.
- `.gitignore` already contained required entries.
- Pre-existing formatting issues were fixed with `cargo fmt` so `cargo xtask check` exits 0.
- Verification: `cargo xtask test` exits 0, `cargo xtask check` exits 0, `cargo build --workspace` exits 0.

## Task 3: gui-host abstractions

- Replaced placeholder `crates/gui-host/src/lib.rs` with `editor` and `parameter` modules.
- Defined `PluginEditor`, `EditorHost`, `ParameterGateway`, `ParameterId`, `NormalizedValue`, `ParameterInfo`, `ParentWindowHandle`, and `SizeConstraints`.
- Kept crate `#![deny(unsafe_code)]`; raw pointer handle types are stored but never dereferenced here.
- `ParentWindowHandle` uses `*mut core::ffi::c_void` to stay platform-agnostic without platform-specific dependencies.
- `NormalizedValue` clamps to `[0.0, 1.0]` in `new()` and provides `new_unchecked()` for callers that already validate.
- Added mock lifecycle tests verifying `open`, `resize`, `idle`, and `close` are recorded, plus clamping tests for `NormalizedValue`.
- Verification: `cargo test -p gui-host` passes (5 tests), `cargo clippy -p gui-host -- -D warnings` passes.

## Task 4: gui-res resource IDs and embedded bundle

- Implemented `ResourceId` newtype around `u32` with `Copy`, `Eq`, `Hash`, `Debug`, and `Default`.
- Added `ResourceId::from_bytes_le` as a `const fn` using a 32-bit FNV-1a-like hash for development convenience (collisions possible; explicit IDs recommended for production).
- Defined `ResourceBundle` trait and `EmbeddedBundle` struct owning a static slice of `(ResourceId, \&'static [u8])` entries.
- Created `build.rs` that scans `crates/gui-res/resources/`, emits per-asset `include_bytes!` statics, and generates `src/generated.rs` with a `pub static EMBEDDED: EmbeddedBundle`.
- Added a minimal 1x1 PNG at `crates/gui-res/resources/test.png` and a roundtrip test in `src/bundle.rs`.
- Verification: `cargo test -p gui-res` passes, `cargo clippy -p gui-res -- -D warnings` passes.

## Task 5: gui-test-host DAW-less test host

- Refactored `gui-test-host` from a single binary into a library + binary crate, exposing `run_test_host(duration_ms, width, height)`.
- Implemented `BlankEditor` (prints lifecycle markers) and `TestHost` (empty `EditorHost` impl) in `src/lib.rs`.
- Added platform modules under `src/platform/`:
  - `win32.rs`: registers a window class, creates an overlapped window, pumps via `PeekMessageW`, returns `ParentWindowHandle::Windows(hwnd)`.
  - `mac.rs`: creates an `NSApplication`, `NSWindow`, and `NSView`, pumps via `nextEventMatchingMask:untilDate:inMode:dequeue:`, returns `ParentWindowHandle::Mac(view)`.
- `src/main.rs` parses `--duration-ms`, `--width`, `--height` and delegates to `run_test_host`.
- Added `examples/blank.rs` as a thin wrapper that runs the test host with default parameters.
- Added unit tests for `BlankEditor` lifecycle and default size constraints.
- Notes:
  - The `cocoa` crate's `base::class` helper is gone in 0.26.1; use `objc::runtime::Class::get` directly.
  - `objc` macros need `#[macro_use] extern crate objc;` at the crate root (gated to macOS).
  - `NSEventMask::NSAnyEventMask.bits()` is a method, not a field, in this `bitflags` version.
- Verification: `cargo build --workspace` passes, `cargo test -p gui-test-host` passes (2 tests), and `cargo run -p gui-test-host --example blank -- --duration-ms 500` opens a window and prints `EditorAttached`/`EditorDetached` markers.


