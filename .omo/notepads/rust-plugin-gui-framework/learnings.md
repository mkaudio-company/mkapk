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

## Task 11: gui-vst3 `IPlugView` wrapper

- `vst3-sys` is not published on crates.io, so the dependency points to the `RustAudio/vst3-sys` git repository.
- Implemented `PluginView` using `#[VST3(implements(IPlugView))]`, exposing the `IPlugView` trait from `vst3_sys::gui`.
- The `vst3-sys` trait methods take `&self`, so the editor, frame, and size are stored with interior mutability (`RefCell` / `Cell`).
- Platform type support is gated with `#[cfg(target_os = ...)]` for `"HWND"` on Windows and `"NSView"` / `"Cocoa"` on macOS.
- `ViewHost` implements `EditorHost` and forwards `request_resize` to `IPlugFrame::resize_view` using a transmuted self-pointer for the view argument.
- Added a compile-only unit test verifying the default view size.
- Verification: `cargo build -p gui-vst3` exits 0, `cargo test -p gui-vst3` passes (1 test), `cargo clippy -p gui-vst3 -- -D warnings` passes.

## Task 7: gui-win32 Win32 windowing and surface management

- Implemented `Win32Window` in `crates/gui-win32/src/window.rs` wrapping an `HWND`.
- Updated `crates/gui-win32/Cargo.toml` to enable the required `windows` crate features (`Win32_Foundation`, `Win32_Graphics_Gdi`, `Win32_System_LibraryLoader`, `Win32_UI_HiDpi`, `Win32_UI_WindowsAndMessaging`).
- Set per-monitor DPI awareness via `SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` at the start of `create`, ignoring errors.
- Registered a `"GuiPluginEditorClass"` window class and created a child window with `WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN`.
- Stored `EditorWindowState` (editor, host, size) in `GWLP_USERDATA` and forwarded `WNDPROC` messages to it.
- Handled `WM_SIZE` (resize editor and repaint), `WM_DPICHANGED` (repaint), `WM_PAINT` (validate rect), `WM_DESTROY` (drop state and clear user data), plus `WM_LBUTTONDOWN`, `WM_LBUTTONUP`, `WM_MOUSEMOVE`, `WM_KEYDOWN`, `WM_KEYUP` with debug prints.
- Implemented `client_size` using `GetClientRect`, `dpi` using `GetDpiForWindow` with a 96 fallback, and `request_repaint` using `InvalidateRect` + `UpdateWindow`.
- Added `win32_window_exports` compile-only test.
- Verification: `cargo test -p gui-win32` passes (1 test). `cargo build --workspace` cannot currently be verified on macOS because `gui-vst3` depends on `vst3-sys`, which is not published on crates.io; building the remaining workspace (with `gui-vst3` temporarily excluded) passes.


