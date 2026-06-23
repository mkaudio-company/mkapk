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

## Task 17: basic controls (slider, knob, button, label)

- Created new `crates/gui-widgets` crate, wired into the workspace root and `gui-test-host` dependencies.
- Crate is `#![deny(unsafe_code)]` and depends only on `gui-core` and `gui-host` (no platform-specific crates).
- Added `Theme` with dark defaults and implemented `Slider`, `Knob`, `Button`, and `Label` as `Widget` implementations.
- Each control stores a generated `WidgetId`, an optional change callback, and a `Cell<Rectf>` frame so the example can set layout boxes after measurement.
- `Slider` maps horizontal mouse position to a normalized value; `Knob` maps vertical position; `Button` triggers a click callback; `Label` emits `PaintCommand::DrawText` with `TextLayoutId(0)`.
- Added `gui_core::downcast_widget_ref`/`downcast_widget_mut` helpers after making `Widget: Any`, enabling the example editor to set frames on concrete control types stored in the retained tree.
- Created `crates/gui-test-host/examples/controls.rs` with a `ControlsEditor` that builds a tree, runs layout on resize, paints controls, and calls `gui_mac::render_to_view` each frame.
- Added 4 unit tests in `crates/gui-widgets/src/lib.rs` verifying each control constructs and implements `Widget`.
- Verification: `cargo test -p gui-widgets` passes (4 tests), `cargo run -p gui-test-host --example controls -- --duration-ms 1000` opens a window and exits cleanly, and `cargo clippy -p gui-widgets -p gui-test-host -- -D warnings` passes.

## Task 18: parameter-bound example plugin

- Extended `crates/gui-au/examples/gain.rs` to use the widget and parameter-binding infrastructure.
- Added `gui-widgets` as a dev-dependency for `gui-au`.
- `GainEditor` now owns a `Tree` (root `Panel`, `Label`, and `Slider` bound to `ParameterId(1)`), a `LayoutEngine`, a `LayoutResult`, and an `Arc<LockFreeParameterGateway>`.
- The slider's `on_changed` callback calls `gateway.set_normalized(id, value)` and prints the new value in dB (`20.0 * log10(value)`), guarding against zero.
- Layout is computed in `new` and recomputed on `resize`; frames are applied to concrete widget types via `downcast_widget_ref`.
- `idle` rebuilds paint commands from the tree and renders via `gui_mac::render_to_view`.
- Added a unit test in `crates/gui-au/src/editor.rs` that creates a `Slider`, wires its callback to a shared `Vec`, simulates a mouse down at x=50 inside a 100px-wide slider frame, and verifies the callback receives a normalized value between 0.4 and 0.6.
- The example is kept `#![deny(unsafe_code)]` by avoiding the macOS-specific backing-scale Objective-C call and using a fixed 1.0 scale factor.
- Verification: `cargo run -p gui-au --example gain -- --test-host --duration-ms 1000` prints `EditorAttached`/`EditorDetached`, `cargo test -p gui-au` passes (2 tests), and `cargo clippy -p gui-au -p gui-test-host -- -D warnings` passes.

## Task 24: gui-res typed resource registry and caching

- Created `crates/gui-res/src/registry.rs` with `ResourceHandle<T>` wrapping `std::sync::Arc<T>`, `Resource` trait, marker types `Image`, `Svg`, and `Font`, and `ResourceRegistry` caching typed decoded resources and raw bytes in `BTreeMap`s keyed by `ResourceId`.
- Added `ResourceRegistry::new`, `register_bytes`, `load`, `get`, and `evict`; `load` returns cached handles and decodes from raw bytes on first use.
- Added `EmbeddedBundle::register_with` to bulk-register embedded raw bytes into a registry.
- Re-exported `registry::{Resource, ResourceHandle, ResourceRegistry, Image, Svg, Font}` from `crates/gui-res/src/lib.rs`.
- Added `Ord`/`PartialOrd` to `ResourceId` so it can be used as a `BTreeMap` key.
- Added 4 unit tests covering raw-byte loading, cached Arc reuse, eviction, and embedded bundle registration.
## Task 28: gui-core accessibility metadata

