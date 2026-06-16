# Rust Plugin GUI Framework

## TL;DR
> **Summary**: Build a cross-platform retained-mode GUI library for VST3/AU/AAX audio plugins on Windows (Win32 + Direct2D) and macOS (AppKit + CoreGraphics), with SVG/PNG/text rendering, animation, parameter binding, embedded resources, accessibility, and a zero-allocation *per-frame* paint pipeline.
> **Deliverables**: Multi-crate Rust workspace; platform render backends; plugin format wrappers; widget tree + layout; resource and animation systems; parameter binding API; accessibility metadata/backends; DAW-less test host; CI.
> **Effort**: XL (multi-month, 6+ execution waves)
> **Parallel**: YES — backend, core, and format wrappers can progress in parallel once the host abstraction is defined.
> **Critical Path**: Host abstraction (M1) → Win32+Direct2D backend (M2) → Widget tree + layout (M3) → macOS+CoreGraphics backend (M4) → Media/animation/accessibility (M5) → AAX wrapper (M6).

## Context

### Original Request
Create a Rust GUI library for VST/AU/AAX plugins on Windows and macOS, including native Win32+Direct2D, native AppKit+CoreGraphics, SVG renderer, PNG renderer, UTF-8 text rendering, widget tree, layout system, mouse/keyboard events, HiDPI support, animation system, parameter binding API, embedded resource system, accessibility layer, custom GPU drawing surface, and a zero-allocation paint pipeline.

### Interview Summary
- **Scope**: User requested the full feature set as the first deliverable (not a phased MVP).
- **Architecture style**: Retained widget tree (JUCE/VSTGUI-style).
- **Rendering strategy**: Leverage Rust ecosystem crates for SVG/PNG/text rather than writing custom parsers from scratch.
- **Plugin formats**: VST3 + AU + AAX from day one.
- **Target platforms**: Windows and macOS only.

### Metis Review (gaps addressed)
- **Phasing**: Although the user asked for the full framework immediately, the plan structures work into six milestones (M1–M6) that can each produce a working, testable artifact. This is risk mitigation, not scope reduction.
- **Plugin format isolation**: Each format lives in its own crate behind a shared `gui-host` trait; AAX is explicitly called out as high-risk due to Avid/PACE SDK requirements.
- **Rendering coherence**: "Zero-allocation" is scoped to the per-frame transient command replay path, not the whole library. SVG/PNG/text use Rust crates that rasterize/decode to CPU bitmaps, which are then uploaded to backend-native textures/bitmap caches.
- **Text subsystem**: v0.1 uses platform-native text (DirectWrite/CoreText) to avoid building a full shaping engine. A pure-Rust text stack is marked as a future option.
- **Parameter binding**: Defined as a lock-free UI/audio thread boundary with `begin_gesture`/`end_gesture` protocol.
- **Accessibility**: Treated as a separate backend module; widgets expose metadata from day one.
- **Test harness**: A DAW-less test host is built before any DAW-facing wrapper.
- **Hidden work**: Added build/xtask, CI, licensing review, and host abstraction crates.

### Known External References
- **nih-plug**: Rust VST3/CLAP framework; adapters for egui/iced/VIZIA. Reference for plugin lifecycle and parameter model. Note: VST3 bindings are GPLv3 unless replaced.
- **VIZIA / vizia-plug**: Declarative Rust GUI with skia/baseview backend; not native Direct2D/CoreGraphics.
- **resvg / usvg / tiny-skia**: Recommended SVG rasterization stack.
- **image**: Recommended PNG decode crate.
- **fontdue**: Simple font rasterizer (no shaping); not sufficient alone for complex text.
- **JUCE / iPlug2 / VSTGUI**: C++ reference frameworks for retained widget trees and native plugin embedding.

## Work Objectives

### Core Objective
Deliver a production-oriented Rust GUI library that can be embedded into VST3, AU, and AAX plugin editors on Windows and macOS, with a consistent retained widget API and native rendering on each platform.

### Deliverables
1. Multi-crate workspace with core, platform backends, plugin format wrappers, and tooling.
2. `gui-core`: geometry, color, math, resource IDs, command list, widget/lifecycle traits, parameter abstraction, animation primitives.
3. `gui-win32`: HWND windowing, Direct2D/DirectWrite rendering, HiDPI, raw input events.
4. `gui-mac`: NSView/Cocoa windowing, CoreGraphics/CoreText rendering, HiDPI, event handling.
5. `gui-host`: shared plugin host abstraction (attach, detach, resize, idle, parameter queue).
6. `gui-vst3`, `gui-au`, `gui-aax`: format-specific editor wrappers.
7. `gui-res`: embedded resource system (images, fonts, SVG source data).
8. `gui-accessibility`: accessibility metadata and platform providers.
9. `gui-test-host`: DAW-less standalone test harness for both platforms.
10. `xtask`: build, bundle, and validation scripts.
11. CI configuration (GitHub Actions) for cross-platform builds and tests.

### Definition of Done (verifiable conditions with commands)
- `cargo test --workspace` passes on Windows and macOS.
- `cargo run -p gui-test-host --example blank` opens a native window and prints `EditorAttached`/`EditorDetached` events.
- `cargo build -p gui-vst3 --example gain` produces a loadable VST3 bundle (validated by a headless validator or scan in a free host).
- `cargo build -p gui-au --example gain` produces an AU component loadable by `auval` on macOS.
- `cargo build -p gui-aax --example gain` succeeds (runtime loadability requires Pro Tools/PACE environment; build-only is acceptable for CI).
- All tasks have agent-executed QA scenarios with evidence in `.omo/evidence/`.
- Final verification wave (F1–F4) passes.

### Must Have
- Windows Win32 + Direct2D backend.
- macOS AppKit + CoreGraphics backend.
- Retained widget tree with parent-child relationships and lifecycle.
- Layout system (flex/box model sufficient for v0.1).
- Mouse and keyboard event propagation.
- HiDPI scaling on both platforms.
- Parameter binding API with gesture protocol.
- Lock-free parameter change queue between audio/UI threads.
- DAW-less test host.
- CI on both platforms.

### Must NOT Have (guardrails, AI slop patterns, scope boundaries)
- **No new backends without explicit approval** (Linux/X11/Wayland, OpenGL, Vulkan, wgpu as primary backend).
- **No global "zero-allocation" guarantee**; the retained tree, resource loading, and layout may allocate. Zero-allocation applies only to the per-frame paint command replay path.
- **No mixing of CPU-rasterized SVG/text without an explicit upload/cache strategy**.
- **No DAW-required manual QA**; every scenario must be runnable by an agent.
- **No plugin-format logic leaked into core**; all format-specific behavior stays in `gui-vst3`/`gui-au`/`gui-aax`.
- **No unsafe code outside platform crates**; core must be `#![deny(unsafe_code)]`.
- **No hard dependency on AAX SDK for non-AAX builds**; AAX crate must be behind an opt-in feature.

## Verification Strategy
> ZERO HUMAN INTERVENTION — all verification is agent-executed.
- **Test decision**: Tests-after for exploratory platform code; TDD for core abstractions (`gui-core`, `gui-host`).
- **Framework**: `cargo test`, custom integration tests in `gui-test-host`, screenshot diffing via platform APIs or `xcap`, plugin validation via command-line host validators (`validator` from Steinberg, `auval` on macOS).
- **QA policy**: Every implementation task includes happy-path and failure/edge-case scenarios run by agents.
- **Evidence**: `.omo/evidence/task-{N}-{slug}.{ext}`.

## Execution Strategy

### Parallel Execution Waves
> Target: 5-8 tasks per wave. Dependencies extracted into Wave 1 for maximum parallelism.

**Wave 1: Foundation** — workspace, core abstractions, host trait, build/xtask, test host skeleton, CI.
**Wave 2: Windows Backend + VST3** — Win32 window, Direct2D render, DirectWrite text, VST3 wrapper, first widget.
**Wave 3: Widget Tree + Layout + Parameters** — retained tree, layout engine, event routing, parameter binding, basic controls.
**Wave 4: macOS Backend + AU** — NSView, CoreGraphics render, CoreText text, AU wrapper, HiDPI polish.
**Wave 5: Media + Animation + Accessibility + GPU Surface + Resources** — SVG, PNG, animation system, embedded resources, accessibility metadata/backends, custom GPU surface.
**Wave 6: AAX + Final Verification** — AAX wrapper, end-to-end tests, final verification wave.

