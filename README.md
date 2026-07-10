# audio-processor-rs

A cross-platform audio plugin build and packaging system for Rust: write **one processor (DSP) file and one UI file**, and build/bundle it as a Standalone app, VST3, AUv2, and AAX plugin on Windows and macOS.

> **Status**: Active development — all core subsystems implemented and passing tests on macOS. Every plugin format (VST3, AUv2, AAX) has been validated against its real, official host-vendor validator (Steinberg's `validator`, Apple's `auval`, Avid's `AAXValidator.framework`), not just unit tests — see [Real validator testing](#real-validator-testing). Windows-specific runtime validation is pending a Windows CI runner.

## Overview

`plugins/gain` is the reference single project: `src/processor.rs` (the DSP) and `src/ui.rs` (the editor) are shared, unmodified, across every build target. Each format's entry point lives behind its own Cargo feature and wires the same processor/UI pair into that format's real plugin ABI — VST3's `IPluginFactory`/`IComponent`/`IAudioProcessor`/`IEditController`, AUv2's `AudioComponentPlugInInterface`, AAX's `AAX_CEffectParameters`/`AAX_CMain`, or a real `cpal`-backed standalone host — without either file needing to know which format it's running under.

Formats are enabled per build via environment variable (no path set means that format is skipped, not just unsigned):

| Format | Enabled by | Notes |
|--------|-----------|-------|
| Standalone | always | Real audio I/O (input + output device selection) via `cpal`; window via [`mkgraphic`](https://crates.io/crates/mkgraphic) on macOS |
| VST3 | `VST3_SDK_PATH` set | `vst3-sys` is self-contained (no separate SDK download needed); the env var is purely an explicit opt-in gate |
| AUv2 | macOS only | No SDK needed (AudioToolbox ships with the OS) |
| AAX | `AAX_SDK_PATH` set | Real, loadable `.aaxplugin`: a generic C++ shim (`crates/gui-aax/cpp/`) built by the AAX SDK's own CMake tooling, bridging into the same `Processor` every other format uses — see [AAX architecture](#aax-architecture-generic-c-shim--rust-processor) |

Underneath the single project, this workspace also provides the platform-agnostic GUI framework it's built on: a retained widget tree, box/flex layout, native rendering (Direct2D on Windows, CoreGraphics on macOS), and a lock-free UI/audio parameter gateway.

### Key Features

| Feature | Status | Notes |
|---------|--------|-------|
| Single project → 4 formats | Implemented | `plugins/gain`: one `Processor` + one `PluginEditor`, four real entry points |
| Standalone host | Implemented | Real duplex audio I/O (`cpal`), input+output device picker on macOS |
| VST3 entry point | Implemented & validated | Real `IPluginFactory`/`IComponent`/`IAudioProcessor`/`IEditController` via `vst3-sys`, gated by `VST3_SDK_PATH`; 47/47 on Steinberg's own `validator` |
| AUv2 entry point | Implemented & validated | Real `AudioComponentPlugInInterface` (hand-written dispatch over `au-sys` bindings) + `AUCocoaUIBase` custom UI, no SDK needed; passes Apple's `auval` |
| AAX entry point | Implemented & validated | Real `AAX_CEffectParameters`/`AAX_CMain` via a generic C++ shim (gated by `AAX_SDK_PATH`); 6/6 on Avid's own `AAXValidator.framework` |
| Windows Win32 + Direct2D backend | Implemented | Runtime validation pending Windows host |
| macOS AppKit + CoreGraphics backend | Implemented & tested | Custom `drawRect:`-based `NSView` (not `lockFocus`, which doesn't reliably composite) |
| Real mouse input | Implemented | `mouseDown:`/`mouseDragged:`/`mouseUp:` on macOS, `WM_LBUTTONDOWN`/`UP`/`MOUSEMOVE` on Windows test host |
| Retained widget tree + lifecycle | Implemented | |
| Box/flex layout engine | Implemented | |
| HiDPI scaling | Implemented | Per-monitor DPI aware on Windows; backing scale on macOS |
| Animation engine | Implemented | Linear, ease-in-out, spring curves |
| Parameter binding API | Implemented | Lock-free UI/audio gateway |
| Real peak metering | Implemented for Standalone/AU | Lock-free `PeakMeter`, audio thread → UI thread; VST3 not yet wired (flat) |
| Embedded resource system | Implemented | Static embedding + typed registry |
| SVG renderer | Implemented | via `resvg`/`usvg`/`tiny-skia` |
| PNG decoder | Implemented | via `image` crate |
| Accessibility metadata | Implemented | Widget roles/values; mirrored to real `NSAccessibilityElement`s on macOS |
| Custom GPU drawing surface | Implemented | D3D11/Metal, gated behind `gpu-surface` feature |
| DAW-less test host | Implemented | Run editors without a DAW |

## Workspace Crates

| Crate | Purpose |
|-------|---------|
| `gui-core` | Geometry, color, math, paint command list, widget tree, layout, events, animation, accessibility metadata. `#![no_std]` / `#![deny(unsafe_code)]`. |
| `gui-host` | `Processor` and `PluginEditor`/host abstraction traits, lock-free parameter gateway (`LockFreeParameterGateway`) and peak meter (`PeakMeter`). `#![deny(unsafe_code)]`. |
| `gui-res` | Resource IDs, static embedding, typed registry, SVG/PNG decoders. `#![deny(unsafe_code)]`. |
| `gui-widgets` | Built-in controls: `Slider`, `Knob`, `Button`, `Label`, plus theme. `#![deny(unsafe_code)]`. |
| `gui-accessibility` | Accessibility backend trait and platform stubs. |
| `gui-win32` | Win32 windowing, Direct2D render backend, DirectWrite text, D3D11 GPU surface. |
| `gui-mac` | AppKit/NSView windowing, CoreGraphics render backend (`drawRect:`-based `GuiPaintView`, with real mouse input), CoreText text, Metal GPU surface. |
| `gui-standalone` | Standalone desktop host: real `cpal` audio I/O (input + output device selection, ring-buffer-bridged duplex capture/playback) and a real window, wired to any `Processor` + `PluginEditor` pair. |
| `gui-vst3` | Real VST3 plugin entry point (`vst3_entry!` macro) gated by `VST3_SDK_PATH`, plus the `IPlugView` wrapper always available. |
| `gui-au` | Real AUv2 plugin entry point (`au_entry!` macro, hand-written `AudioComponentPlugInInterface` dispatch over `au-sys`) plus an `AUCocoaUIBase`-ready Cocoa UI. |
| `gui-aax` | Real AAX plugin entry point: a generic, plugin-agnostic C++ shim (`cpp/`) built by the AAX SDK's own CMake tooling, bridged into any `gui_host::Processor` via the `aax_entry!` macro and a page-table generator (`page_table` module); gated by `AAX_SDK_PATH`. Build-only stub without the SDK. |
| `gui-test-host` | DAW-less standalone test host for rapid iteration. |
| `xtask` | Build, bundle, sign, validate, and CI helper commands. |

## The reference plugin: `plugins/gain`

```
plugins/gain/
  build.rs            # mirrors gui-vst3's/gui-aax's VST3_SDK_PATH/AAX_SDK_PATH gates at this crate's own compile time
  src/
    processor.rs       # the one processor file — GainProcessor: gui_host::Processor
    ui.rs               # the one UI file — GainEditor: gui_host::PluginEditor
    lib.rs               # ties them together; vst3_entry!/au_entry!/aax_entry! invocations behind cargo features
    bin/
      standalone.rs      # gui_standalone::run(GainProcessor::new(), ..., GainEditor::new(...), ...)
      aax_page_table.rs  # prints this plugin's AAX page-table XML, for xtask to capture at build time
```

Cargo features: `standalone` (pulls in `gui-standalone`), `vst3` (pulls in `gui-vst3`; needs `VST3_SDK_PATH` at build time for a real entry point), `au` (pulls in `gui-au`; macOS only, real entry point unconditionally), `aax` (pulls in `gui-aax`; needs `AAX_SDK_PATH` at build time for a real entry point).

## Creating a new plugin

`cargo xtask new-plugin <slug>` scaffolds a new `plugins/<slug>` crate from `plugins/gain` as a template — same processor/UI starting point (`GainProcessor`/`GainEditor` renamed to `<Slug>Processor`/`<Slug>Editor`), fresh VST3 GUIDs, fresh AAX/AU FourCC codes derived from the plugin name and company, and registers the new crate in the workspace automatically:

```bash
cargo xtask new-plugin delay \
  --display-name Delay \
  --company "Acme Audio" \
  --formats standalone,vst3,au,aax
```

| Flag | Default | Purpose |
|------|---------|---------|
| `<slug>` (positional) | *required* | Lowercase ASCII letters/digits/hyphens; becomes `plugins/<slug>`, package name `<slug>-plugin`, lib name `<slug>_plugin` |
| `--display-name` | Titlecased slug | Shown to hosts/users (VST3 `name`, AAX `name`/plugin display, bundle `CFBundleName`) |
| `--company` | `"mkaudio"` | VST3 `vendor`, AAX `manufacturer_name`, and the source for every generated FourCC/bundle-identifier |
| `--formats` | `standalone,vst3,au,aax` | Comma list of formats to include; trims `Cargo.toml`'s deps/features/bin entries and `lib.rs`'s entry-point blocks to just these |

What gets copied verbatim (from the live `plugins/gain` source, not a baked-in template, so it can never drift) versus constructed fresh:

- **Copied + identifier-substituted**: `build.rs`, `src/processor.rs`, `src/ui.rs`, `src/bin/standalone.rs`, `src/bin/aax_page_table.rs` (the latter two only if their format was requested).
- **Constructed fresh**: `Cargo.toml` and `src/lib.rs`, since their content genuinely varies by which formats were selected (conditional features/deps/bin entries and entry-point macro invocations).

Once created, every `bundle-*`/`xtask` command operates on whichever plugin `PLUGIN_CRATE` names (default `"gain"`) — nothing in `xtask` itself hardcodes a plugin name:

```bash
cd plugins/delay
# edit src/processor.rs (DSP) and src/ui.rs (editor UI)
cargo build -p delay-plugin
PLUGIN_CRATE=delay cargo xtask bundle-all
```

## Quick Start

```bash
# Build the entire workspace
cargo build --workspace

# Run all tests
cargo test --workspace

# Run the validation matrix (tests + clippy)
cargo xtask validate

# Run the DAW-less test host with a blank editor
cargo run -p gui-test-host --example blank -- --duration-ms 1000

# Run the Gain plugin standalone (real audio I/O + real window)
cargo run -p gain-plugin --bin gain-standalone --features standalone

# Build the Gain plugin as a real VST3 plugin
VST3_SDK_PATH=/path/to/anything cargo build -p gain-plugin --features vst3

# Build the Gain plugin as a real Audio Unit (macOS)
cargo build -p gain-plugin --features au

# Build the Gain plugin's AAX Rust bridge (the real .aaxplugin bundle needs
# the full `cargo xtask bundle-aax` pipeline below, since AAX also requires
# compiling a C++ shim via the AAX SDK's own CMake tooling)
AAX_SDK_PATH=/path/to/aax-sdk cargo build -p gain-plugin --features aax

# Build the GPU surface example
cargo run -p gui-test-host --example gpu-surface --features gpu-surface -- --duration-ms 1000
```

### Bundling

```bash
# Bundle one format (assembles a real .app/.vst3/.component, code-signs it
# if a signing identity can be resolved — see CODESIGN_IDENTITY below)
cargo xtask bundle-standalone
cargo xtask bundle-vst3
cargo xtask bundle-au
cargo xtask bundle-aax

# Bundle everything and print a PASS/SKIP/FAIL summary
cargo xtask bundle-all
```

Bundling honesty: `bundle-vst3`/`bundle-au` check (via `nm`, not assumption) whether the built cdylib actually exports the format's real entry point symbol (`GetPluginFactory` / `<slug>_au_factory`). If the format's gate wasn't set, the bundle is still assembled and signed, but reported as `SKIP` — packaging plumbing only, not yet DAW-scannable.

Which plugin gets bundled, and its identity/code signing, are all resolved from env vars (falling back to an interactive prompt if stdin is a terminal): `PLUGIN_CRATE` (default `"gain"` — which `plugins/<slug>` crate to build; see [Creating a new plugin](#creating-a-new-plugin)), `PLUGIN_NAME` (default: that plugin's titlecased slug), `PLUGIN_COMPANY` (default "mkaudio"), `CODESIGN_IDENTITY` (a keychain identity name, or `-` for ad-hoc signing; unset + non-interactive skips signing).

`bundle-aax` is a multi-step pipeline (see [AAX architecture](#aax-architecture-generic-c-shim--rust-processor)), requiring:

| Env var | Required | Purpose |
|---------|----------|---------|
| `AAX_SDK_PATH` | Yes | Root of the AAX SDK's source tree (needed for `Interfaces/AAX_Exports.cpp`, which the SDK's own CMake tooling doesn't install) |
| `AAX_SDK_CMAKE_DIR` | No (defaults to `<AAX_SDK_PATH>/INSTALL`) | The AAX SDK's *installed* CMake package dir (from running `cmake --install` on the SDK), containing `AAX_SDKConfig.cmake` |
| `AAX_VALIDATOR_FRAMEWORKS_DIR` | No | The `Frameworks/` directory of a real AAX Plug-In Validator install; when set, `bundle-aax` compiles a small driver against `AAXValidator.framework`'s C API and runs it against the built bundle, reporting real PASS/FAIL |

Without `AAX_VALIDATOR_FRAMEWORKS_DIR`, `bundle-aax` still builds a real, loadable `.aaxplugin` (confirmed via `nm` showing real `ACFStartup`/`ACFRegisterPlugin`/etc. exports) but reports `SKIP` rather than claiming a validator pass it never ran.

## Architecture

### Retained Widget Tree

Widgets live in a `Tree` owned by the plugin editor. Each widget has a stable `WidgetId`, optional children, layout constraints, and lifecycle hooks (`mount`, `unmount`, `update`).

### Zero-Allocation Paint Path

Each frame the widget tree rebuilds a `CommandList` of `PaintCommand`s. The command list reuses its backing capacity across frames, so the per-frame paint replay path does not allocate. Resource loading, layout, and tree mutations are allowed to allocate.

### Rendering: real `drawRect:`, not `lockFocus`

`gui-mac` draws through a custom `NSView` subclass (`GuiPaintView`) with a real `drawRect:` override, invoked by AppKit's own display cycle via `setNeedsDisplay:`. An earlier version drew directly into the host-provided view via `lockFocus`/`unlockFocus`; that succeeds at the API level (valid `CGContext`, no errors) but Apple has deprecated `lockFocus` since 10.14 because the WindowServer's Core Animation compositor isn't guaranteed to pick up content drawn that way — confirmed on real hardware (every draw call succeeded, nothing ever appeared on screen). The same view also owns real mouse input (`mouseDown:`/`mouseDragged:`/`mouseUp:`), forwarded through `PluginEditor::on_mouse_down`/`on_mouse_move`/`on_mouse_up`.

### Parameter Gateway and Peak Meter

`gui-host::LockFreeParameterGateway` provides bounded lock-free queues between the audio thread and the UI thread. Widgets call `begin_gesture`, `set_normalized`, and `end_gesture`; the audio thread pushes changes back via `send_from_audio`. `gui-host::PeakMeter` is the same idea in the other direction: a lock-free scalar the audio thread writes a real per-block peak level into, and the UI thread reads for level metering.

### Platform Backends

The core is platform-agnostic. Platform crates translate `PaintCommand`s into Direct2D or CoreGraphics calls and native window events into `gui-core::Event`s. Unsafe code is isolated to the platform crates and gated with `#![deny(unsafe_code)]` in `gui-core`, `gui-host`, `gui-res`, `gui-accessibility`, `gui-widgets`, and `gui-standalone`.

### AAX architecture: generic C++ shim + Rust processor

Unlike VST3/AU, whose COM-vtable/C-ABI interfaces Rust implements directly, AAX's SDK is C++-only (`AAX_CEffectParameters` is an abstract C++ base class, not a plain C interface). That means AAX is the one format that genuinely needs a C++ layer — but that layer, in `crates/gui-aax/cpp/`, is **written once and reused for any plugin**, not hand-written per plugin:

- `AaxPlugin_Describe.cpp` / `AaxPlugin_Parameters.cpp` / `AaxPlugin_AlgProc.cpp` contain no plugin-specific identifiers or parameter names. At Describe/`EffectInit` time they loop over `extern "C"` getters (`gui_aax_parameter_count`/`_name`/`_default`/`_step_count`, plus plugin-identity getters) that Rust provides — up to a fixed capacity of 16 parameters (`gui_aax::component::MAX_PARAMS`, mirrored as `kAaxGeneric_MaxParams` in `cpp/gui_aax_bridge.h`).
- All of those `extern "C"` functions, plus the real-time bridge `gui_aax_process_block`, are generated by one macro invocation: `gui_aax::aax_entry!` in `plugins/gain/src/lib.rs` — the same shape as `vst3_entry!`/`au_entry!`. A new plugin needs zero new C++.
- Each real-time block constructs a fresh, stack-allocated `Processor` (no boxing, no heap allocation) since AAX's packet dispatcher redelivers every parameter's current value on every block; a processor with real per-block state (e.g. a filter's delay line) would need AAX's private-data instance lifecycle instead, which this bridge doesn't implement.
- AAX's one inherently per-plugin resource — the page-table XML mapping parameters to hardware-console knobs — is *generated*, not hand-maintained: `plugins/gain/src/bin/aax_page_table.rs` prints it from the same parameter metadata, and `cargo xtask bundle-aax` captures that output before invoking CMake.
- Bypass is a real audio passthrough (`memcpy`), not "process with some neutral parameter value" — correct for any future processor, not just a pure-gain one.

The AAX SDK's own CMake tooling (`aax_plugin()` in `AAX_SDKFunctions.cmake`) compiles this shim, links it against the Rust staticlib xtask builds, and assembles the `.aaxplugin` bundle. Two genuine bugs in the SDK's CMake tooling were found and worked around along the way (not patched in the vendored SDK): its example projects' CMakeLists.txt files omit `Interfaces/AAX_Exports.cpp` (present in every one of the SDK's own Xcode/VS templates, needed for the real `ACFStartup`/`ACFRegisterPlugin`/etc. exports), and the top-level SDK CMakeLists.txt unconditionally references an `aax_wrapper` target that only exists when `SMTG_AAX_SDK_PATH` is set.

## Platform Support Matrix

| Target | Build | Runtime Tests | Notes |
|--------|-------|---------------|-------|
| macOS (aarch64/x86_64) | PASS | PASS on host | CoreGraphics, CoreText, Metal GPU surface, real VST3/AU/AAX entry points, and real duplex audio all validated on real hardware |
| Windows (x86_64) | PASS via cross-check | SKIP on this host | Direct2D/DirectWrite/D3D11 code and the Win32 mouse-input path compile; runtime needs Windows CI |
| Linux | Not supported | N/A | Out of scope |

## Real validator testing

Every plugin format has been checked against its real, official host-vendor validator — not just this workspace's own unit tests:

| Format | Validator | Result |
|--------|-----------|--------|
| VST3 | Steinberg's own `validator` (built from `vst3sdk` source) | 47/47 tests pass |
| AUv2 | Apple's `auval` | Passes (session-state persistence — `ClassInfo`/`PresentPreset` — is deliberately out of scope) |
| AAX | Avid's `AAXValidator.framework` C API (`test.data_model`, `.load_unload`, `.parameters`, `.parameter_traversal.linear`, `.page_table.load`, `.describe_validation`) | 6/6 tests pass |

Getting each of these to pass surfaced and fixed real bugs, not hypothetical ones — e.g. AU's manufacturer OSType needing a non-lowercase character, a `<string>`/`<integer>` Info.plist type mismatch that silently deregistered the whole component, a real render crash from an unhandled null `AudioBuffer.mData`, VST3's mandatory `IProcessContextRequirements` (added in 3.7), and the AAX SDK CMake gaps described above.

## Development

### IDE

A `rust-toolchain.toml` is not required; the workspace uses stable Rust 2024 edition (minimum 1.85).

### Formatting and Linting

```bash
cargo fmt --check
cargo clippy --workspace -- -D warnings
```

### RustDoc

Generate documentation for the whole workspace:

```bash
cargo doc --workspace --no-deps
```

Module-level documentation is provided in each crate root; public APIs are documented for RustDoc generation.

## CI

The GitHub Actions workflow (`.github/workflows/ci.yml`) runs on `windows-latest` and `macos-latest`:

- `cargo test --workspace`
- `cargo clippy --workspace -- -D warnings`
- `cargo fmt --check`

VST3 builds are gated by the `vst3` feature and `VST3_SDK_PATH`; AAX builds are gated by the `aax` feature and `AAX_SDK_PATH`; AU needs neither (macOS only).

## License

This project is licensed under the MIT OR Apache-2.0 license. See `LICENSE-MIT` and `LICENSE-APACHE`.

## Acknowledgments

- `resvg` / `usvg` / `tiny-skia` for SVG rasterization
- `image` for PNG decoding
- `vst3-sys` for VST3 bindings
- `au-sys` for AUv2 bindings
- `cpal` for cross-platform audio I/O
- `mkgraphic` for the standalone app's native window
- Inspiration from JUCE, VSTGUI, iPlug2, nih-plug, and VIZIA