- Created `crates/gui-core/src/accessibility.rs` with `Role`, bitflags-like `State`, `AccessibilityNode`, and `AccessibilityTree`, all using `alloc` types and staying `#![no_std]`.
- Implemented `Display` for `AccessibilityNode` (and `State`) so tests can serialize nodes without `serde`.
- Extended the `Widget` trait with a default `accessibility()` method returning an `AccessibilityNode` with `Role::None`.
- Added `Tree::accessibility_tree()` that mirrors the widget tree, applying bounds from an optional stored `LayoutResult` and falling back to zero bounds.
- Updated `Slider`, `Knob`, `Button`, and `Label` in `gui-widgets` to override `accessibility()` with their roles, labels, and values (slider value as a percentage).
- Added `Slider::set_label` and `Knob::set_label` so callers can provide accessibility labels without changing constructors.
- Added unit tests in `gui-core` (tree with panel/slider/label) and `gui-widgets` (per-control accessibility).
- Verification: `cargo test -p gui-core` passes (40 tests), `cargo test -p gui-widgets` passes (9 tests), `cargo clippy -p gui-core -p gui-widgets -- -D warnings` passes, `cargo fmt -p gui-core -p gui-widgets` applied.

## Task 25: gui-res SVG renderer

- Created `crates/gui-res/src/svg.rs` with `SvgImage`, storing a decoded `usvg::Tree` and an optional rasterized `tiny_skia::Pixmap` cache.
- Implemented `Resource` for `SvgImage` by parsing bytes as UTF-8 and calling `usvg::Tree::from_str` with default options.
- Added `tree`, `width`, `height`, `render`, and `render_rgba` methods; `render` caches the pixmap and reuses it when the same size is requested.
- Wired `pub mod svg` and re-exported `SvgImage` from `crates/gui-res/src/lib.rs`.
- Added `resvg = "0.47"`, `usvg = "0.47"`, and `tiny-skia = "0.12"` to `crates/gui-res/Cargo.toml` while keeping `#![deny(unsafe_code)]`.
- Added unit tests verifying intrinsic size, rendered RGBA byte length/non-zero alpha, and pixmap cache reuse.
- Verification: `cargo test -p gui-res` passes (8 tests), `cargo clippy -p gui-res -- -D warnings` passes, `cargo fmt -p gui-res` applied.

## Task 26: gui-res PNG renderer

- Created `crates/gui-res/src/png.rs` with `PngImage`, storing decoded width, height, and non-premultiplied RGBA pixels as `alloc::vec::Vec<u8>`.
- Implemented `Resource` for `PngImage` using `image::load_from_memory` and `DynamicImage::into_rgba8`, limiting the `image` crate to the `png` feature only.
- Added `width`, `height`, `rgba`, and `rgba_premultiplied` accessors; premultiplication scales each RGB channel by `alpha / 255`.
- Wired `pub mod png` and re-exported `PngImage` from `crates/gui-res/src/lib.rs`.
- Added `image = { version = "0.25", default-features = false, features = ["png"] }` to `crates/gui-res/Cargo.toml` while keeping `#![deny(unsafe_code)]`.
- Added unit tests verifying the embedded 1x1 red `test.png` decodes correctly, cached `Arc` reuse via `ResourceRegistry`, and premultiplied pixel math.
- Verification: `cargo test -p gui-res` passes, `cargo clippy -p gui-res -- -D warnings` passes, `cargo fmt -p gui-res` applied.

## Task 32: gui-aax AAX plugin wrapper

