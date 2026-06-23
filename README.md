# Rust Plugin GUI Framework

A cross-platform retained-mode GUI library for VST3/AU/AAX audio plugins on Windows and macOS.

> **Status**: Active development — all core subsystems implemented and passing tests on macOS. Windows-specific runtime validation is pending a Windows CI runner.

## Overview

This workspace provides a native, retained widget-tree GUI framework designed specifically for audio plugin editors. It combines platform-native rendering (Direct2D on Windows, CoreGraphics on macOS) with a platform-agnostic core so that plugin UIs can be built once and wrapped for VST3, AU, and AAX hosts.

### Key Features

| Feature | Status | Notes |
|---------|--------|-------|
| Windows Win32 + Direct2D backend | Implemented | Runtime validation pending Windows host |
| macOS AppKit + CoreGraphics backend | Implemented & tested | |
| DirectWrite / CoreText text rendering | Implemented | |
| Retained widget tree + lifecycle | Implemented | |
| Box/flex layout engine | Implemented | |
| Mouse & keyboard event routing | Implemented | |
| HiDPI scaling | Implemented | Per-monitor DPI aware on Windows; backing scale on macOS |
| Animation engine | Implemented | Linear, ease-in-out, spring curves |
| Parameter binding API | Implemented | Lock-free UI/audio gateway |
| Embedded resource system | Implemented | Static embedding + typed registry |
| SVG renderer | Implemented | via `resvg`/`usvg`/`tiny-skia` |
| PNG decoder | Implemented | via `image` crate |
| Accessibility metadata | Implemented | Widget roles/values; platform backends are stubs |
| Custom GPU drawing surface | Implemented | D3D11/Metal, gated behind `gpu-surface` feature |
| VST3 wrapper | Implemented | via `vst3-sys` |
| AU wrapper | Implemented | Cocoa UI for AUv2 |
| AAX wrapper | Build-only stub | Requires AAX SDK and `aax` feature |
| DAW-less test host | Implemented | Run examples without a DAW |

## Workspace Crates

| Crate | Purpose |
|-------|---------|
| `gui-core` | Geometry, color, math, paint command list, widget tree, layout, events, animation, accessibility metadata. `#![no_std]` / `#![deny(unsafe_code)]`. |
| `gui-host` | Plugin editor and host abstraction traits; lock-free parameter gateway. `#![deny(unsafe_code)]`. |
| `gui-res` | Resource IDs, static embedding, typed registry, SVG/PNG decoders. `#![deny(unsafe_code)]`. |
| `gui-widgets` | Built-in controls: `Slider`, `Knob`, `Button`, `Label`, plus theme. `#![deny(unsafe_code)]`. |
| `gui-accessibility` | Accessibility backend trait and platform stubs. |
| `gui-win32` | Win32 windowing, Direct2D render backend, DirectWrite text, D3D11 GPU surface. |
| `gui-mac` | AppKit/NSView windowing, CoreGraphics render backend, CoreText text, Metal GPU surface. |
| `gui-vst3` | VST3 `IPlugView` wrapper around `gui-host::PluginEditor`. |
| `gui-au` | Audio Unit Cocoa UI wrapper around `gui-host::PluginEditor`. |
| `gui-aax` | AAX editor wrapper (build-only stub without the AAX SDK). |
| `gui-test-host` | DAW-less standalone test host for rapid iteration. |
| `xtask` | Build, bundle, validate, and CI helper commands. |

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

# Run the parameter-bound gain example (VST3 wrapper in test-host mode)
cargo run -p gui-vst3 --example gain -- --test-host --duration-ms 3000

# Run the AU gain example on macOS
cargo run -p gui-au --example gain -- --test-host --duration-ms 1000

# Build the GPU surface example
cargo run -p gui-test-host --example gpu-surface --features gpu-surface -- --duration-ms 1000
```

## Architecture

### Retained Widget Tree

Widgets live in a `Tree` owned by the plugin editor. Each widget has a stable `WidgetId`, optional children, layout constraints, and lifecycle hooks (`mount`, `unmount`, `update`).

### Zero-Allocation Paint Path

Each frame the widget tree rebuilds a `CommandList` of `PaintCommand`s. The command list reuses its backing capacity across frames, so the per-frame paint replay path does not allocate. Resource loading, layout, and tree mutations are allowed to allocate.

### Parameter Gateway

`gui-host::LockFreeParameterGateway` provides bounded lock-free queues between the audio thread and the UI thread. Widgets call `begin_gesture`, `set_normalized`, and `end_gesture`; the audio thread pushes changes back via `send_from_audio`.

### Platform Backends

The core is platform-agnostic. Platform crates translate `PaintCommand`s into Direct2D or CoreGraphics calls and native window events into `gui-core::Event`s. Unsafe code is isolated to the platform crates and gated with `#![deny(unsafe_code)]` in `gui-core`, `gui-host`, `gui-res`, `gui-accessibility`, and `gui-widgets`.

## Platform Support Matrix

| Target | Build | Runtime Tests | Notes |
|--------|-------|---------------|-------|
| macOS (aarch64/x86_64) | PASS | PASS on host | CoreGraphics, CoreText, Metal GPU surface validated |
| Windows (x86_64) | PASS via cross-check | SKIP on this host | Direct2D/DirectWrite/D3D11 code compiles; runtime needs Windows CI |
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

AAX builds are gated by the `aax` feature and the `AAX_SDK_PATH` environment variable.

## License

This project is licensed under the MIT OR Apache-2.0 license. See `LICENSE-MIT` and `LICENSE-APACHE` (to be added).

## Acknowledgments

- `resvg` / `usvg` / `tiny-skia` for SVG rasterization
- `image` for PNG decoding
- `vst3-sys` for VST3 bindings
- Inspiration from JUCE, VSTGUI, iPlug2, nih-plug, and VIZIA
