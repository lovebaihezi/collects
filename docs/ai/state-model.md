# State model rules (StateCtx / Compute / Command)

This doc defines the **minimal rules** for state, computes, and commands in `collects` to support:
- `eframe/egui` (single-threaded UI frame loop)
- concurrent async work (including `wasm32`)
- deterministic, debuggable updates

If you are changing any state/compute/command logic, read this doc.

---

## Terms

- **State**: durable application/workflow state stored in `StateCtx` (selection, visibility flags, inputs that must persist).
- **Compute**: derived/cached values, typically async lifecycle results `Idle | Loading | Loaded | Error`.
- **Command**: manual-only action that may perform IO/async and updates Computes/States via queued updates.
- **UI-affine state**: state that contains non-`Send` UI types (e.g. `egui::TextureHandle`) and must only be mutated on the UI thread.

---

## Core invariants (non-negotiable)

1. **Single writer**: Only the UI thread mutates live `StateCtx` storage (states + computes).
2. **End-of-frame execution**: UI events enqueue commands; commands execute at **end-of-frame** in a flush step.
3. **Snapshot reads**: Commands must read state/compute values from **snapshots (owned clones)** created during flush.  
   Commands must not borrow live `&` references to state/compute values.
4. **Queued writes**: Commands must write results through the queued update mechanism (e.g. `Updater::set(...)`, `Updater::set_state(...)`).
5. **Compute purity**: `Compute::compute()` must not perform network IO or spawn async. Computes are either:
   - pure derived computations, or
   - caches updated explicitly by commands via `Updater::set(...)`.

---

## Where things live (ownership)

- All domain/app `State`, `Compute`, and `Command` types live in `collects/business` (aka `collects_business`).
- UI code under `collects/ui` is UI-only:
  - may read business state/compute
  - may mutate UI-affine state on the UI thread
  - must not define new domain state/compute/command types

---

## UI → State: synchronous mutation rules

Preferred mutation API:
- Use `StateCtx::update::<T>(|s| ...)` for synchronous mutations that should mark dependent computes dirty.

Allowed use of `StateCtx::state_mut::<T>()`:
- binding widget inputs (`text_edit_singleline(&mut ...)`)
- UI-affine state updates (textures, egui handles)
- rare cases where you intentionally do not want dirty propagation

Avoid:
- holding state mutable references across frames
- using UI state as an “async message bus”

---

## Commands: inputs, execution, and outputs

### Execution timing
- Commands are not executed inline from widget callbacks.
- UI enqueues commands; the app flushes the command queue **once per frame** (end-of-frame).

### Inputs: snapshots only
Commands receive a `CommandSnapshot` (or equivalent) that provides **owned clones** of needed values:
- `snap.get_state::<T>() -> T` (clone/owned)
- `snap.get_compute::<T>() -> T` (clone/owned)

Rules:
- Commands must not mutate live state/compute directly.
- Commands must not depend on egui types or `egui::Context`.

### Outputs: queued updates only
Commands update the world through queued updates:
- Compute updates: `Updater::set(ComputeType { ... })` (preferred)
- State updates: `Updater::set_state(StateType { ... })` **only if State is `Send`-safe**

UI-affine state **must not** be updated via `Updater::set_state()`.

---

## Concurrent async + out-of-order safety

### Allowed: multiple in-flight jobs
Commands may spawn concurrent async operations. Inputs are snapshots, so jobs must capture owned data.

### Required: request identity (stale result protection)
`TypeId` alone is not sufficient because multiple requests of the same compute type can be in flight.

Use:
- `(TypeId, generation)` where `generation: u64` increments per compute type.

Pattern:
1. Command starts request:
   - increments generation for the compute type
   - sets compute -> `Loading { generation, ... }`
2. Async completion emits:
   - compute -> `Loaded { generation, ... }` or `Error { generation, ... }`
3. UI/app only treats the latest generation as authoritative.

Implementation note:
- Prefer encoding `generation` inside the compute value itself so stale completions naturally become “older” state.

---

## UI-affine state boundary (textures, egui handles)

Examples of UI-affine types:
- `egui::TextureHandle`
- `eframe`/`egui` types that are not `Send`

Rules:
- Keep UI-affine state in UI code when possible.
- If a business feature needs to display a texture, prefer:
  - command returns raw bytes / decoded pixels / URL in a compute
  - UI converts it into an egui texture on the UI thread and stores it in UI-affine state

---

## Common anti-patterns (do not do these)

- Running network IO inside `Compute::compute()`.
- Commands borrowing live references to state/compute (no `&T` / `&mut T` from `StateCtx`).
- Using “mutable-from-ref” APIs in command paths (manufacturing `&mut` from `&self`).
- Putting `egui::Context` temp memory into a domain workflow.
- Updating non-`Send` state via async completion channels.

---

## Checklist (when you add/refactor a feature)

- [ ] State/Compute/Command types are in `collects/business`
- [ ] UI mutates state via `update()` (unless UI binding/affine exception)
- [ ] Commands execute end-of-frame (queued)
- [ ] Commands read only snapshot clones
- [ ] Async completion updates compute via `Updater::set(...)`
- [ ] For concurrent jobs, compute includes `generation: u64` and stale results are safe
- [ ] UI-affine state stays on UI thread