- Created `crates/gui-aax/build.rs` that reads `AAX_SDK` at build time. When set, it emits `cfg(aax_sdk)` and links `{AAX_SDK}/Libs`; otherwise it prints a cargo warning and the crate builds as a no-op stub.
- Updated `crates/gui-aax/Cargo.toml` with an optional `aax` feature (default off), `gui-core`/`gui-host` dependencies, and platform-gated dev-dependencies (`gui-mac` on macOS, `gui-win32` on Windows, plus `gui-test-host` and `gui-widgets`).
- Implemented `AaxEditor` in `crates/gui-aax/src/lib.rs` wrapping `Box<dyn PluginEditor>` and an optional `Box<dyn EditorHost>`, with lifecycle methods (`create_view`, `view_size`, `draw`, `timer_wakeup`, `set_parameter`, `destroy_view`) that delegate to the editor when `aax_sdk` is absent.
- Added `#![cfg_attr(not(aax_sdk), deny(unsafe_code))]` so unsafe code is only allowed when the real SDK is present.
- Created `crates/gui-aax/examples/gain.rs` reusing the `gui-au` example pattern: macOS runs through `gui-test-host` + `gui-mac::render_to_view`; Windows attempts `gui-win32::Win32Window::create` inside a test-host window and falls back to "AAX stub on Windows"; other platforms print a message and exit.
- Added compile-only `aax_editor_lifecycle` unit test constructing `AaxEditor` with a mock editor.
- Fixed a pre-existing `gui-core` `#![no_std]` compile issue by importing `alloc::string::ToString` in `crates/gui-core/src/accessibility.rs`.
- Verification: `cargo test -p gui-aax`, `cargo build -p gui-aax`, `cargo clippy -p gui-aax -- -D warnings`, `cargo run -p gui-aax --example gain -- --test-host --duration-ms 1000`, and `cargo fmt -p gui-aax` all pass.

## Task 33: gui-aax build-only example

- Converted `crates/gui-aax/examples/gain.rs` from a runtime test-host example into a build-only example that links against `AaxEditor`.
- Removed the macOS `gui-test-host`/`gui-mac` windowing path and the Windows `gui-win32`/`gui-test-host` windowing path.
- Reused the existing `GainEditor` from the file; its `idle` implementation now rebuilds the paint command list without calling a platform-specific render backend, so the example compiles on macOS and Windows without a DAW.
- Added a single `run` helper exercised by platform-gated `main` functions:
  - macOS uses `ParentWindowHandle::Mac(core::ptr::null_mut())`.
  - Windows uses `ParentWindowHandle::Windows(core::ptr::null_mut())`.
  - The helper constructs `AaxEditor::new`, calls `create_view`, `view_size`, `timer_wakeup`, `set_parameter(1, 0.5)`, `destroy_view`, and prints `AAX example built successfully`.
- Kept `#![deny(unsafe_code)]` on the example; raw pointer handles are only stored/constructed, never dereferenced.
- Removed unused `gui-test-host`, `gui-mac`, and `gui-win32` dev-dependencies from `crates/gui-aax/Cargo.toml`, keeping only `gui-widgets`.
- Verification: `cargo build -p gui-aax --example gain`, `cargo build -p gui-aax --example gain --features aax`, `cargo clippy -p gui-aax -- -D warnings`, and `cargo fmt -p gui-aax` all pass.

## Task 31: Custom GPU drawing surface

