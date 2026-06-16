# Rust Plugin GUI Framework - Learnings
## Task 13: gui-core retained widget tree and lifecycle

- Created `crates/gui-core/src/widget.rs` with `WidgetId` newtype around `u64`, `Widget` trait with lifecycle defaults, and `LayoutConstraints` placeholder.
- Created `crates/gui-core/src/tree.rs` with `Tree` and `Node` storing `Box<dyn Widget>` plus parent/child links via `WidgetId`, supporting insert/remove with mount/unmount, find, root, children, and pre/post/breadth-first traversal.
- Used `core::sync::atomic::AtomicU64` for ID generation, keeping the crate `#![no_std]` and `#![deny(unsafe_code)]`.
- Added 4 tree unit tests covering insert/remove links, recursive unmount, traversal orders, and ID uniqueness.
- Verification: `cargo test -p gui-core` passes (28 tests), `cargo clippy -p gui-core -- -D warnings` passes.


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

## Task 10: gui-core zero-allocation paint command list

- Added `crates/gui-core/src/paint.rs` with `PaintCommand`, `CommandList`, `RenderBackend`, `ImageId`, `TextLayoutId`, and `ColorStop`.
- `CommandList` is backed by `alloc::vec::Vec<PaintCommand>` and retains allocated capacity across `clear()` calls for zero-allocation frame reuse.
- Path points and gradient stops use `&'static [Pointf]` / `&'static [ColorStop]` to stay allocation-free and `#![no_std]` compatible.
- `RenderBackend` trait provides default empty bodies so it is object-safe and simple for `gui-mac`/`gui-win32` to implement later.
- Added integration tests in `crates/gui-core/tests/paint_command.rs` exercising 10,000 pushes/clears and iteration.
- Verification: `cargo test -p gui-core` passes (24 tests), `cargo clippy -p gui-core -- -D warnings` passes.



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

## Task 22: gui-au AUv2 Cocoa UI wrapper

- Implemented `AuEditor` in `crates/gui-au/src/editor.rs` wrapping a `Box<dyn PluginEditor>`.
- Created a dynamic `NSView` subclass (`GuiAuView`) using `objc::declare::ClassDecl`.
- Overrode `initWithFrame:` to open the editor via `ParentWindowHandle::Mac`, `setFrameSize:` to call `editor.resize`, and `dealloc` to call `editor.close` and drop the editor/host state.
- Stored the editor/host state in an ivar (`_auState`) on the view.
- Used a thread-local pending state to pass the editor into `initWithFrame:` (v0.1 simplification).
- Defined `AudioComponentDescription` and `AuCocoaViewInfo` for the `kAudioUnitProperty_CocoaUI` response; `get_cocoa_view_info()` returns the registered view class.
- Gated the crate on an `au` feature (enabled by default) and macOS-specific Objective-C dependencies under `cfg(target_os = "macos")`.
- Added a non-macOS stub so the API exists on other platforms but returns null/zero-filled values.
- Added compile-only `au_editor_exports` test.
- Verification: `cargo build -p gui-au` exits 0, `cargo test -p gui-au` passes (1 test), and `cargo build -p gui-au --no-default-features` exits 0.

## Task 20: gui-mac CoreGraphics render backend

- Added `crates/gui-mac/src/render.rs` with `CoreGraphicsRenderBackend` implementing `gui_core::RenderBackend`.
- `NSGraphicsContext.currentContext` exposes its CG context via the `CGContext` selector (not the legacy `graphicsPort`).
- Use `core_graphics::context::CGContext::from_existing_context_ptr` to retain the borrowed CG context before storing it; release happens automatically on drop.
- CoreGraphics uses a bottom-left origin, so the backend flips the Y axis in `begin()` to match the framework's top-left coordinate convention.
- `core-graphics` 0.24 does not bind `CGPathCreateWithRoundedRect`; declare it manually with `#[link(name = "CoreGraphics", kind = "framework")]` and wrap the result in `core_graphics::path::CGPath`.
- Linear gradients need a color space (`CGColorSpace::create_device_rgb()`) and component/location arrays; use `ctx.save()/restore()` around `clip_to_rect` so gradient clipping does not leak to later commands.
- `DrawImage` and `DrawText` are left as no-ops for v0.1.
- Added a non-macOS stub so `gui-mac` still compiles on other platforms.
- Verification: `cargo build -p gui-mac`, `cargo test -p gui-mac`, `cargo run -p gui-test-host --example cg-rect -- --duration-ms 1000`, and `cargo clippy -p gui-mac -p gui-test-host -- -D warnings` all pass.

## Task 21: gui-mac CoreText text rendering

- Added `TextLayout` wrapping a `CTFont` and `CTLine`, with `size`, `baseline`, and `draw` methods.
- `core-text` 20.1 depends on `core-graphics` 0.23 / `core-foundation` 0.9, which conflicts with the workspace's `core-graphics` 0.24 / `core-foundation` 0.10. Used `core-text` 21.0 instead for compatibility.
- `core-foundation` 0.10's `CFAttributedString::new` no longer accepts attributes; use `CFMutableAttributedString`, `replace_str`, and `set_attribute` with `kCTFontAttributeName`.
- `kCTFontAttributeName` is an `extern static`, so it needs an `unsafe` block to read.
- `CTLine::draw` takes `&CGContext` (owned), not `&CGContextRef`; pass the context directly.
- CoreText draws from the baseline. With the backend's flipped Y axis, place the text position at `top_left.y + ascent`.
- Added `render_text_to_view` to lock focus, fetch the current `NSGraphicsContext` CG context, and call `TextLayout::draw` directly.
- Verification: `cargo build -p gui-mac`, `cargo test -p gui-mac`, `cargo run -p gui-test-host --example cg-text -- --duration-ms 1000`, and `cargo clippy -p gui-mac -p gui-test-host -- -D warnings` all pass.

