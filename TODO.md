# TODO: Migration plan — async-safe command execution (eframe/egui) + snapshot deps

This TODO tracks a staged refactor to make `collects/ui` + `collects/states` compatible with:
- running async work concurrently (including WASM)
- eliminating command access to live mutable references
- executing commands deterministically at **end-of-frame**
- using **snapshot/clone** inputs for Commands (and later for Computes if desired)
- preserving UI-only mutable state for egui-affine types (e.g. `egui::TextureHandle`)

> Migration starts in `collects/states` (trait changes first), then adds one command queue flush in UI.

---

## Background / Current issues

### Problem: `Dep` + immediate dispatch permits rule-breaking mutation patterns
`collects/states` currently constructs a `Dep` object backed by raw pointers (`NonNull<dyn Any>`) and supports returning `&'static mut T` derived from `&self` through `Dep::state_mut()`. This is unsafe and becomes untenable once commands spawn concurrent async work.

Additionally, `StateCtx::dispatch::<T>()` executes commands immediately in the caller thread, mid-frame.

### Goal state (invariants)
1. **Only UI thread mutates live state/compute storage.**
2. **Commands read only snapshots (owned clones) of state/compute.**
3. Commands produce updates via **queued events** (existing `Updater` channel is acceptable).
4. Commands are enqueued and executed **at end-of-frame**.
5. Async jobs are allowed to run concurrently; completions may arrive out-of-order → apply only if the request is still current.
6. WASM support: no assumption of threads; runtime separation is logical.

---

## Core design decisions

### 1) Concurrency + request validation
- `TypeId` alone is not a sufficient request id (multiple in-flight requests of the same compute type share the same `TypeId`).
- Use `(TypeId, generation)` as request identity:
  - `generation: u64` monotonically increases per compute type.
  - Store current `generation` in the compute’s state (`Loading { generation, ... }`).
  - Async completion sends `(TypeId, generation, result)`; UI applies only if `generation` matches.

### 2) Command storage
- Use `Box<dyn Command>` in the command queue for memory friendliness.

### 3) Snapshot cloning
- Commands must not borrow state by reference.
- Business state should be made clone-friendly via `Arc<...>`, `Ustr`, small `Copy` fields, etc.
- UI-affine state (e.g. `TextureHandle`) remains UI-owned and mutated only on UI thread via `state_ctx.update()` / `state_ctx.state_mut()`.

---

## Phase 1 — `collects/states`: change command trait + remove live `Dep` from command execution

### 1.1 Introduce snapshot types
Add new snapshot containers in `collects/states`:
- `StateSnapshot`: typed reads returning owned clones
- `ComputeSnapshot`: typed reads returning owned clones
- `CommandSnapshot`: wraps both, plus optional util helpers

Notes:
- Snapshot creation happens on UI thread during flush (end-of-frame).
- Snapshot should be **narrow** eventually (only required types), but can start as a “clone all registered states/computes” baseline for simplicity.

### 1.2 Update `Command` trait
Replace:
- `fn run(&self, deps: Dep, updater: Updater)`

With:
- `fn run(&self, snap: CommandSnapshot, updater: Updater)`

Rules:
- `CommandSnapshot` provides read-only, owned clones.
- No mutable borrows to live state.

### 1.3 Keep `Updater` (for now)
Continue using `Updater::{ set, set_state }` as the message queue mechanism.
- Strongly prefer updating Computes via `Updater::set(...)`.
- Allow `Updater::set_state(...)` only for `Send`-safe business states.
- UI-affine states must not use `set_state` (keep UI-only mutation).

### 1.4 Deprecate / restrict `Dep::state_mut`
- Remove `Dep::state_mut()` OR restrict it to internal-only usage.
- Goal: prevent any command from mutating live state through `Dep`.
- Add doc comment + (optional) compile-time gating so downstream crates cannot access it.

### 1.5 Ensure Compute trait stays side-effect free
Computes remain:
- either derived pure computations
- or no-op caches updated explicitly via `Updater::set()`

Do not run network/async in `Compute::compute()`.

---

## Phase 2 — `collects/ui`: add command queue + one flush per frame