- Added `crates/gui-core/src/widgets/gpu_surface.rs` with `GpuSurface` widget implementing `Widget`, plus `GpuContext` enum (`D3D11`, `Metal`, `Stub`) and raw handle structs `D3D11Context`/`MetalContext` using `*mut c_void` to keep `gui-core` platform-agnostic and `#![deny(unsafe_code)]`.
- `GpuSurface::on_render` stores a `'static` `FnMut(&mut GpuContext)` callback in a `Cell<Option<Box<dyn FnMut>>>`, matching the pattern used by `gui-widgets` controls.
- Added `PaintCommand::DrawGpuSurface { rect }` in `crates/gui-core/src/paint.rs` so the widget participates in the command list; the 2D backend can ignore it.
- Gated the widget/module behind a `gpu-surface` feature in `gui-core`, `gui-win32`, and `gui-mac` so Metal/DirectX are not required for basic widgets.
- Implemented `crates/gui-win32/src/gpu.rs`: `render_gpu_surface_to_hwnd` creates a D3D11 device, DXGI swap chain, and render target view, sets the viewport, invokes the callback with `GpuContext::D3D11`, and presents. Added `clear_render_target` convenience helper for callbacks.
- Implemented `crates/gui-mac/src/gpu.rs`: `render_gpu_surface_to_view` configures the `NSView` with a `CAMetalLayer`, creates the default Metal device and command queue, and invokes the callback with `GpuContext::Metal`. Added `clear_to_color` convenience helper that builds a render pass descriptor and commits a clear.
- Created `crates/gui-test-host/examples/gpu-surface.rs` with a `PluginEditor` that creates a `GpuSurface`, sets a callback clearing to dark blue, and renders each frame. Marked the example as `required-features = ["gpu-surface"]` so `cargo test --workspace` skips it when the feature is off.
- Added unit tests in `gui-core` for `GpuSurface` construction, `GpuContext` size, and callback invocation, plus compile-only export tests in `gui-mac` and `gui-win32`.
- Verified on macOS: `cargo build -p gui-test-host --example gpu-surface --features gpu-surface`, `cargo run -p gui-test-host --example gpu-surface --features gpu-surface -- --duration-ms 1000`, `cargo test -p gui-core -p gui-mac -p gui-win32 --features gpu-surface`, `cargo clippy -p gui-core -p gui-mac -p gui-win32 -- -D warnings`, and `cargo test --workspace` all pass.
- Cross-checked Windows compilation with `cargo check -p gui-win32 --target x86_64-pc-windows-msvc --features gpu-surface` and `cargo clippy -p gui-win32 --target x86_64-pc-windows-msvc --features gpu-surface -- -D warnings`; both pass.


## Task 27: gui-core animation system

- Created `crates/gui-core/src/animation.rs` with `AnimationCurve::{Linear, EaseInOut, Spring}` and an `ease(t: f32) -> f32` method.
- Defined `Animatable` trait with `lerp` and implemented it for `f32`, `Color`, and `Transform`.
- Defined `Animation<T: Animatable>` with `start`, `stop`, `pause`, `resume`, `restart`, completion callback support, and elapsed/duration tracking.
- Defined `AnimationController<T: Animatable>` that owns active animations, provides `tick(dt: Duration, on_event)`, and returns `AnimationEvent::Value` / `AnimationEvent::Completed` events via callback.
- The controller's active list uses `alloc::vec::Vec` and retains capacity across frames; a secondary reusable `removals` Vec tracks indices for `swap_remove`, so no per-frame allocation occurs.
- Made `Transform` matrix fields and the existing `sin_cos` helper `pub(crate)` so `Transform::lerp` and the spring curve can reuse the no_std math.
- Added a manual `exp` Taylor-series helper because `f32::exp` is unavailable under `#![no_std]`.
- Added 10 unit tests covering linear completion, ease-in-out bounds, spring overshoot, cancellation, restart, concurrent independent animations, color/transform interpolation, and pause/resume.
- Wired `pub mod animation` and re-exported `Animatable`, `Animation`, `AnimationController`, `AnimationCurve`, `AnimationEvent`, `AnimationId`, `AnimationState`, and `AnimationTick` from `crates/gui-core/src/lib.rs`.
- Verification: `cargo test -p gui-core animation` passes (10 tests), `cargo test -p gui-core` passes (50 unit + 2 integration tests), `cargo clippy -p gui-core -- -D warnings` passes.

## Task 8: gui-win32 Direct2D render backend