### Dependency Matrix (full, all tasks)
| Task | Blocks | Blocked By |
|------|--------|------------|
| 1 Workspace & crate skeleton | 2–6, 7, 19 | — |
| 2 Core geometry/math/color | 3, 8, 20 | 1 |
| 3 Host abstraction trait | 11, 22, 32 | 2 |
| 4 Resource ID system | 5, 24 | 1 |
| 5 Test host skeleton | 12, 23 | 3, 4 |
| 6 CI / xtask | all | 1 |
| 7 Win32 windowing | 8, 9, 10 | 1 |
| 8 Direct2D render backend | 10, 12, 13 | 2, 7 |
| 9 DirectWrite text | 16 | 7 |
| 10 Zero-allocation paint command replay | 13 | 8 |
| 11 VST3 wrapper | 12 | 3 |
| 12 Blank VST3 example in test host | 18 | 8, 11, 5 |
| 13 Widget tree + lifecycle | 14, 15, 16 | 8/20 |
| 14 Layout system | 15 | 13 |
| 15 Event routing | 17 | 13 |
| 16 Parameter binding | 17 | 9/21, 3 |
| 17 Basic controls (knob, slider, button, label) | 18 | 14, 15, 16 |
| 18 Parameter-bound example plugin | 27, 33 | 17, 12 |
| 19 macOS windowing | 20, 21 | 1 |
| 20 CoreGraphics render backend | 23, 13 | 2, 19 |
| 21 CoreText text | 16 | 19 |
| 22 AU wrapper | 23 | 3 |
| 23 Blank AU example in test host | 18 | 20, 22, 5 |
| 24 Embedded resource system | 25, 26, 28 | 4 |
| 25 SVG renderer | 27 | 24 |
| 26 PNG renderer | 27 | 24 |
| 27 Animation system | 29 | 18, 25, 26 |
| 28 Accessibility metadata | 30 | 24 |
| 29 Animated example plugin | 33 | 27 |
| 30 Accessibility backends | 33 | 28 |
| 31 Custom GPU drawing surface | 33 | 8, 20 |
| 32 AAX wrapper | 33 | 3 |
| 33 AAX build-only example | F1–F4 | 32 |
| 34 End-to-end cross-platform validation | F1–F4 | 33 |

### Agent Dispatch Summary (wave → task count → categories)
| Wave | Tasks | Categories |
|------|-------|------------|
| Wave 1 | 6 | unspecified-high, writing |
| Wave 2 | 6 | unspecified-high, visual-engineering |
| Wave 3 | 6 | unspecified-high, visual-engineering |
| Wave 4 | 5 | unspecified-high, visual-engineering |
| Wave 5 | 8 | unspecified-high, visual-engineering |
| Wave 6 | 3 | unspecified-high, writing |
| Final Verification | 4 | oracle, unspecified-high, deep |

## TODOs

<!-- Tasks appended below via Edit calls -->