### 2.1 Add `CommandQueue`
Create UI-owned queue:
- `VecDeque<Box<dyn Command>>`

### 2.2 Change dispatch call sites
Replace `ctx.dispatch::<SomeCommand>()` call sites with:
- `dispatcher.enqueue(Box::new(SomeCommand { ... }))`
or
- `state_ctx.enqueue_command::<SomeCommand>()` (if you provide a facade)

### 2.3 Execute commands at end-of-frame (one flush)
In `eframe::App::update` after UI rendering:
1. `state_ctx.sync_computes()` (drain updater messages from prior async completions)
2. Flush command queue:
   - Build `CommandSnapshot` from current state/compute values (clone)
   - Run each command sequentially with `(snapshot, updater)`
3. `state_ctx.sync_computes()` (apply updates emitted by the command flush)
4. Run compute graph if needed (existing dirty propagation should continue to work)

**Deliverable for this phase:**
- Implement the flush pipeline and migrate exactly one small command path to prove it works end-to-end.

Suggested first command to migrate:
- `ToggleApiStatusCommand` (minimal state/compute interaction)

---

## Phase 3 — Async runtime abstraction (WASM-compatible)

> Keep runtime design compatible with WASM: threads are not guaranteed.

### 3.1 Add `Spawner` abstraction
Introduce a minimal interface exposed to commands:
- `spawner.spawn(async move { ... })`

On native:
- can be backed by tokio (current-thread or multi-thread)
On wasm:
- tokio current-thread + wasm-friendly spawn mechanism

### 3.2 Ensure all async tasks send results through `Updater`
Async task completion must:
- send Compute updates via `Updater::set(...)` (preferred),
- or send State updates via `Updater::set_state(...)` only for `Send` state.

---

## Phase 4 — Request generation + out-of-order completion safety

For each compute that can have concurrent requests:
- Add `generation: u64` and store it in compute (`Loading { generation }`).
- When command starts a fetch:
  - increment generation (in compute update) and emit `Loading { generation }`
  - spawn async capturing generation
- On completion:
  - emit `Loaded { generation, ... }` (or include generation in the compute payload)
- In the UI thread apply step:
  - ignore completion if generation != current expected generation

Notes:
- If we keep “updates are whole compute values”, the “ignore old updates” check needs to happen either:
  - inside the compute payload application logic, or
  - before calling `Updater::set` (but async tasks won’t know current generation), or
  - by encoding generation into compute and letting UI treat newer generation as authoritative.

Preferred: encode generation into the compute; UI simply displays latest generation state.

---

## Phase 5 — Rule checks / enforcement

### 5.1 Ban mutable-from-ref patterns for commands
Add CI checks (or deny-lint conventions) to prevent:
- `Dep::state_mut`
- any command implementation calling `state_ctx.state_mut()` (commands should not have a `StateCtx` anyway in the new signature)

### 5.2 Keep UI-only mutation explicit
Allow `state_ctx.state_mut::<T>()` / `state_ctx.update::<T>()`:
- only in `collects/ui/**`
- only for UI-affine state like `TextureHandle` and text edit bindings

### 5.3 Document “Send-safe state” boundary
Any state intended to be updated from async completion must:
- implement `State::assign_box(...)`
- be `Send`
- not contain egui-affine values

---

## Initial checklist (do this next)

- [ ] Add snapshot types to `collects/states`
- [ ] Update `Command` trait signature to accept snapshots
- [ ] Update `StateCtx::dispatch` or replace with queue-friendly API (do not execute immediately)
- [ ] Remove or restrict `Dep::state_mut`
- [ ] Implement UI command queue and flush once per frame
- [ ] Migrate `ToggleApiStatusCommand` to confirm pipeline
- [ ] Add generation-based request tracking on one async compute as example
- [ ] Add automated rule checks to prevent regressions

---

## Notes / Known UI-affine exceptions
Some state intentionally includes `egui::TextureHandle` (non-Send), e.g. internal users QR texture and image preview. These must remain UI-only and must not be updated via async Updater state messages.

Commands should never depend on these UI-affine states; instead, commands should return data (bytes/URLs/etc.) and UI should build textures from that in-frame.

---