- Added `crates/gui-win32/src/render.rs` with `D2DRenderBackend` implementing `gui_core::RenderBackend`.
- Created a Direct2D factory (`D2D1CreateFactory`), a D3D11 device, a DXGI swap chain bound to the HWND, and a device context render target from the swap-chain back buffer (`CreateBitmapFromDxgiSurface`).
- Implemented `Clear`, `FillRect`, `StrokeRect`, `FillRoundedRect`, `StrokeRoundedRect`, `FillPath`, `StrokePath`, and `LinearGradient`; left `DrawImage`/`DrawText` as no-ops for v0.1.
- Direct2D uses a top-left origin, so no Y-axis flip is required (unlike the CoreGraphics backend).
- `Color::to_premultiplied_f32()` feeds D2D color/gradient stops directly.
- Per-frame transient data (path points, gradient stops) is kept in reusable `Vec` buffers cleared each command.
- `render_to_hwnd` constructs the backend each frame, resizes the swap chain in `begin()` when the logical size changed, and presents with `Present(1, DXGI_PRESENT(0))`.
- Added `windows-numerics` for `Vector2` used by `ID2D1GeometrySink::AddLines`/`BeginFigure` and `D2D1_LINEAR_GRADIENT_BRUSH_PROPERTIES`.
- Added the requested `windows` crate Direct2D/DXGI/DirectComposition/Imaging features.
- Added a non-Windows stub so `cargo build -p gui-win32` succeeds on macOS.
- Added `d2d-rect` example in `crates/gui-test-host/examples/` mirroring `cg-rect.rs`, gated with `#[cfg(target_os = "windows")]`.
- Added compile-only `d2d_render_backend_exports` unit test.
- Verification: `cargo build -p gui-win32`, `cargo test -p gui-win32`, `cargo clippy -p gui-win32 -- -D warnings`, and `cargo build -p gui-test-host --example d2d-rect` succeed on macOS; runtime test awaits Windows.

## Task 12: gui-vst3 blank test-host example

- Created `crates/gui-vst3/examples/gain.rs` implementing a minimal `GainEditor` that runs inside `gui-test-host` without a DAW.
- Added `gui-test-host`, `gui-mac` (macOS), `gui-win32` (Windows), `gui-widgets`, and `gui-host` as dev-dependencies in `crates/gui-vst3/Cargo.toml`, using target-specific dev-dependencies for the platform crates.
- `GainEditor` owns a `Tree` with a root `Panel`, a `Label`, and a `Slider` bound to `ParameterId(1)`, plus a shared `Arc<LockFreeParameterGateway>`.
- The slider's `on_changed` callback forwards normalized values through the gateway and prints the value in dB.
- Layout is computed in `new` and recomputed on `resize`; frames are applied to concrete widget types via `gui_core::downcast_widget_ref`.
- `idle` rebuilds paint commands from the tree and renders via `gui_mac::render_to_view` on macOS or `gui_win32::render_to_hwnd` on Windows.
- `main()` parses `--duration-ms`, `--width`, and `--height` and delegates to `gui_test_host::run_test_host_with_editor`.
- Added a `#[cfg(test)]` unit test in the example that wires a `Slider` callback to a shared `Vec`, dispatches a mouse-down event through `EventDispatcher`, and asserts the callback receives a normalized value between 0.4 and 0.6.
- Kept the example `#![deny(unsafe_code)]`; raw pointer handles are stored but never dereferenced.
- Fixed a pre-existing resolver error in `crates/gui-win32/Cargo.toml` by removing the non-existent `Win32_Graphics_DirectWrite` and `Win32_Graphics_DirectWrite_Common` `windows` crate features (DirectWrite is unused in the D2D backend).
- Verification: `cargo build -p gui-vst3 --example gain`, `cargo run -p gui-vst3 --example gain -- --test-host --duration-ms 1000` (prints `EditorAttached`/`EditorDetached`), `cargo test -p gui-vst3 --example gain` (1 passed), `cargo clippy -p gui-vst3 -- -D warnings`, and `cargo clippy -p gui-win32 -- -D warnings` all pass.

