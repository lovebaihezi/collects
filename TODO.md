# TODO — Multi-platform refactor roadmap (winit + egui + wgpu, FlatBuffers ABI, host-driven frames)

This document tracks the refactor needed to support a clean multi-platform architecture where:

- The **core app + UI (egui)** is platform-agnostic.
- Each platform has its own **shell** (desktop runner, web runner, native wrappers).
- Rendering is done via **wgpu**.
- Cross-language/native wrapper integration uses **FlatBuffers** over a stable **C ABI**.
- The rendering loop is **host-driven** (native host decides when `frame()` is called).
- **eframe is removed** (no eframe dependency in the final architecture).
- WASM support remains (Android/iOS support later; not P0).

---

## Goals (what “done” looks like)

### Architecture goals
- `collects-ui` becomes a pure library crate (no `main.rs`, no platform deps).
- A new `collects-engine` crate owns:
  - the egui context + frame orchestration
  - conversion between input events and `egui::RawInput`
  - command/event queues
  - generation of paint output (meshes, textures) for wgpu rendering
- Platform shells:
  - `apps/desktop` uses `winit + wgpu + egui_winit + egui_wgpu`
  - `apps/web` uses web bindings + wgpu (WebGPU) or fallback path as needed
  - `crates/ffi` exposes C ABI functions for native wrappers
  - `native/*` projects (Swift/Kotlin/C#/etc.) call into the FFI library and drive frames
- Platform-specific code is confined to platform crates/projects (not in core crates).

### ABI/FFI goals
- FFI boundary uses **FlatBuffers** messages for:
  - Host → Rust events (input, lifecycle, clipboard responses, etc.)
  - Rust → Host commands (open URL, pick file, set clipboard, etc.)
  - Rust → Host render output metadata (texture updates, buffers, etc.) as needed
- ABI is **stable, versioned, and backwards compatible** (schema evolution plan).

### Developer experience goals
- Rust Analyzer autocomplete works per platform because platform crates have distinct targets/deps.
- Minimal `cfg(...)` in core crates.
- Clean dependency graph: core crates do not depend on OS/web bindings directly.

---

## Non-goals (for now)
- iOS/Android native wrappers (planned, not P0).
- System tray integration.
- Full native menu support (only add when required; macOS may be special later).
- Perfectly zero-copy everything across FFI on day one (correctness first).

---

## Current pain points (to eliminate)
- Platform bootstrap mixed into `collects-ui` (native + wasm `main`).
- Platform deps (`eframe`, windowing, wasm bindings, clipboard impls) leak into UI crate.
- Difficult to reason about platform-specific behavior and feature gating.
- Hard to expand to mobile-native wrappers cleanly.

---

## Target workspace structure (proposal)

```/dev/null/collects_layout.txt#L1-60
collects/
  Cargo.toml                       # workspace
  crates/
    platform-api/                  # traits + shared types: events/commands/capabilities
    engine/                        # egui orchestration + queues + per-frame pipeline
    ui/                            # egui widgets/panels; platform-agnostic
    ffi/                           # C ABI + FlatBuffers encode/decode + opaque handles
  apps/
    desktop/                       # winit + wgpu runner (dev + production desktop)
    web/                           # wasm/web runner (keep wasm support)
  native/
    macos/                         # Swift/Xcode wrapper (later)
    ios/                           # Swift wrapper (later)
    android/                       # Kotlin wrapper (later)
    windows/                       # C#/WinUI wrapper (optional later)
```

Notes:
- Existing crates like `business`, `states`, `utils`, `services` remain, but dependencies should flow “inward” (platform shells depend on core; core does not depend on shells).
- `collects/ui` crate may be renamed to `crates/ui` eventually, but we can migrate incrementally.

---

## Key design decisions

### 1) Host-driven frames
The host calls into Rust each frame:
- provides time, dt, size, scale/DPI, focus, etc.
- provides a batch of input events since last frame
- Rust returns:
  - paint output (to render with wgpu)
  - “platform commands” for the host to execute

Host is responsible for:
- scheduling frames (vsync/timer, request redraw)
- presenting swapchain frames
- executing platform commands and delivering results back as events

### 2) Commands + Events boundary (platform separation)
Core/UI does not perform syscalls. It emits commands:
- `OpenUrl(url)`
- `SetClipboardText(text)`
- `PickFile(options)`
- `HttpRequest(request)` (optional; consider keeping network in Rust if you don’t need host networking)
- `Log(line)`
- etc.

Host returns events:
- `ClipboardText(text)`
- `FilePicked(path/bytes/handle)`
- `WindowResized(...)`
- `KeyDown`, `PointerMoved`, `TextInput`, `Touch...`
- etc.

### 3) FlatBuffers for ABI
We use FlatBuffers because it has strong multi-language support (Swift/Kotlin/C#/C++).
- All FFI payloads are FlatBuffers messages (bytes).
- C ABI functions accept/return raw pointers + lengths.
- Version each message root. Add a compatibility plan:
  - new fields are optional, old hosts ignore unknown fields
  - keep a `protocol_version` handshake

### 4) Remove eframe
Desktop runner uses:
- `winit` (events + window)
- `wgpu` (rendering)
- `egui` (UI)
- `egui_winit` (translating winit events → egui input)
- `egui_wgpu` (rendering egui paint jobs via wgpu)

Web runner uses a wasm-compatible stack:
- `wasm-bindgen` + `web-sys` for platform glue (only in `apps/web`)
- `wgpu` WebGPU backend
- `egui` + a web input adapter (either custom or reuse existing crates if appropriate)

---

## Work breakdown (multi-TODO plan)

### P0 — Prepare the workspace for a clean split (no behavior changes)
- [ ] Create new crates:
  - [ ] `crates/platform-api`
  - [ ] `crates/engine`
  - [ ] `crates/ffi`
  - [ ] `apps/desktop`
  - [ ] `apps/web` (or migrate from existing wasm entry)
- [ ] Update root `Cargo.toml` workspace members accordingly.
- [ ] Move platform bootstrap out of the current UI crate:
  - [ ] Remove `collects/ui/src/main.rs` (move code into `apps/desktop` and `apps/web`).
- [ ] Ensure `collects-ui` (or `crates/ui`) is **lib-only**.

Acceptance:
- Workspace builds for desktop and wasm with separate binaries.
- UI crate has no `winit`, `wgpu`, `web-sys`, paste/clipboard impls, or platform-specific deps.

---

### P1 — Desktop runner: winit + wgpu + egui (replace eframe)
- [ ] Implement `apps/desktop`:
  - [ ] Create window + event loop (winit)
  - [ ] Initialize wgpu device/queue/surface
  - [ ] Integrate egui:
    - [ ] input translation via `egui_winit` (or custom)
    - [ ] rendering via `egui_wgpu`
  - [ ] Host-driven frame loop:
    - [ ] collect events
    - [ ] build `egui::RawInput`
    - [ ] call `engine.frame(...)`
    - [ ] execute commands
    - [ ] render paint jobs
    - [ ] present
- [ ] Add window icon, size constraints, drag-and-drop if needed (platform shell concern).

Acceptance:
- Desktop app runs without eframe.
- Rendering uses wgpu and egui via desktop runner.
- Feature parity baseline with current desktop behavior.

---

### P2 — Engine crate: stable per-frame pipeline (platform-agnostic)
- [ ] Define in `crates/platform-api`:
  - [ ] `HostEvent` (platform → rust)
  - [ ] `Command` (rust → platform)
  - [ ] capability description (what the host supports)
- [ ] Define in `crates/engine`:
  - [ ] `Engine` object holding:
    - [ ] egui context + memory (or allow host to manage persistence)
    - [ ] app state root (current `State` etc.)
    - [ ] event queue
    - [ ] outgoing command queue
  - [ ] `Engine::push_event(...)`
  - [ ] `Engine::frame(frame_args) -> FrameOutput`
    - [ ] apply events
    - [ ] run UI
    - [ ] collect commands
    - [ ] produce egui paint output (shapes/meshes + texture deltas)
- [ ] Ensure `crates/engine` has **no** platform deps and minimal `cfg(...)`.

Acceptance:
- Desktop runner is a thin adapter: winit events → `HostEvent`; `FrameOutput` → egui_wgpu render; execute `Command`.

---

### P3 — Web runner (keep wasm support)
- [ ] Implement `apps/web` using the new engine:
  - [ ] create canvas + event listeners
  - [ ] map web input → `HostEvent`
  - [ ] host-driven frames via `requestAnimationFrame`
  - [ ] wgpu WebGPU surface setup
  - [ ] render egui paint output
- [ ] Keep wasm font loading + assets (move out of UI crate, into `apps/web`).

Acceptance:
- Web app builds and runs, using the same engine+UI crates.

---

### P4 — FlatBuffers ABI + C FFI crate (for future native wrappers)
- [ ] Add FlatBuffers schema package:
  - [ ] `protocol/collects.fbs` (or `crates/ffi/schema/*.fbs`)
  - [ ] define:
    - [ ] `Hello` / handshake message
    - [ ] `HostEventBatch`
    - [ ] `CommandBatch`
    - [ ] `FrameArgs`
    - [ ] `FrameOutput`
- [ ] Generate bindings for Rust (build.rs) and check in (or generate in CI).
- [ ] Implement `crates/ffi` C ABI:
  - [ ] opaque `EngineHandle`
  - [ ] `collects_engine_new(...)`
  - [ ] `collects_engine_free(handle)`
  - [ ] `collects_engine_push_events(handle, bytes, len)`
  - [ ] `collects_engine_frame(handle, frame_args_bytes, len) -> bytes`
  - [ ] memory ownership rules (who allocates/frees)
 see below
- [ ] Add protocol versioning rules and compatibility testing.

Acceptance:
- A minimal “ffi test harness” (could be Rust-only) can call the C ABI, push events, run a frame, and receive deterministic output.

#### Memory management rules (must be explicit)
- [ ] Define one allocator strategy:
  - Rust allocates return buffers; host calls `collects_free(ptr, len)` to free
  - Host provides an allocator callback (more complex)
- [ ] Document lifetime, thread-safety, and reentrancy constraints.

---

### P5 — Native wrappers (later; not P0)
- [ ] macOS wrapper (Swift):
  - [ ] create window/view
  - [ ] create wgpu surface (Metal layer)
  - [ ] drive frames and forward input
  - [ ] execute commands
- [ ] iOS wrapper (Swift):
  - [ ] UIView / CAMetalLayer
  - [ ] touch input mapping
- [ ] Android wrapper (Kotlin):
  - [ ] SurfaceView / Choreographer
  - [ ] touch/IME input mapping
- [ ] Windows wrapper (C#/WinUI) optional:
  - [ ] swapchain + input mapping + commands

Acceptance:
- Wrapper can boot the Rust engine and render egui in a native surface with host-driven frames.

---

## Cross-cutting todos / tracking

### Dependency hygiene
- [ ] Move platform dependencies out of UI/core crates:
  - [ ] `web-sys`, `wasm-bindgen*` only in `apps/web`
  - [ ] `winit`, `wgpu`, `egui_winit`, `egui_wgpu` only in platform runners (or in a dedicated “renderer” crate used by runners)
- [ ] Lock wgpu version (avoid `version="*"`).
- [ ] Ensure `collects-ui` only depends on platform-agnostic crates.

### Async strategy (keep host-driven)
- [ ] Keep async work inside Rust (recommended) using a runtime that works with:
  - desktop (multithread or current-thread)
  - wasm (spawn_local)
- [ ] Ensure async completions feed into engine via event queue and trigger redraw via host command (e.g., `RequestRedraw`).
- [ ] Avoid blocking in `frame()`.

### Testing
- [ ] Unit test: engine frame determinism given a known input batch.
- [ ] Snapshot test: UI output for known states (kittest/harness).
- [ ] FFI contract tests:
  - [ ] schema compatibility tests
  - [ ] buffer ownership tests
  - [ ] fuzz-ish tests for malformed FlatBuffers input (must not panic/UB)

### Documentation
- [ ] Add an architecture doc in `docs/` with:
  - crate responsibilities
  - allowed dependency directions
  - event/command model
  - FFI memory rules
  - threading model (host-driven frames + engine is single-threaded unless explicitly designed otherwise)

---

## Tracking table (status)

### Milestones
- [ ] M0: Workspace split complete, eframe removed from build graph
- [ ] M1: Desktop runner working (winit+wgpu+egui)
- [ ] M2: Engine crate stable API (events/commands/frame output)
- [ ] M3: Web runner migrated to engine
- [ ] M4: FlatBuffers ABI + FFI crate working (Rust harness)
- [ ] M5: Native wrapper PoC (macOS or iOS)

---

## Notes / open questions (to resolve early)
- [ ] Where should persistence live? (egui memory/state serialization)
- [ ] Who owns DPI scaling and font configuration? (host vs engine)
- [ ] How to handle IME/text input consistently across platforms?
- [ ] How to handle file dialog results: host-managed path vs bytes vs handle?
- [ ] How to handle GPU resource lifetime across FFI (textures/buffers):
  - likely keep GPU ownership within the host process, but within Rust for desktop/web runners
- [ ] Protocol versioning strategy (semantic vs integer handshakes).

---