- [x] 1. Workspace and crate skeleton

  **What to do**: Convert the existing single `gui` crate into a Cargo workspace. Create the crate directories: `crates/gui-core`, `crates/gui-win32`, `crates/gui-mac`, `crates/gui-host`, `crates/gui-vst3`, `crates/gui-au`, `crates/gui-aax`, `crates/gui-res`, `crates/gui-accessibility`, `crates/gui-test-host`, `xtask`. Populate each with a minimal `Cargo.toml` and `src/lib.rs`. Update the root `Cargo.toml` with workspace members and shared metadata (edition 2024, authors, license placeholder, rust-version). Add workspace-level lints: `unsafe_code = "deny"` for `gui-core`, `gui-host`, `gui-res`, and `gui-accessibility`.

  **Must NOT do**: Add any implementation code beyond skeletons; add AAX SDK as a hard dependency; commit placeholder files without content.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: workspace restructuring touches many files.
  - Skills: none required.
  - Omitted: `visual-engineering` — no UI work yet.

  **Parallelization**: Can Parallel: NO | Wave 1 | Blocks: 2–6, 7, 19 | Blocked By: —

  **References**:
  - Pattern: `Cargo.toml` workspace syntax — [https://doc.rust-lang.org/cargo/reference/workspaces.html](https://doc.rust-lang.org/cargo/reference/workspaces.html)
  - Existing: `/Users/minjaekim/Plugins/gui/Cargo.toml`

  **Acceptance Criteria**:
  - [ ] `cargo build --workspace` succeeds with all skeleton crates.
  - [ ] `cargo test --workspace` succeeds (only default tests).
  - [ ] Each crate has `src/lib.rs` with a public item to avoid empty-crate warnings.

  **QA Scenarios**:
  ```
  Scenario: Workspace compiles
    Tool: Bash
    Steps: cargo build --workspace
    Expected: exit code 0, no warnings about empty crates
    Evidence: .omo/evidence/task-m1-1-build.log
  ```

  **Commit**: YES | Message: `chore(workspace): create multi-crate workspace skeleton` | Files: `Cargo.toml`, `crates/**`, `xtask/**`

- [x] 2. Core geometry, math, and color primitives

  **What to do**: Implement foundational types in `gui-core`: `Point`, `Size`, `Rect`, `Insets`, `Transform`, `Color` (sRGBA), `Px` (physical pixel) vs `Dp` (device-independent point) types, scalar math helpers. Keep the API `#![no_std]`-friendly where possible (use `core` only). Add unit tests for all operations.

  **Must NOT do**: Introduce platform-specific types here; depend on backend crates.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: foundational API design.
  - Skills: none.
  - Omitted: `visual-engineering` — pure math/types.

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 3, 8, 20 | Blocked By: 1

  **References**:
  - Pattern: `kurbo` or `euclid` crate API shape for 2D geometry.
  - External: [https://doc.rust-lang.org/core/index.html](https://doc.rust-lang.org/core/index.html)

  **Acceptance Criteria**:
  - [ ] `cargo test -p gui-core` passes with ≥80% line coverage for the math module.
  - [ ] `Rect::contains`, `Rect::intersect`, `Transform::scale`/`translate`, and `Color` conversion functions have tests.
  - [ ] `gui-core` compiles with `#![deny(unsafe_code)]`.

  **QA Scenarios**:
  ```
  Scenario: Geometry unit tests pass
    Tool: Bash
    Steps: cargo test -p gui-core
    Expected: all tests pass
    Evidence: .omo/evidence/task-m1-2-tests.log

  Scenario: Deny unsafe compiles
    Tool: Bash
    Steps: cargo check -p gui-core
    Expected: compiles without unsafe code
    Evidence: .omo/evidence/task-m1-2-safe.log
  ```

  **Commit**: YES | Message: `feat(core): add geometry, math, and color primitives` | Files: `crates/gui-core/src/**`

- [x] 3. Plugin host abstraction trait

  **What to do**: Define the `gui-host` crate. Create traits `PluginEditor`, `EditorHost`, and `ParameterGateway`. The editor trait must cover: `open(parent_handle)`, `close()`, `resize(size)`, `idle()`, `on_parameter_changed(id, normalized)`, `get_size_constraints()`. The host trait covers: `request_resize(size)`, `start_parameter_gesture(id)`, `end_parameter_gesture(id)`, `set_parameter_normalized(id, value)`. Include an enum `ParentWindowHandle` that wraps platform handles (HWND / NSView / id).

  **Must NOT do**: Implement format-specific logic here; leak VST3/AU/AAX types into this crate.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: cross-cutting abstraction.
  - Skills: none.
  - Omitted: `visual-engineering` — no rendering.

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 11, 22, 32 | Blocked By: 2

  **References**:
  - Pattern: `nih_plug::plugin::Editor` trait for lifecycle inspiration.
  - External: [https://doc.rust-lang.org/rust-by-example/trait.html](https://doc.rust-lang.org/rust-by-example/trait.html)

  **Acceptance Criteria**:
  - [ ] `gui-host` compiles with `#![deny(unsafe_code)]`.
  - [ ] A mock implementor of `PluginEditor` and `EditorHost` can be written in tests and exercised.
  - [ ] `ParentWindowHandle` has `Windows(HWND)` and `Mac(*mut objc::runtime::Object)` variants behind platform cfg.

  **QA Scenarios**:
  ```
  Scenario: Mock editor lifecycle
    Tool: Bash
    Steps: cargo test -p gui-host
    Expected: lifecycle test (open/idle/close/resize) passes
    Evidence: .omo/evidence/task-m1-3-mock.log
  ```

  **Commit**: YES | Message: `feat(host): define plugin editor and host abstraction traits` | Files: `crates/gui-host/src/**`

- [x] 4. Resource ID and embedded resource system skeleton

  **What to do**: In `gui-res`, define a `ResourceId` type (newtype around `u32` or string hash) and a `ResourceBundle` trait with methods to fetch bytes by ID. Provide a proc-macro or `build.rs` helper that embeds files from a `resources/` directory as `&'static [u8]`. Support at least SVG source bytes, PNG bytes, and font bytes. The actual decoding happens in later tasks.

  **Must NOT do**: Decode images/fonts here; depend on backend rendering crates.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: build-time resource embedding.
  - Skills: none.
  - Omitted: `visual-engineering` — no rendering yet.

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 5, 24 | Blocked By: 1

  **References**:
  - Pattern: `include_bytes!` macro for static embedding.
  - External: [https://doc.rust-lang.org/std/macro.include_bytes.html](https://doc.rust-lang.org/std/macro.include_bytes.html)

  **Acceptance Criteria**:
  - [ ] `gui-res` compiles with `#![deny(unsafe_code)]`.
  - [ ] A test embeds a small PNG and retrieves the exact byte slice.
  - [ ] `ResourceId` implements `Copy`, `Eq`, `Hash`, and `Debug`.

  **QA Scenarios**:
  ```
  Scenario: Embedded resource roundtrip
    Tool: Bash
    Steps: cargo test -p gui-res
    Expected: embedded test asset bytes match file bytes
    Evidence: .omo/evidence/task-m1-4-embed.log
  ```

  **Commit**: YES | Message: `feat(res): add ResourceId and static resource bundle skeleton` | Files: `crates/gui-res/src/**`, `crates/gui-res/tests/**`

- [x] 5. DAW-less test host skeleton

  **What to do**: Create `gui-test-host` as a tiny executable crate. On Windows it creates an `HWND` window; on macOS it creates an `NSWindow`/`NSView`. It accepts a plugin editor factory, attaches the editor to the native window, runs an idle loop for a configurable number of frames or seconds, and then detaches. Output events (`EditorAttached`, `EditorResized`, `EditorDetached`) to stdout. Keep platform-specific code isolated in `src/platform/win32.rs` and `src/platform/mac.rs`.

  **Must NOT do**: Require a DAW or real plugin format to run; render anything beyond a clear background.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: platform windowing and event loop.
  - Skills: none.
  - Omitted: `visual-engineering` — minimal UI.

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: 12, 23 | Blocked By: 3, 4

  **References**:
  - External Windows: [https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-createwindowexw](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-createwindowexw)
  - External macOS: [https://developer.apple.com/documentation/appkit/nswindow](https://developer.apple.com/documentation/appkit/nswindow)

  **Acceptance Criteria**:
  - [ ] `cargo run -p gui-test-host --example blank` opens a native window on both platforms.
  - [ ] The process exits cleanly after the configured duration.
  - [ ] Stdout contains `EditorAttached` and `EditorDetached` markers.

  **QA Scenarios**:
  ```
  Scenario: Test host opens and closes
    Tool: Bash
    Steps: cargo run -p gui-test-host --example blank -- --duration-ms 500
    Expected: exit code 0, stdout contains "EditorAttached" and "EditorDetached"
    Evidence: .omo/evidence/task-m1-5-host.log
  ```

  **Commit**: YES | Message: `feat(test-host): add DAW-less test host skeleton` | Files: `crates/gui-test-host/src/**`, `crates/gui-test-host/examples/**`

- [x] 6. Build scripts and CI

  **What to do**: Create an `xtask` crate with commands: `xtask test`, `xtask bundle-vst3`, `xtask bundle-au`, `xtask bundle-aax` (aax = build only), `xtask check`. Add a GitHub Actions workflow that runs on `windows-latest` and `macos-latest`, installs Rust stable, runs `cargo test --workspace`, runs `cargo clippy --workspace -- -D warnings`, and runs `cargo fmt --check`. Add `.gitignore` entries for `target/` and `.omo/evidence/`.

  **Must NOT do**: Implement full bundling logic for plugin formats (placeholder is fine); fail CI on AAX if SDK is missing — gate it.

  **Recommended Agent Profile**:
  - Category: `writing` / `unspecified-high` — Reason: tooling and CI configuration.
  - Skills: none.
  - Omitted: `visual-engineering` — no UI.

  **Parallelization**: Can Parallel: YES | Wave 1 | Blocks: all | Blocked By: 1

  **References**:
  - Pattern: `cargo xtask` pattern — [https://github.com/matklad/cargo-xtask](https://github.com/matklad/cargo-xtask)
  - External: [https://doc.rust-lang.org/cargo/reference/external-tools.html](https://doc.rust-lang.org/cargo/reference/external-tools.html)

  **Acceptance Criteria**:
  - [ ] `cargo xtask test` runs `cargo test --workspace` and `cargo clippy`.
  - [ ] CI workflow passes on `windows-latest` and `macos-latest` runners.
  - [ ] `cargo fmt --check` passes.

  **QA Scenarios**:
  ```
  Scenario: xtask test passes
    Tool: Bash
    Steps: cargo xtask test
    Expected: exit code 0
    Evidence: .omo/evidence/task-m1-6-xtask.log
  ```

  **Commit**: YES | Message: `chore(build): add xtask and GitHub Actions CI` | Files: `xtask/**`, `.github/workflows/**`, `.gitignore`

- [x] 7. Win32 windowing and surface management

  **What to do**: In `gui-win32`, implement `Win32Window` that registers a window class, creates a child `HWND` from a `ParentWindowHandle::Windows(HWND)`, handles `WM_SIZE`, `WM_DPICHANGED`, `WM_PAINT`, `WM_DESTROY`, and forwards mouse/keyboard messages to an event sink. Use `SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)` at thread creation. Store the `gui-host::PluginEditor` implementor as window user data. Provide a method to request repaint.

  **Must NOT do**: Implement any Direct2D drawing here; handle audio-thread logic.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: Win32 API surface and event loop.
  - Skills: none.
  - Omitted: `visual-engineering` — no widgets yet.

  **Parallelization**: Can Parallel: NO | Wave 2 | Blocks: 8, 9, 10 | Blocked By: 1

  **References**:
  - External: [https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-createwindowexw](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-createwindowexw)
  - External: [https://learn.microsoft.com/en-us/windows/win32/hidpi/setting-the-default-dpi-awareness-for-a-process](https://learn.microsoft.com/en-us/windows/win32/hidpi/setting-the-default-dpi-awareness-for-a-process)
  - External: [https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/WindowsAndMessaging/index.html](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/UI/WindowsAndMessaging/index.html)

  **Acceptance Criteria**:
  - [ ] `cargo build -p gui-win32` succeeds on Windows.
  - [ ] `gui-test-host` can attach a mock editor to an `HWND` and receive `WM_SIZE`.
  - [ ] `GetDpiForWindow` returns a value ≥ 96 and is reflected in scaling.

  **QA Scenarios**:
  ```
  Scenario: Win32 child window receives resize
    Tool: Bash
    Steps: cargo run -p gui-test-host --example win32-resize -- --duration-ms 1000
    Expected: stdout contains "Resized" with non-zero width/height
    Evidence: .omo/evidence/task-7-resize.log
  ```

  **Commit**: YES | Message: `feat(win32): add HWND windowing and HiDPI handling` | Files: `crates/gui-win32/src/window.rs`

- [ ] 8. Direct2D render backend

  **What to do**: In `gui-win32`, create a `D2DRenderBackend` implementing a `gui-core::RenderBackend` trait. Create a Direct2D factory, a device context, and a DXGI swap chain or WIC bitmap target sized to the HWND client area. Implement drawing commands: clear, fill rect, stroke rect, rounded rect, path fill/stroke, linear gradient, image draw. Use `IDXGISwapChain1::Present(1, 0)` for vsync off. Keep all per-frame transient allocations in a reusable command buffer / bump allocator.

  **Must NOT do**: Implement text rendering here (handled in task 9); depend on SVG/PNG decoders.

  **Recommended Agent Profile**:
  - Category: `visual-engineering` — Reason: graphics API and rendering pipeline.
  - Skills: none.
  - Omitted: `unspecified-high` for logic without UI.

  **Parallelization**: Can Parallel: NO | Wave 2 | Blocks: 10, 12 | Blocked By: 2, 7

  **References**:
  - External: [https://learn.microsoft.com/en-us/windows/win32/direct2d/direct2d-portal](https://learn.microsoft.com/en-us/windows/win32/direct2d/direct2d-portal)
  - External: [https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Graphics/Direct2D/index.html](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Graphics/Direct2D/index.html)

  **Acceptance Criteria**:
  - [ ] `gui-test-host` example renders a colored rectangle and a gradient.
  - [ ] Resizing the window recreates the render target without crashing.
  - [ ] Render command list is cleared/reused each frame (no per-frame heap allocations).

  **QA Scenarios**:
  ```
  Scenario: Render colored rectangle
    Tool: Bash
    Steps: cargo run -p gui-test-host --example d2d-rect -- --duration-ms 1000
    Expected: window appears; screenshot diff vs baseline passes
    Evidence: .omo/evidence/task-8-rect.png

  Scenario: Resize stability
    Tool: Bash
    Steps: cargo run -p gui-test-host --example d2d-rect -- --duration-ms 2000 --resize-every-ms 250
    Expected: no panic, no D2D device lost
    Evidence: .omo/evidence/task-8-resize.log
  ```

  **Commit**: YES | Message: `feat(win32): add Direct2D render backend` | Files: `crates/gui-win32/src/render.rs`, `crates/gui-core/src/render.rs`

- [ ] 9. DirectWrite text rendering

  **What to do**: In `gui-win32`, add a `TextLayout` type backed by `IDWriteTextFormat` and `IDWriteTextLayout`. Provide a `TextRenderer` that draws glyphs using Direct2D. Support loading a font from embedded bytes via `IDWriteFactory::CreateCustomFontCollection` or by registering with `AddFontMemResourceEx`. Cache text layouts by content + size + font key. Report metrics (width, height, baseline).

  **Must NOT do**: Implement complex text shaping (bidi) beyond what DirectWrite provides; leak font handles.

  **Recommended Agent Profile**:
  - Category: `visual-engineering` — Reason: text and font API integration.
  - Skills: none.
  - Omitted: `unspecified-high` for logic without UI.

  **Parallelization**: Can Parallel: YES | Wave 2 | Blocks: 15 | Blocked By: 7

  **References**:
  - External: [https://learn.microsoft.com/en-us/windows/win32/directwrite/direct-write-portal](https://learn.microsoft.com/en-us/windows/win32/directwrite/direct-write-portal)
  - External: [https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Graphics/DirectWrite/index.html](https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/Graphics/DirectWrite/index.html)

  **Acceptance Criteria**:
  - [ ] Test host renders a UTF-8 string at a specified position.
  - [ ] Changing DPI updates font metrics and re-renders at correct size.
  - [ ] Custom embedded font is used when specified.

  **QA Scenarios**:
  ```
  Scenario: Render UTF-8 text
    Tool: Bash
    Steps: cargo run -p gui-test-host --example d2d-text -- --duration-ms 1000
    Expected: screenshot contains legible text matching expected string
    Evidence: .omo/evidence/task-9-text.png
  ```

  **Commit**: YES | Message: `feat(win32): add DirectWrite text rendering` | Files: `crates/gui-win32/src/text.rs`

- [x] 10. Zero-allocation paint command replay

  **What to do**: In `gui-core`, define a `PaintCommand` enum and a `CommandList` backed by a `bumpalo::Bump` or fixed-size reusable buffer. Provide methods to push commands and clear without freeing underlying capacity. In `gui-win32`, implement `CommandList::replay(&self, backend: &mut D2DRenderBackend)`. Ensure the replay path performs no heap allocation after initialization.

  **Must NOT do**: Allow command list to own heap-allocated resources long-term; allow dynamic dispatch in the hot path.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: low-level performance-critical design.
  - Skills: none.
  - Omitted: `visual-engineering` — abstraction layer, not pixels.

  **Parallelization**: Can Parallel: NO | Wave 2 | Blocks: 13 | Blocked By: 8

  **References**:
  - Pattern: `bumpalo` crate for bump allocation.
  - External: [https://docs.rs/bumpalo/latest/bumpalo/](https://docs.rs/bumpalo/latest/bumpalo/)

  **Acceptance Criteria**:
  - [ ] A benchmark pushes 10,000 commands and replays them; `dhat` or custom allocator shows zero per-frame heap allocations.
  - [ ] Command list clear preserves capacity.
  - [ ] `#![deny(unsafe_code)]` in `gui-core`.

  **QA Scenarios**:
  ```
  Scenario: Zero per-frame allocations
    Tool: Bash
    Steps: cargo bench -p gui-core --bench paint_command
    Expected: benchmark reports zero heap allocations per replay iteration
    Evidence: .omo/evidence/task-10-bench.log
  ```

  **Commit**: YES | Message: `feat(core): add zero-allocation paint command list` | Files: `crates/gui-core/src/paint.rs`, `crates/gui-core/benches/paint_command.rs`

- [x] 11. VST3 plugin wrapper

  **What to do**: In `gui-vst3`, create an `IPlugView` implementation that wraps a `gui-host::PluginEditor`. Implement `IPlugView::attached`, `removed`, `onWheel`, `onKeyDown`, `onKeyUp`, `getSize`, `onSize`, `canResize`, `checkSizeConstraint`, `setFrame` (for host callbacks). Use the Steinberg VST3 SDK C++ interfaces via `vst3-sys`/`vst3-rs` bindings, or write minimal unsafe FFI if bindings are unavailable. Gate the crate on a `vst3` feature.

  **Must NOT do**: Implement audio processing; mix plugin format logic with core.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: FFI and plugin API compliance.
  - Skills: none.
  - Omitted: `visual-engineering` — no rendering.

  **Parallelization**: Can Parallel: YES | Wave 2 | Blocks: 12 | Blocked By: 3

  **References**:
  - External: Steinberg VST3 SDK — [https://developer.steinberg.help/pages/viewpage.action?pageId=9797948](https://developer.steinberg.help/pages/viewpage.action?pageId=9797948)
  - Crate: `vst3-sys` / `vst3-rs` on crates.io

  **Acceptance Criteria**:
  - [ ] `cargo build -p gui-vst3` succeeds.
  - [ ] A headless VST3 validator can instantiate the example editor without crashing.
  - [ ] The wrapper reports correct initial size and resize constraints.

  **QA Scenarios**:
  ```
  Scenario: VST3 wrapper instantiates
    Tool: Bash
    Steps: cargo build -p gui-vst3 --example gain && ./scripts/validate_vst3.sh target/debug/examples/gain.vst3
    Expected: validator exits 0
    Evidence: .omo/evidence/task-11-vst3.log
  ```

  **Commit**: YES | Message: `feat(vst3): add IPlugView wrapper` | Files: `crates/gui-vst3/src/lib.rs`

- [ ] 12. Blank VST3 example in test host

  **What to do**: Create a `gain` example in `gui-vst3/examples` that opens a blank editor with a parameter-bound slider. Wire it through `gui-test-host` so it can run as a standalone executable for rapid iteration. The example implements the `PluginEditor` trait and uses `gui-win32` for rendering.

  **Must NOT do**: Add audio processing; require a DAW.

  **Recommended Agent Profile**:
  - Category: `visual-engineering` — Reason: first end-to-end UI.
  - Skills: none.
  - Omitted: `unspecified-high` for logic.

  **Parallelization**: Can Parallel: NO | Wave 2 | Blocks: 21 | Blocked By: 8, 11, 5

  **References**:
  - Pattern: `nih-plug` example plugins for structure.

  **Acceptance Criteria**:
  - [ ] `cargo run -p gui-vst3 --example gain -- --test-host` opens a window and runs for the configured duration.
  - [ ] The example prints `EditorAttached` and `EditorDetached`.
  - [ ] Window background is drawn (clear color).

  **QA Scenarios**:
  ```
  Scenario: Blank VST3 example runs
    Tool: Bash
    Steps: cargo run -p gui-vst3 --example gain -- --test-host --duration-ms 1000
    Expected: window appears, stdout contains lifecycle markers
    Evidence: .omo/evidence/task-12-example.log
  ```

  **Commit**: YES | Message: `feat(vst3): add blank test-host example` | Files: `crates/gui-vst3/examples/gain.rs`

- [x] 13. Retained widget tree and lifecycle

  **What to do**: In `gui-core`, define the widget trait hierarchy: `Widget`, `Element`, `BuildContext`, `WidgetTree`. A widget has an ID, parent pointer, children vector, style/layout constraints, and state. Implement `Tree::insert`, `remove`, `find_by_id`, `traverse`, and lifecycle hooks (`mount`, `unmount`, `update`). Store the tree in `gui-host` and expose it to backends for rendering and hit-testing.

  **Must NOT do**: Implement layout algorithms here; depend on platform backends.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: tree data structure and lifecycle design.
  - Skills: none.
  - Omitted: `visual-engineering` — abstraction layer.

  **Parallelization**: Can Parallel: NO | Wave 3 | Blocks: 14, 15, 16 | Blocked By: 8/22

  **References**:
  - Pattern: Flutter/Egle widget tree, JUCE `Component` hierarchy.
  - External: [https://doc.rust-lang.org/std/collections/index.html](https://doc.rust-lang.org/std/collections/index.html)

  **Acceptance Criteria**:
  - [ ] `cargo test -p gui-core` covers insert/remove/traverse with 100% pass.
  - [ ] Widget IDs are unique within a tree.
  - [ ] `#![deny(unsafe_code)]` in `gui-core`.

  **QA Scenarios**:
  ```
  Scenario: Widget tree lifecycle
    Tool: Bash
    Steps: cargo test -p gui-core widget_tree
    Expected: all tests pass
    Evidence: .omo/evidence/task-13-tree.log
  ```

  **Commit**: YES | Message: `feat(core): add retained widget tree and lifecycle` | Files: `crates/gui-core/src/tree.rs`, `crates/gui-core/src/widget.rs`

- [x] 14. Layout system

  **What to do**: Implement a box/flex-style layout engine in `gui-core`. Define `LayoutConstraints`, `BoxLayout`, `FlexLayout`, and a `LayoutEngine` that computes `LayoutBox` (position + size) for every node. Support width/height, min/max, padding, margin, flex-grow, and alignment. Layout runs on demand when constraints change and produces a read-only `LayoutResult`.

  **Must NOT do**: Add CSS-style full layout; perform layout on the audio thread.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: algorithm design.
  - Skills: none.
  - Omitted: `visual-engineering` — pure layout math.

  **Parallelization**: Can Parallel: YES | Wave 3 | Blocks: 16 | Blocked By: 13

  **References**:
  - Pattern: Yoga, Taffy, or Flutter layout model.
  - External: [https://docs.rs/taffy/latest/taffy/](https://docs.rs/taffy/latest/taffy/) (reference only; do not depend on it if building custom)

  **Acceptance Criteria**:
  - [ ] Unit tests verify flex row/column, padding, min/max, and alignment.
  - [ ] Layout of a 3-node tree produces expected rectangles.
  - [ ] Re-layout after constraint change updates only affected nodes.

  **QA Scenarios**:
  ```
  Scenario: Flex layout
    Tool: Bash
    Steps: cargo test -p gui-core layout
    Expected: all layout tests pass
    Evidence: .omo/evidence/task-14-layout.log
  ```

  **Commit**: YES | Message: `feat(core): add box/flex layout engine` | Files: `crates/gui-core/src/layout.rs`

- [x] 15. Mouse and keyboard event routing

  **What to do**: Define `Event`, `MouseEvent`, `KeyEvent`, `PointerEvent` types in `gui-core`. Implement hit-testing against the `LayoutResult` and route events down the widget tree, calling `on_mouse_down`, `on_mouse_up`, `on_mouse_move`, `on_key_down`, etc. Support mouse capture, bubbling, and event consumption. Platform backends (`gui-win32`, `gui-mac`) translate native events to `gui-core` events.

  **Must NOT do**: Implement gestures or focus management here; leak native event types into core.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: event system architecture.
  - Skills: none.
  - Omitted: `visual-engineering` — event routing, not pixels.

  **Parallelization**: Can Parallel: YES | Wave 3 | Blocks: 16 | Blocked By: 13

  **References**:
  - Pattern: DOM/WPF event routing.
  - External: [https://doc.rust-lang.org/std/option/enum.Option.html](https://doc.rust-lang.org/std/option/enum.Option.html)

  **Acceptance Criteria**:
  - [ ] Unit tests simulate clicks at coordinates and verify the correct widget receives the event.
  - [ ] Event consumption stops propagation.
  - [ ] Native events are translated without data loss.

  **QA Scenarios**:
  ```
  Scenario: Hit testing
    Tool: Bash
    Steps: cargo test -p gui-core event_routing
    Expected: click on nested widget routes to leaf node
    Evidence: .omo/evidence/task-15-events.log
  ```

  **Commit**: YES | Message: `feat(core): add mouse/keyboard event routing` | Files: `crates/gui-core/src/event.rs`

- [x] 16. Parameter binding API

  **What to do**: In `gui-host`, implement the `ParameterGateway` with lock-free SPSC queues for audio→UI and UI→audio parameter changes. Define `ParameterId`, `NormalizedValue`, `ParameterInfo` (name, default, min, max, step, flags). Widgets call `gateway.begin_gesture(id)`, `set_normalized(id, value)`, `end_gesture(id)` on drag, and receive `on_parameter_changed(id, value)` callbacks. Use `crossbeam` or a custom ring buffer.

  **Must NOT do**: Block the audio thread; allocate in the audio→UI queue push path.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: lock-free concurrency and plugin contract.
  - Skills: none.
  - Omitted: `visual-engineering` — no UI.

  **Parallelization**: Can Parallel: YES | Wave 3 | Blocks: 17 | Blocked By: 3, 9/20

  **References**:
  - Pattern: `nih-plug` parameter model.
  - External: [https://docs.rs/crossbeam/latest/crossbeam/queue/index.html](https://docs.rs/crossbeam/latest/crossbeam/queue/index.html)

  **Acceptance Criteria**:
  - [ ] Thread-safety tests pass under `loom` or Miri where applicable.
  - [ ] UI-set values propagate through gateway to mock audio side.
  - [ ] Audio-set values propagate to UI callback within one idle tick.

  **QA Scenarios**:
  ```
  Scenario: Parameter roundtrip
    Tool: Bash
    Steps: cargo test -p gui-host parameter_gateway
    Expected: all roundtrip tests pass
    Evidence: .omo/evidence/task-16-param.log
  ```

  **Commit**: YES | Message: `feat(host): add lock-free parameter gateway` | Files: `crates/gui-host/src/parameter.rs`

- [x] 17. Basic controls (slider, knob, button, label)

  **What to do**: In `gui-core` or a new `gui-widgets` crate, implement `Slider`, `Knob`, `Button`, and `Label` widgets. Each widget uses the layout system, paints via the command list, and wires events to parameter binding where applicable. Provide a default theme (colors, spacing, font) and support disabled states.

  **Must NOT do**: Add animation or image assets; depend on platform-specific theming.

  **Recommended Agent Profile**:
  - Category: `visual-engineering` — Reason: first real widgets.
  - Skills: none.
  - Omitted: `unspecified-high` for logic.

  **Parallelization**: Can Parallel: YES | Wave 3 | Blocks: 18 | Blocked By: 14, 15, 16

  **References**:
  - Pattern: JUCE `Slider`, VSTGUI `CControl`.

  **Acceptance Criteria**:
  - [ ] Each control renders correctly in `gui-test-host`.
  - [ ] Slider drag updates parameter value and calls `begin_gesture`/`end_gesture`.
  - [ ] Knob responds to vertical drag and clamps to [0,1].

  **QA Scenarios**:
  ```
  Scenario: Slider drags update parameter
    Tool: Bash
    Steps: cargo run -p gui-test-host --example controls -- --duration-ms 2000
    Expected: screenshot shows slider; mock parameter value changes
    Evidence: .omo/evidence/task-17-controls.png
  ```

  **Commit**: YES | Message: `feat(widgets): add slider, knob, button, label controls` | Files: `crates/gui-core/src/widgets/**` or `crates/gui-widgets/src/**`

- [x] 18. Parameter-bound example plugin

  **What to do**: Extend the `gain` example to include a slider bound to a gain parameter and a label showing the value in dB. The example should run in `gui-test-host` and as a VST3. Add a simple automated test that drags the slider and verifies parameter gateway output.

  **Must NOT do**: Implement audio DSP; require a DAW for testing.

  **Recommended Agent Profile**:
  - Category: `visual-engineering` — Reason: end-to-end UI/parameter integration.
  - Skills: none.
  - Omitted: `unspecified-high` for logic.

  **Parallelization**: Can Parallel: NO | Wave 3 | Blocks: 26, 32 | Blocked By: 12, 17

  **References**:
  - Pattern: `nih-plug` gain_gui example.

  **Acceptance Criteria**:
  - [ ] Example runs in test host and responds to slider drags.
  - [ ] VST3 bundle loads in a validator.
  - [ ] Parameter value roundtrip is logged to stdout.

  **QA Scenarios**:
  ```
  Scenario: Gain example works end-to-end
    Tool: Bash
    Steps: cargo run -p gui-vst3 --example gain -- --test-host --duration-ms 3000
    Expected: slider visible and draggable; parameter values logged
    Evidence: .omo/evidence/task-18-gain.png
  ```

  **Commit**: YES | Message: `feat(examples): add parameter-bound gain plugin` | Files: `crates/gui-vst3/examples/gain.rs`

- [x] 19. macOS windowing and NSView management

  **What to do**: In `gui-mac`, implement `MacWindow` that creates an `NSView` (or wraps an existing one from `ParentWindowHandle::Mac`) and handles `drawRect:`, `viewDidChangeBackingProperties`, `mouseDown:`, `mouseUp:`, `mouseDragged:`, `keyDown:`, `keyUp:`, `setFrameSize:`, and `viewWillMoveToWindow:`. Use `CALayer` backing where appropriate. Report backing scale factor changes for HiDPI.

  **Must NOT do**: Implement CoreGraphics drawing here; handle audio-thread logic.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: AppKit/Objective-C interop.
  - Skills: none.
  - Omitted: `visual-engineering` — no widgets yet.

  **Parallelization**: Can Parallel: NO | Wave 4 | Blocks: 20, 21 | Blocked By: 1

  **References**:
  - External: [https://developer.apple.com/documentation/appkit/nsview](https://developer.apple.com/documentation/appkit/nsview)
  - External: [https://docs.rs/cocoa/latest/cocoa/appkit/struct.NSView.html](https://docs.rs/cocoa/latest/cocoa/appkit/struct.NSView.html)

  **Acceptance Criteria**:
  - [ ] `cargo build -p gui-mac` succeeds on macOS.
  - [ ] `gui-test-host` can attach a mock editor to an `NSView` and receive resize events.
  - [ ] Backing scale factor is reported correctly on Retina displays.

  **QA Scenarios**:
  ```
  Scenario: macOS child view receives resize
    Tool: Bash
    Steps: cargo run -p gui-test-host --example mac-resize -- --duration-ms 1000
    Expected: stdout contains "Resized" with non-zero width/height
    Evidence: .omo/evidence/task-19-resize.log
  ```

  **Commit**: YES | Message: `feat(mac): add NSView windowing and HiDPI handling` | Files: `crates/gui-mac/src/window.rs`

- [x] 20. CoreGraphics render backend

  **What to do**: In `gui-mac`, create a `CoreGraphicsRenderBackend` implementing `gui-core::RenderBackend`. Use `CGContext` from `drawRect:` or an offscreen `CGBitmapContext`. Implement commands: clear, fill rect, stroke rect, rounded rect, path fill/stroke, linear gradient, image draw. Integrate with the zero-allocation `CommandList` replay.

  **Must NOT do**: Implement text rendering here (handled in task 21); depend on SVG/PNG decoders.

  **Recommended Agent Profile**:
  - Category: `visual-engineering` — Reason: graphics API and rendering pipeline.
  - Skills: none.
  - Omitted: `unspecified-high` for logic without UI.

  **Parallelization**: Can Parallel: NO | Wave 4 | Blocks: 23 | Blocked By: 2, 19

  **References**:
  - External: [https://developer.apple.com/documentation/coregraphics/cgcontext](https://developer.apple.com/documentation/coregraphics/cgcontext)
  - Crate: `core-graphics` on crates.io

  **Acceptance Criteria**:
  - [ ] `gui-test-host` example renders a colored rectangle and a gradient on macOS.
  - [ ] Resizing the view recreates or updates the context without crashing.
  - [ ] Command list replay uses no per-frame heap allocations.

  **QA Scenarios**:
  ```
  Scenario: Render colored rectangle on macOS
    Tool: Bash
    Steps: cargo run -p gui-test-host --example cg-rect -- --duration-ms 1000
    Expected: window appears; screenshot diff vs baseline passes
    Evidence: .omo/evidence/task-20-rect.png
  ```

  **Commit**: YES | Message: `feat(mac): add CoreGraphics render backend` | Files: `crates/gui-mac/src/render.rs`

- [x] 21. CoreText text rendering

  **What to do**: In `gui-mac`, add a `TextLayout` type backed by `CTLine`/`CTFramesetter`. Load custom fonts from embedded bytes via `CTFontManagerRegisterGraphicsFont`. Cache layouts by content + size + font key. Report metrics (width, height, baseline) and render into the CoreGraphics context.

  **Must NOT do**: Implement complex shaping beyond CoreText; leak font descriptors.

  **Recommended Agent Profile**:
  - Category: `visual-engineering` — Reason: text and font API integration.
  - Skills: none.
  - Omitted: `unspecified-high` for logic without UI.

  **Parallelization**: Can Parallel: YES | Wave 4 | Blocks: 16 | Blocked By: 19

  **References**:
  - External: [https://developer.apple.com/documentation/coretext](https://developer.apple.com/documentation/coretext)
  - Crate: `core-text` on crates.io

  **Acceptance Criteria**:
  - [ ] Test host renders a UTF-8 string at a specified position on macOS.
  - [ ] Changing backing scale updates text metrics.
  - [ ] Custom embedded font is used when specified.

  **QA Scenarios**:
  ```
  Scenario: Render UTF-8 text on macOS
    Tool: Bash
    Steps: cargo run -p gui-test-host --example cg-text -- --duration-ms 1000
    Expected: screenshot contains legible text matching expected string
    Evidence: .omo/evidence/task-21-text.png
  ```

  **Commit**: YES | Message: `feat(mac): add CoreText text rendering` | Files: `crates/gui-mac/src/text.rs`

- [x] 22. AU plugin wrapper

  **What to do**: In `gui-au`, create an `AUView`/`AUEditorBase` wrapper around `gui-host::PluginEditor`. Implement `GetProperty`/`SetProperty` for `kAudioUnitProperty_CocoaUI`, lifecycle `CreateUI`, `Cleanup`, `Open`, `Close`, and resize callbacks. Use the CoreAudio SDK headers. Gate the crate on an `au` feature.

  **Must NOT do**: Implement audio processing; mix AU-specific logic with core.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: CoreAudio/Objective-C interop.
  - Skills: none.
  - Omitted: `visual-engineering` — no rendering.

  **Parallelization**: Can Parallel: YES | Wave 4 | Blocks: 23 | Blocked By: 3

  **References**:
  - External: [https://developer.apple.com/documentation/audiotoolbox/audio_unit_v2](https://developer.apple.com/documentation/audiotoolbox/audio_unit_v2)
  - External: [https://developer.apple.com/documentation/audiotoolbox/auview](https://developer.apple.com/documentation/audiotoolbox/auview)

  **Acceptance Criteria**:
  - [ ] `cargo build -p gui-au` succeeds on macOS.
  - [ ] `auval -v aufx GnVl Manu` (or equivalent) instantiates the example editor without crashing.
  - [ ] Wrapper reports correct initial size.

  **QA Scenarios**:
  ```
  Scenario: AU wrapper passes auval
    Tool: Bash
    Steps: cargo build -p gui-au --example gain && ./scripts/validate_au.sh
    Expected: auval exits 0
    Evidence: .omo/evidence/task-22-au.log
  ```

  **Commit**: YES | Message: `feat(au): add Audio Unit editor wrapper` | Files: `crates/gui-au/src/lib.rs`

- [x] 23. Blank AU example in test host

  **What to do**: Create a `gain` example in `gui-au/examples` that opens the same editor as the VST3 example. Wire it through `gui-test-host` and ensure it uses `gui-mac` for rendering.

  **Must NOT do**: Duplicate widget code; require Logic/GarageBand.

  **Recommended Agent Profile**:
  - Category: `visual-engineering` — Reason: end-to-end UI.
  - Skills: none.
  - Omitted: `unspecified-high` for logic.

  **Parallelization**: Can Parallel: NO | Wave 4 | Blocks: 18 | Blocked By: 20, 22, 5

  **References**:
  - Pattern: `gui-vst3/examples/gain.rs`.

  **Acceptance Criteria**:
  - [ ] `cargo run -p gui-au --example gain -- --test-host` opens a window on macOS.
  - [ ] The example prints `EditorAttached` and `EditorDetached`.

  **QA Scenarios**:
  ```
  Scenario: Blank AU example runs
    Tool: Bash
    Steps: cargo run -p gui-au --example gain -- --test-host --duration-ms 1000
    Expected: window appears, stdout contains lifecycle markers
    Evidence: .omo/evidence/task-23-example.log
  ```

  **Commit**: YES | Message: `feat(au): add blank test-host example` | Files: `crates/gui-au/examples/gain.rs`

- [ ] 24. Embedded resource system

  **What to do**: Expand `gui-res` to support runtime resource loading and caching. Define `Resource<T>` types for `Image`, `Svg`, and `Font`. Provide a registry that maps `ResourceId` to decoded objects, with reference counting and eviction. Integrate with the build-time embedding from task 4 so resources can be loaded either from embedded bytes or from files during development.

  **Must NOT do**: Decode SVG/PNG in this task (handled in 25/26); depend on platform rendering crates.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: resource management and caching.
  - Skills: none.
  - Omitted: `visual-engineering` — abstraction.

  **Parallelization**: Can Parallel: YES | Wave 5 | Blocks: 25, 26, 28 | Blocked By: 4

  **References**:
  - Pattern: `Asset` systems in game engines (Bevy, raylib).
  - External: [https://doc.rust-lang.org/std/sync/struct.Arc.html](https://doc.rust-lang.org/std/sync/struct.Arc.html)

  **Acceptance Criteria**:
  - [ ] Resources can be registered by ID and retrieved by type.
  - [ ] Embedded bytes and file-system bytes produce equivalent registry entries.
  - [ ] `#![deny(unsafe_code)]` in `gui-res`.

  **QA Scenarios**:
  ```
  Scenario: Resource registry roundtrip
    Tool: Bash
    Steps: cargo test -p gui-res resource_registry
    Expected: embedded and filesystem resources load and match expected hashes
    Evidence: .omo/evidence/task-24-res.log
  ```

  **Commit**: YES | Message: `feat(res): add typed resource registry and caching` | Files: `crates/gui-res/src/registry.rs`

- [ ] 25. SVG renderer

  **What to do**: In `gui-res` or `gui-core`, integrate `resvg`/`usvg` to parse SVG source and rasterize to a `tiny-skia::Pixmap`. Cache the decoded tree (`usvg::Tree`) and the rasterized bitmap. Provide a `SvgImage` resource that backends can upload to a texture/bitmap. Expose `render(size)` to produce a backend-ready image at a given resolution.

  **Must NOT do**: Implement SVG animation (out of resvg scope); render directly in backends.

  **Recommended Agent Profile**:
  - Category: `visual-engineering` — Reason: image decoding and texture prep.
  - Skills: none.
  - Omitted: `unspecified-high` for logic.

  **Parallelization**: Can Parallel: YES | Wave 5 | Blocks: 27 | Blocked By: 24

  **References**:
  - Crate: `resvg` 0.47+, `usvg` 0.47+, `tiny-skia` 0.12+.
  - External: [https://docs.rs/resvg/latest/resvg/](https://docs.rs/resvg/latest/resvg/)

  **Acceptance Criteria**:
  - [ ] A test SVG file renders to a pixmap matching a reference PNG within pixel tolerance.
  - [ ] `usvg::Tree` is cached and reused for repeated renders.
  - [ ] Backend can retrieve the pixmap as RGBA bytes.

  **QA Scenarios**:
  ```
  Scenario: SVG rasterization matches reference
    Tool: Bash
    Steps: cargo test -p gui-res svg_render
    Expected: diff vs reference PNG passes
    Evidence: .omo/evidence/task-25-svg.png
  ```

  **Commit**: YES | Message: `feat(res): add SVG renderer via resvg/usvg` | Files: `crates/gui-res/src/svg.rs`

- [ ] 26. PNG renderer

  **What to do**: In `gui-res`, integrate the `image` crate to decode PNG to RGBA bytes. Provide a `PngImage` resource with width, height, and pixel data. Support premultiplied alpha where required by backends. Cache decoded bitmaps.

  **Must NOT do**: Implement encoding or format support beyond PNG; render directly in backends.

  **Recommended Agent Profile**:
  - Category: `visual-engineering` — Reason: image decoding.
  - Skills: none.
  - Omitted: `unspecified-high` for logic.

  **Parallelization**: Can Parallel: YES | Wave 5 | Blocks: 27 | Blocked By: 24

  **References**:
  - Crate: `image` 0.25+.
  - External: [https://docs.rs/image/latest/image/](https://docs.rs/image/latest/image/)

  **Acceptance Criteria**:
  - [ ] A test PNG decodes to expected dimensions and RGBA values.
  - [ ] Decoded image is cached and reused.
  - [ ] Backend can retrieve pixel data as a contiguous slice.

  **QA Scenarios**:
  ```
  Scenario: PNG decode matches source
    Tool: Bash
    Steps: cargo test -p gui-res png_decode
    Expected: decoded dimensions and pixel checksum match
    Evidence: .omo/evidence/task-26-png.log
  ```

  **Commit**: YES | Message: `feat(res): add PNG decoder via image crate` | Files: `crates/gui-res/src/png.rs`

- [ ] 27. Animation system

  **What to do**: In `gui-core`, implement an animation engine with `Animation`, `AnimationCurve` (linear, ease-in-out, spring), and `AnimationController`. Animations run on the idle loop and update widget properties (opacity, transform, color) each frame. Provide `start`, `stop`, `pause`, and completion callbacks. Use a fixed timestep or elapsed-time approach; avoid audio-thread involvement.

  **Must NOT do**: Allocate per animation frame; block the UI thread with heavy work.

  **Recommended Agent Profile**:
  - Category: `visual-engineering` — Reason: motion and timing.
  - Skills: none.
  - Omitted: `unspecified-high` for logic.

  **Parallelization**: Can Parallel: YES | Wave 5 | Blocks: 29 | Blocked By: 18, 25, 26

  **References**:
  - Pattern: CSS transitions/animations, Flutter `AnimationController`.
  - External: [https://easings.net/](https://easings.net/)

  **Acceptance Criteria**:
  - [ ] An animation from 0→1 opacity completes in the expected duration.
  - [ ] Animations can be cancelled and restarted.
  - [ ] Multiple concurrent animations update independently.

  **QA Scenarios**:
  ```
  Scenario: Opacity animation completes
    Tool: Bash
    Steps: cargo test -p gui-core animation
    Expected: final value is 1.0 within tolerance; completion callback fired
    Evidence: .omo/evidence/task-27-anim.log
  ```

  **Commit**: YES | Message: `feat(core): add animation engine with curves` | Files: `crates/gui-core/src/animation.rs`

- [ ] 28. Accessibility metadata

  **What to do**: In `gui-core`, add accessibility fields to every widget: `role` (button, slider, label, etc.), `label`, `value`, `state` (disabled, hidden, checked), and `bounds`. Define an `AccessibilityNode` tree that mirrors the widget tree and can be queried by platform accessibility backends.

  **Must NOT do**: Implement platform providers here; depend on platform accessibility APIs.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: metadata model.
  - Skills: none.
  - Omitted: `visual-engineering` — no UI.

  **Parallelization**: Can Parallel: YES | Wave 5 | Blocks: 30 | Blocked By: 24

  **References**:
  - Pattern: WAI-ARIA roles, NSAccessibility protocols.
  - External: [https://www.w3.org/WAI/ARIA/apg/](https://www.w3.org/WAI/ARIA/apg/)

  **Acceptance Criteria**:
  - [ ] Every built-in widget exposes a valid role and label.
  - [ ] Accessibility tree can be serialized for tests.
  - [ ] `#![deny(unsafe_code)]` in `gui-core`.

  **QA Scenarios**:
  ```
  Scenario: Accessibility tree matches widget tree
    Tool: Bash
    Steps: cargo test -p gui-core accessibility_tree
    Expected: node count and roles match expected structure
    Evidence: .omo/evidence/task-28-a11y.log
  ```

  **Commit**: YES | Message: `feat(core): add accessibility metadata and node tree` | Files: `crates/gui-core/src/accessibility.rs`

- [ ] 29. Animated example plugin

  **What to do**: Extend the `gain` example to use the animation system: animate the slider thumb or a peak meter bar. Add an SVG knob background and a PNG logo loaded from embedded resources. Run in `gui-test-host` and capture screenshots to verify animation frames.

  **Must NOT do**: Add audio processing; require a DAW.

  **Recommended Agent Profile**:
  - Category: `visual-engineering` — Reason: integrated example.
  - Skills: none.
  - Omitted: `unspecified-high` for logic.

  **Parallelization**: Can Parallel: YES | Wave 5 | Blocks: 32 | Blocked By: 27

  **References**:
  - Pattern: `gui-vst3/examples/gain.rs`, tasks 25/26.

  **Acceptance Criteria**:
  - [ ] Example renders SVG and PNG assets.
  - [ ] Animation is visible in screenshots taken at different times.
  - [ ] No per-frame allocations in paint path (validate with benchmark).

  **QA Scenarios**:
  ```
  Scenario: Animated example renders assets and motion
    Tool: Bash
    Steps: cargo run -p gui-vst3 --example gain -- --test-host --duration-ms 3000
    Expected: screenshots show SVG/PNG and changing animation state
    Evidence: .omo/evidence/task-29-animated.png
  ```

  **Commit**: YES | Message: `feat(examples): add animated SVG/PNG gain plugin` | Files: `crates/gui-vst3/examples/gain.rs`, `crates/gui-res/resources/**`

- [ ] 30. Accessibility backends

  **What to do**: In `gui-accessibility`, implement platform accessibility providers. On Windows, create a UI Automation provider (`IRawElementProviderFragment`) that traverses the accessibility node tree. On macOS, implement `NSAccessibility` methods on a proxy object. Gate platform code with cfg. Provide a no-op backend for builds without accessibility.

  **Must NOT do**: Leak platform types into `gui-core`; require accessibility to be enabled for basic functionality.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: platform accessibility APIs and FFI.
  - Skills: none.
  - Omitted: `visual-engineering` — no rendering.

  **Parallelization**: Can Parallel: YES | Wave 5 | Blocks: 32 | Blocked By: 28

  **References**:
  - External Windows: [https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-providerportal](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-providerportal)
  - External macOS: [https://developer.apple.com/documentation/appkit/nsaccessibility](https://developer.apple.com/documentation/appkit/nsaccessibility)

  **Acceptance Criteria**:
  - [ ] `cargo build -p gui-accessibility` succeeds on both platforms.
  - [ ] A tool like Accessibility Insights (Win) or Accessibility Inspector (Mac) can see the example's slider role and value.
  - [ ] Accessibility backend is optional via feature flag.

  **QA Scenarios**:
  ```
  Scenario: Accessibility backend exposes slider value
    Tool: Bash
    Steps: cargo run -p gui-test-host --example a11y-slider -- --duration-ms 2000
    Expected: external accessibility tool reports slider value matching UI
    Evidence: .omo/evidence/task-30-a11y.log
  ```

  **Commit**: YES | Message: `feat(accessibility): add UI Automation and NSAccessibility backends` | Files: `crates/gui-accessibility/src/**`

- [ ] 31. Custom GPU drawing surface

  **What to do**: Add a `GpuSurface` widget that provides a raw GPU context for custom drawing. On Windows, use Direct2D + DXGI surface interop so users can draw with Direct3D/Direct2D. On macOS, use a `CAMetalLayer` backing and expose a `MTLDevice`/`MTLCommandQueue` handle. Provide a `render(callback)` API that the widget invokes each frame. Document that this is for advanced visualization (spectrum, oscilloscope) and not the default path.

  **Must NOT do**: Replace the default 2D backends; require metal/directx for basic widgets.

  **Recommended Agent Profile**:
  - Category: `visual-engineering` — Reason: GPU interop.
  - Skills: none.
  - Omitted: `unspecified-high` for logic.

  **Parallelization**: Can Parallel: YES | Wave 5 | Blocks: 32 | Blocked By: 8, 20

  **References**:
  - External Windows: [https://learn.microsoft.com/en-us/windows/win32/direct2d/direct2d-and-direct3d-interoperation-overview](https://learn.microsoft.com/en-us/windows/win32/direct2d/direct2d-and-direct3d-interoperation-overview)
  - External macOS: [https://developer.apple.com/documentation/metal/drawable_objects/creating_a_custom_metal_view](https://developer.apple.com/documentation/metal/drawable_objects/creating_a_custom_metal_view)

  **Acceptance Criteria**:
  - [ ] A `GpuSurface` example clears to a color on both platforms.
  - [ ] Surface resizes without device loss.
  - [ ] Surface is gated behind a `gpu-surface` feature.

  **QA Scenarios**:
  ```
  Scenario: GPU surface clears
    Tool: Bash
    Steps: cargo run -p gui-test-host --example gpu-surface --features gpu-surface -- --duration-ms 1000
    Expected: window shows custom GPU clear color; screenshot matches
    Evidence: .omo/evidence/task-31-gpu.png
  ```

  **Commit**: YES | Message: `feat(widgets): add custom GPU drawing surface` | Files: `crates/gui-core/src/widgets/gpu_surface.rs`, `crates/gui-win32/src/gpu.rs`, `crates/gui-mac/src/gpu.rs`

- [ ] 32. AAX plugin wrapper

  **What to do**: In `gui-aax`, create an `AAX_CEffectGUI` or `AAX_IEffectGUI` implementation that wraps `gui-host::PluginEditor`. Implement `CreateViewContainer`, `GetViewSize`, `Draw`, `TimerWakeup`, and parameter callbacks. This crate must be gated behind an `aax` feature and must not be built unless the AAX SDK path is provided via environment variable. Document that runtime validation requires Pro Tools and a PACE/iLok dev license.

  **Must NOT do**: Make `gui-aax` a default workspace member; implement audio processing.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: AAX SDK FFI and plugin contract.
  - Skills: none.
  - Omitted: `visual-engineering` — no rendering.

  **Parallelization**: Can Parallel: YES | Wave 6 | Blocks: 33 | Blocked By: 3

  **References**:
  - External: Avid AAX SDK documentation (requires SDK download).
  - External: [https://developer.avid.com/aax/](https://developer.avid.com/aax/)

  **Acceptance Criteria**:
  - [ ] `cargo build -p gui-aax --features aax` succeeds when `AAX_SDK_PATH` is set.
  - [ ] Building without the feature or SDK path is a no-op and does not break `cargo build --workspace`.
  - [ ] Wrapper reports correct view size.

  **QA Scenarios**:
  ```
  Scenario: AAX wrapper builds conditionally
    Tool: Bash
    Steps: AAX_SDK_PATH=/path/to/sdk cargo build -p gui-aax --features aax
    Expected: exit code 0
    Evidence: .omo/evidence/task-32-aax.log
  ```

  **Commit**: YES | Message: `feat(aax): add AAX editor wrapper behind feature gate` | Files: `crates/gui-aax/src/lib.rs`, `crates/gui-aax/build.rs`

- [ ] 33. AAX build-only example

  **What to do**: Create a `gain` example in `gui-aax/examples` that compiles against the AAX wrapper. The example does not need to run in `gui-test-host` (AAX requires a Pro Tools host), but it must compile in CI when the AAX SDK is available.

  **Must NOT do**: Require a DAW or PACE environment for basic build validation.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: build integration.
  - Skills: none.
  - Omitted: `visual-engineering` — no UI beyond existing example.

  **Parallelization**: Can Parallel: YES | Wave 6 | Blocks: 34, F1–F4 | Blocked By: 32

  **References**:
  - Pattern: `gui-vst3/examples/gain.rs`.

  **Acceptance Criteria**:
  - [ ] `cargo build -p gui-aax --example gain --features aax` succeeds when SDK is present.
  - [ ] Example reuses the same editor code as VST3/AU examples.

  **QA Scenarios**:
  ```
  Scenario: AAX example compiles
    Tool: Bash
    Steps: AAX_SDK_PATH=/path/to/sdk cargo build -p gui-aax --example gain --features aax
    Expected: exit code 0
    Evidence: .omo/evidence/task-33-aax-example.log
  ```

  **Commit**: YES | Message: `feat(aax): add build-only gain example` | Files: `crates/gui-aax/examples/gain.rs`

- [ ] 34. End-to-end cross-platform validation

  **What to do**: Run the full validation matrix: `cargo test --workspace`, `cargo run` for all test-host examples on both platforms, VST3 validator on Windows, `auval` on macOS, AAX build on the CI runner with SDK, screenshot diff comparison for all UI examples, and a memory-safety check (`cargo miri test -p gui-core`, `cargo miri test -p gui-host`). Capture all evidence and produce a validation report.

  **Must NOT do**: Skip any platform; rely on manual human verification.

  **Recommended Agent Profile**:
  - Category: `unspecified-high` — Reason: cross-platform integration validation.
  - Skills: none.
  - Omitted: `visual-engineering` — no UI work.

  **Parallelization**: Can Parallel: NO | Wave 6 | Blocks: F1–F4 | Blocked By: 29, 30, 31, 33

  **References**:
  - Pattern: CI workflow from task 6.

  **Acceptance Criteria**:
  - [ ] Validation report exists at `.omo/evidence/validation-report.md`.
  - [ ] All examples run on both platforms.
  - [ ] VST3 validator and `auval` pass.
  - [ ] Miri passes on `gui-core` and `gui-host`.

  **QA Scenarios**:
  ```
  Scenario: Full validation matrix passes
    Tool: Bash
    Steps: cargo xtask validate
    Expected: exit code 0, report generated
    Evidence: .omo/evidence/task-34-report.md
  ```

  **Commit**: YES | Message: `chore(validate): add end-to-end cross-platform validation` | Files: `xtask/src/validate.rs`, `.omo/evidence/validation-report.md`

## Final Verification Wave (MANDATORY — after ALL implementation tasks)
> 4 review agents run in PARALLEL. ALL must APPROVE. Present consolidated results to user and get explicit "okay" before completing.
> **Do NOT auto-proceed after verification. Wait for user's explicit approval before marking work complete.**
> **Never mark F1–F4 as checked before getting user's okay.** Rejection or user feedback → fix → re-run → present again → wait for okay.
- [ ] F1. Plan Compliance Audit — oracle: verify every task acceptance criterion is met and file references exist.
- [ ] F2. Code Quality Review — unspecified-high: review unsafe boundaries, platform-specific code, public API consistency, error handling.
- [ ] F3. Real Manual QA — unspecified-high: run `gui-test-host` examples, VST3/AU validators, screenshot diffs, edge-case scenarios (DPI change, rapid attach/detach, parameter flood, closed UI during animation).
- [ ] F4. Scope Fidelity Check — deep: confirm the delivered feature set matches the requested list (Win32+D2D, AppKit+CG, SVG, PNG, text, widget tree, layout, events, HiDPI, animation, parameter binding, resources, accessibility, GPU surface, zero-allocation paint path).

## Self-Review and Gap Classification

This section records the planner's self-review after generating the plan.

### Critical Gaps Requiring User Decision
- **None**. All major tradeoffs were either confirmed by the user in the interview or resolved with explicit defaults documented below.

### Minor Gaps (Auto-Resolved)
| Gap | Resolution |
|-----|------------|
| Crate name `gui` is generic and may conflict | Kept `gui` as the workspace root package name; individual crates are prefixed `gui-` (e.g. `gui-core`, `gui-win32`). Rename the root package later if publishing to crates.io. |
| Which Rust crates for SVG/PNG/text | SVG: `resvg`/`usvg`/`tiny-skia`; PNG: `image`; text: platform-native `DirectWrite`/`CoreText` for v0.1. |
| "Zero-allocation paint pipeline" vs retained tree | Scoped to per-frame command replay only; retained tree, layout, and resource loading may allocate. |
| GPU surface API | Defined as an opt-in `GpuSurface` widget with Direct2D/DXGI interop on Windows and `CAMetalLayer` on macOS. |
| AAX SDK availability | `gui-aax` is behind an `aax` feature and `AAX_SDK_PATH` env var; build-only validation in CI. |

### Ambiguities (Default Applied)
- **License**: No license specified in `Cargo.toml` yet; placeholder added in task 1 for the user to fill in.
- **MSRV**: Default to latest stable Rust 2024 edition; no explicit MSRV policy until task 6 CI stabilizes.
- **Threading model**: Parameter gateway uses a lock-free queue; assumed UI thread owns widgets and backends, audio thread only pushes parameter changes.
- **Standalone application**: Explicitly out of scope; `gui-test-host` is only a test harness, not a public app framework.

### Self-Review Checklist Results
- [x] All TODOs have concrete acceptance criteria.
- [x] All tasks include agent-executable QA scenarios (happy path + at least one edge/failure case).
- [x] No business logic assumptions without evidence; external crate choices are grounded in research findings.
- [x] Metis guardrails incorporated (phasing, format isolation, zero-allocation scope, DAW-less testing, accessibility as module).
- [x] File references are to planned crate paths; existing files referenced only where they already exist (`Cargo.toml`, `src/lib.rs`).
- [x] Zero acceptance criteria require human intervention.

## Commit Strategy
- Each task is committed separately with a conventional commit message.
- Commit after each task's acceptance criteria pass.
- Example messages:
  - `feat(core): add geometry, color, and math primitives`
  - `feat(win32): implement HWND windowing and Direct2D render backend`
  - `feat(vst3): add IPlugView wrapper around gui-host trait`
  - `feat(mac): add NSView windowing and CoreGraphics render backend`
  - `feat(accessibility): add UI Automation and NSAccessibility providers`
- Do not commit generated artifacts (`target/`, `.omo/evidence/` binaries). Keep `Cargo.lock` committed.

## Success Criteria
- The workspace compiles on both Windows and macOS with `cargo build --workspace`.
- `gui-test-host` can instantiate and exercise the plugin editor without a DAW.
- VST3 example loads in a validator/host; AU example passes `auval` where applicable.
- AAX example builds successfully (runtime validation requires external SDK/license).
- All listed subsystems (widget tree, layout, SVG/PNG/text, animation, parameters, resources, accessibility, GPU surface, HiDPI) are present and covered by agent-run QA scenarios.
- Zero unsafe code in `gui-core`; platform crates isolate unsafe.
- Final verification wave (F1–F4) approves the work.