## Task 9: gui-win32 DirectWrite text rendering

- Added `crates/gui-win32/src/text.rs` with `TextLayout` backed by `IDWriteTextFormat`/`IDWriteTextLayout`, a `TextCache` keyed by `(text_hash, font_size_bits, font_key)`, and `draw_text_to_hwnd`.
- `TextLayout::new` uses the system font collection (Segoe UI -> Arial -> Microsoft Sans Serif -> Tahoma -> first family fallback).
- `TextLayout::with_font` loads embedded font bytes via `IDWriteFactory5::CreateInMemoryFontFileLoader`, builds a font set, creates a custom collection, and unregisters the loader on drop so custom font resources are released.
- Metrics come from `IDWriteTextLayout::GetMetrics` (width/height) and `GetLineMetrics` (baseline).
- `TextLayout::draw` sets the D2D solid brush to white and renders via `ID2D1RenderTarget::DrawTextLayout`.
- Added non-Windows stubs so `cargo build -p gui-win32` succeeds on macOS.
- Created `crates/gui-test-host/examples/d2d-text.rs` rendering "Hello DirectWrite" via `gui_win32::draw_text_to_hwnd` on Windows and printing on macOS.
- While wiring this up, fixed pre-existing `gui-win32`/`gui-test-host` Windows compile issues against `windows` crate 0.62.2:
  - `HWND`/`HMENU`/`HINSTANCE` handles now require `Some(...)` in many API calls.
  - `windows_numerics::Vector2` fields are uppercase `X`/`Y`.
  - `DXGI_SWAP_CHAIN_DESC1` has a required `Stereo` field.
  - `D3D_DRIVER_TYPE_*` lives under `Win32_Graphics_Direct3D`, not `Win32_Graphics_Direct3D11`.
  - `ID2D1DeviceContext` shadows `CreateGradientStopCollection` with a newer overload; call the `ID2D1RenderTarget` overload explicitly.
  - `CreatePathGeometry` is on `ID2D1Factory1`, not the device context.
- Verification: `cargo build -p gui-win32`, `cargo test -p gui-win32`, `cargo clippy -p gui-win32 -- -D warnings`, and `cargo build -p gui-test-host --example d2d-text` pass on macOS. Cross-checked the Windows code path with `cargo check -p gui-win32 --target x86_64-pc-windows-msvc` and `cargo clippy -p gui-win32 --target x86_64-pc-windows-msvc -- -D warnings`.

## Task 29: gui-vst3 animated example plugin

- Extended `crates/gui-vst3/examples/gain.rs` with a looping peak-meter animation and embedded SVG/PNG assets.
- Added `crates/gui-res/resources/knob.svg` (64x64 gray circle with white tick) and `crates/gui-res/resources/logo.png` (32x32 blue square). `crates/gui-res/build.rs` auto-registered both in `src/generated.rs` via `ResourceId::from_bytes_le`.
- `GainEditor` now owns an `AnimationController<f32>` driving a 1.5-second oscillation: 0.0→1.0 over 750 ms, then 1.0→0.0 over 750 ms, using `AnimationCurve::EaseInOut`. On `AnimationEvent::Completed`, the direction flips and a new animation is started.
- Each `idle()` measures elapsed wall-clock time, advances the controller, and rebuilds the paint command list to reflect the animated peak-meter bar height.
- Resources are loaded once in `GainEditor::new()` through a `ResourceRegistry` registered with `gui_res::generated::EMBEDDED`; the resulting `ResourceHandle<SvgImage>` and `ResourceHandle<PngImage>` are stored in the editor.
- Drawing uses placeholder `PaintCommand::DrawImage` rectangles sized to the resources' intrinsic dimensions; the actual render backends still leave `DrawImage` as a no-op in v0.1.
- To avoid per-frame allocation, `GainEditor` keeps a `CommandList` with pre-allocated capacity (32) and calls `clear()` each frame; `AnimationController`'s internal `Vec`s retain capacity across ticks.
- Added two unit tests in the example: one verifying the animation value advances across two simulated frames, and one verifying `rebuild_commands()` does not change `CommandList` capacity between frames.
- Added `gui-res` as a dev-dependency in `crates/gui-vst3/Cargo.toml`.
- Verification: `cargo run -p gui-vst3 --example gain -- --test-host --duration-ms 3000` prints `EditorAttached`/`EditorDetached`, `cargo test -p gui-vst3 --example gain` passes (3 tests), `cargo clippy -p gui-vst3 -- -D warnings` passes, `cargo test -p gui-res` passes (11 tests), and `cargo clippy -p gui-res -- -D warnings` passes.

