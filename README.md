# audio-processor-rs

A cross-platform audio plugin build and packaging system for Rust: write **one processor (DSP) file and one UI file**, and build/bundle it as a Standalone app, VST3, AUv2, and AAX plugin on Windows and macOS.

> **Status**: Active development — all core subsystems implemented and passing tests on macOS. Windows-specific runtime validation is pending a Windows CI runner.

## Overview

`plugins/gain` is the reference single project: `src/processor.rs` (the DSP) and `src/ui.rs` (the editor) are shared, unmodified, across every build target. Each format's entry point lives behind its own Cargo feature and wires the same processor/UI pair into that format's real plugin ABI — VST3's `IPluginFactory`/`IComponent`/`IAudioProcessor`/`IEditController`, AUv2's `AudioComponentPlugInInterface`, or a real `cpal`-backed standalone host — without either file needing to know which format it's running under.

Formats are enabled per build via environment variable (no path set means that format is skipped, not just unsigned):

| Format | Enabled by | Notes |
|--------|-----------|-------|
| Standalone | always | Real audio I/O (input + output device selection) via `cpal`; window via [`mkgraphic`](https://crates.io/crates/mkgraphic) on macOS |
| VST3 | `VST3_SDK_PATH` set | `vst3-sys` is self-contained (no separate SDK download needed); the env var is purely an explicit opt-in gate |
| AUv2 | macOS only | No SDK needed (AudioToolbox ships with the OS) |
| AAX | `AAX_SDK_PATH` set | Build-only: verifies `gui-aax` builds against a real SDK; no `AAX_CMain`/effect description yet, so not yet a loadable `.aaxplugin` |

Underneath the single project, this workspace also provides the platform-agnostic GUI framework it's built on: a retained widget tree, box/flex layout, native rendering (Direct2D on Windows, CoreGraphics on macOS), and a lock-free UI/audio parameter gateway.

### Key Features

| Feature | Status | Notes |
|---------|--------|-------|
| Single project → 4 formats | Implemented | `plugins/gain`: one `Processor` + one `PluginEditor`, four real entry points |
| Standalone host | Implemented | Real duplex audio I/O (`cpal`), input+output device picker on macOS |
| VST3 entry point | Implemented | Real `IPluginFactory`/`IComponent`/`IAudioProcessor`/`IEditController` via `vst3-sys`, gated by `VST3_SDK_PATH` |
| AUv2 entry point | Implemented | Real `AudioComponentPlugInInterface` (hand-written dispatch over `au-sys` bindings) + `AUCocoaUIBase` custom UI, no SDK needed |
| AAX wrapper | Build-only stub | Gated by `AAX_SDK_PATH`; no real `AAX_CMain` yet |
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
| `gui-aax` | AAX editor wrapper (build-only stub without the AAX SDK). |
| `gui-test-host` | DAW-less standalone test host for rapid iteration. |
| `xtask` | Build, bundle, sign, validate, and CI helper commands. |

## The reference plugin: `plugins/gain`

```
plugins/gain/
  build.rs            # mirrors gui-vst3's VST3_SDK_PATH gate at this crate's own compile time
  src/
    processor.rs       # the one processor file — GainProcessor: gui_host::Processor
    ui.rs               # the one UI file — GainEditor: gui_host::PluginEditor
    lib.rs               # ties them together; vst3_entry!/au_entry! invocations behind cargo features
    bin/
      standalone.rs      # gui_standalone::run(GainProcessor::new(), ..., GainEditor::new(...), ...)
```

Cargo features: `standalone` (pulls in `gui-standalone`), `vst3` (pulls in `gui-vst3`; needs `VST3_SDK_PATH` at build time for a real entry point), `au` (pulls in `gui-au`; macOS only, real entry point unconditionally).

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

Bundling honesty: `bundle-vst3`/`bundle-au` check (via `nm`, not assumption) whether the built `libgain_plugin.dylib` actually exports the format's real entry point symbol (`GetPluginFactory` / `gain_au_factory`). If the format's gate wasn't set, the bundle is still assembled and signed, but reported as `SKIP` — packaging plumbing only, not yet DAW-scannable.

Plugin identity and code signing are resolved from env vars (falling back to an interactive prompt if stdin is a terminal): `PLUGIN_NAME` (default "Gain"), `PLUGIN_COMPANY` (default "mkaudio"), `CODESIGN_IDENTITY` (a keychain identity name, or `-` for ad-hoc signing; unset + non-interactive skips signing).

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

## Platform Support Matrix

| Target | Build | Runtime Tests | Notes |
|--------|-------|---------------|-------|
| macOS (aarch64/x86_64) | PASS | PASS on host | CoreGraphics, CoreText, Metal GPU surface, real VST3/AU entry points, and real duplex audio all validated on real hardware |
| Windows (x86_64) | PASS via cross-check | SKIP on this host | Direct2D/DirectWrite/D3D11 code and the Win32 mouse-input path compile; runtime needs Windows CI |
| Linux | Not supported | N/A | Out of scope |

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