## Task 23: gui-au DAW-less test host example

- Created `crates/gui-au/examples/gain.rs` implementing a minimal `PluginEditor` that renders a dark clear using `gui_mac::render_to_view` inside the test host window.
- Added `gui-mac` and `gui-test-host` as dev-dependencies for `gui-au`; also added `gui-test-host` to the workspace root `[workspace.dependencies]` so `workspace = true` resolves.
- Gated the macOS-specific rendering path with `#[cfg(target_os = "macos")]`; non-macOS builds print a message and exit.
- Extended the `au_editor_exports` compile-only test to exercise `AuEditor::new`.
- Verification: `cargo build -p gui-au --example gain`, `cargo run -p gui-au --example gain -- --test-host --duration-ms 1000` (prints `EditorAttached`/`EditorDetached`), and `cargo clippy -p gui-au -p gui-test-host -- -D warnings` all pass.

## Task 13: gui-core retained widget tree and lifecycle

- Created `crates/gui-core/src/widget.rs` with `WidgetId` newtype around `u64`, `Widget` trait with lifecycle defaults, and `LayoutConstraints` placeholder.
- Created `crates/gui-core/src/tree.rs` with `Tree` and `Node` storing `Box<dyn Widget>` plus parent/child links via `WidgetId`, supporting insert/remove with mount/unmount, find, root, children, and pre/post/breadth-first traversal.
- Used `core::sync::atomic::AtomicU64` for ID generation, keeping the crate `#![no_std]` and `#![deny(unsafe_code)]`.
- Added 4 tree unit tests covering insert/remove links, recursive unmount, traversal orders, and ID uniqueness.
- Verification: `cargo test -p gui-core` passes (28 tests), `cargo clippy -p gui-core -- -D warnings` passes.



## Task: gui-core layout engine wired

- Wired `crates/gui-core/src/layout.rs` into `gui-core` by adding `pub mod layout` and re-exporting `Alignment`, `LayoutBox`, `LayoutDirection`, `LayoutEngine`, `LayoutNode`, and `LayoutResult` from `crates/gui-core/src/lib.rs`.
- Kept `#![no_std]` and `#![deny(unsafe_code)]` intact.
- Verification: `cargo test -p gui-core` passes (33 tests), `cargo clippy -p gui-core -- -D warnings` passes.

## Task 15: gui-core mouse and keyboard event routing

- Created `crates/gui-core/src/event.rs` with platform-agnostic input types (`MouseButton`, `KeyCode`, `Modifiers`), event structs (`MouseEvent`, `KeyEvent`, `PointerEvent`), the `Event` enum, and `EventResponse` (`Handled` / `Bubble`).
- Extended the `Widget` trait in `crates/gui-core/src/widget.rs` with optional `on_mouse_down`, `on_mouse_up`, `on_mouse_move`, `on_key_down`, and `on_key_up` handlers defaulting to `Bubble`.
- Implemented `EventDispatcher` with hit-testing against `LayoutResult` boxes (deepest widget first), bubbling up the parent chain, and mouse capture support.
- Wired `Tree` widgets through `RefCell<Box<dyn Widget>>` so `EventDispatcher` can mutate widget state while holding an immutable `&Tree` reference, keeping dispatch safe without `unsafe`.
- Updated `LayoutEngine::measure` to borrow widget references from the `RefCell`.
- Added 5 unit tests covering deepest-leaf hit-testing, outside-hit bubble, handled-event stop, keyboard dispatch to root, and mouse capture redirection.
- Verification: `cargo test -p gui-core` passes (36 unit + 2 integration tests), `cargo clippy -p gui-core -- -D warnings` passes, `cargo fmt -p gui-core` applied.

## Task 16: gui-host lock-free parameter gateway

- Extended `crates/gui-host/src/parameter.rs` with `ParameterMessage` and `LockFreeParameterGateway`.
- Used `crossbeam-channel` bounded channels for both `ui_to_audio` and `audio_to_ui` directions, with a default capacity of 256.
- Implemented `ParameterGateway` for `LockFreeParameterGateway`; UI-side sends are non-blocking (`try_send`) so the audio thread is never blocked.
- Added `poll_ui_changes`, `poll_audio_changes`, and `send_from_audio` for queue draining and audio-thread pushes.
- Cached current values in a `Mutex<BTreeMap<ParameterId, NormalizedValue>>` for `get_normalized` reads.
- Added `Ord`/`PartialOrd` to `ParameterId` so it can be used as a `BTreeMap` key.
- Added 4 unit tests covering UI→audio ordering, audio→UI draining, latest value reads, and bounded-channel backpressure (`TrySendError::Full`).
- Verification: `cargo test -p gui-host` passes (9 tests), `cargo clippy -p gui-host -- -D warnings` passes, `cargo fmt -p gui-host` applied.