## Task 31: custom GPU drawing surface

- Added `crates/gui-core/src/widgets/gpu_surface.rs` with `GpuSurface` widget, `GpuContext` enum (`D3D11`/`Metal`/`Stub`), and `D3D11Context`/`MetalContext` raw handle structs.
- Gated `pub mod widgets` and the `GpuSurface` re-exports behind a `gpu-surface` feature in `gui-core/Cargo.toml`.
- Added `crates/gui-win32/src/gpu.rs` with `render_gpu_surface_to_hwnd` creating a D3D11 device/swap chain/render target view and invoking a callback with `GpuContext::D3D1`; includes `clear_render_target` helper.
- Added `crates/gui-mac/src/gpu.rs` with `render_gpu_surface_to_view` configuring a `CAMetalLayer`, default Metal device, command queue, and invoking a callback with `GpuContext::Metal`; includes `clear_to_color` helper.
- Added `crates/gui-test-host/examples/gpu-surface.rs` clearing the surface to dark blue each frame, with `required-features = ["gpu-surface"]`.
- Verification: `cargo build -p gui-test-host --example gpu-surface --features gpu-surface`, `cargo run -p gui-test-host --example gpu-surface --features gpu-surface -- --duration-ms 1000`, and `cargo test -p gui-core -p gui-mac -p gui-win32 --features gpu-surface` all pass.

## Task 34: end-to-end cross-platform validation

- Updated `xtask/src/main.rs` so `cargo xtask validate` runs `cargo test --workspace` and `cargo clippy --workspace -- -D warnings`, printing PASS/FAIL for each.
- Generated `.omo/evidence/validation-report.md` summarizing the validation matrix.
- `cargo xtask validate` exits 0 on macOS.
- Runnable checks on macOS (workspace tests/clippy, test-host examples, VST3/AU test-host examples, GPU surface example) all pass.

## Documentation follow-up

- Created `/Users/minjaekim/Plugins/gui/README.md` with project overview, feature matrix, crate listing, quick-start commands, architecture notes, platform support matrix, development commands, CI summary, and license placeholder.
- Updated `/Users/minjaekim/Plugins/gui/.gitignore` with standard Rust/project entries (target/, IDE directories, swap/backup files, logs, OS files) while keeping existing entries and retaining `Cargo.lock`.
- Added `description`, `repository`, and `documentation` fields to root `Cargo.toml` under `[workspace.package]`.
- Added concise `//!` crate-level doc comments to all 12 crate roots (`gui-core`, `gui-host`, `gui-res`, `gui-accessibility`, `gui-win32`, `gui-mac`, `gui-vst3`, `gui-au`, `gui-aax`, `gui-test-host`, `gui-widgets`, `xtask`).
- Verified `cargo doc --workspace --no-deps` builds successfully and generates docs for all crates.
- Note: subagent tasks timed out repeatedly on this documentation-only work, so the changes were applied directly after delegation failures.
- Windows-specific runtime checks (Direct2D examples, VST3 validator), AAX SDK build, `auval`, and Miri are marked SKIP in the report with environment-specific reasons.


