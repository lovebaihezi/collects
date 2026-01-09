# TODO: Migration plan — Full async architecture (eframe/egui) + Structured Concurrency

This TODO tracks an **aggressive refactor** to make `collects/ui` + `collects/states` fully async with:
- **Structured Concurrency** with parent-child task relationships
- **CancellationToken** pattern for cooperative task cancellation
- Full async Compute and Command traits
- **reqwest** replacing ehttp for HTTP requests
- **flume** in async mode for channels
- WASM compatibility via `tokio` current-thread runtime
- No backward compatibility — clean break from sync patterns

> Migration is aggressive: all computes/commands become async, sync APIs removed.

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
5. **Structured Concurrency** — StateCtx owns task lifecycle, auto-cancels superseded tasks.
6. WASM support: `tokio` current-thread runtime, no thread assumptions.

---

## Core design decisions

### 1) Structured Concurrency + CancellationToken (Recommended Pattern)

**Architecture:**
- `StateCtx` acts as **parent scope**, owns a `JoinSet` of active tasks
- Each spawned task gets a `CancellationToken` from `tokio_util::sync`
- Spawning a new task for the same compute type **auto-cancels** the previous token
- Clean shutdown: `StateCtx::shutdown()` cancels all tokens and awaits completion

**How it separates concerns:**
- **Framework (states crate)**: Manages `TaskHandle` lifecycle, abort signals, cleanup
- **Business code**: Just calls `ctx.spawn_task::<ApiStatus>(async { ... })` — no abort logic needed
- **Async callback**: Framework auto-checks `token.is_cancelled()` before applying results

**Benefits:**
- Business compute stays pure — no generation/abort tracking in business code
- StateCtx can kill any task type (HTTP, IO, long computations)
- Works for network, IO, CPU-bound work equally
- WASM-friendly (no threads required)
- Matches how Tokio, sqlx, sea-orm, and gRPC work

### 2) Full Async Traits

**Compute trait (async):**

> Note: Rust now supports async fn in traits natively (stabilized in Rust 1.75).
> We use `impl Future<Output = ...>` syntax instead of the `async_trait` macro.

```rust
pub trait Compute: Debug + Any + SnapshotClone + Send + Sync {
    fn compute(&self, deps: Dep, updater: Updater, cancel: CancellationToken) -> impl Future<Output = ()> + Send;
    fn deps(&self) -> ComputeDeps;
}
```

**Command trait (async):**
```rust
pub trait Command: Debug + Any + Send + Sync {
    fn run(&self, snap: CommandSnapshot, updater: Updater, cancel: CancellationToken) -> impl Future<Output = ()> + Send;
}
```

### 3) TaskHandle abstraction

```rust
pub struct TaskHandle {
    id: TaskId,
    cancel_token: CancellationToken,
    join_handle: JoinHandle<()>,
}

impl StateCtx {
    /// Spawns task, auto-cancels previous task for same compute type
    pub fn spawn_task<T: Compute>(&mut self, f: impl Future + Send + 'static) -> TaskHandle;
    
    /// Cancel all tasks and await completion
    pub async fn shutdown(&mut self);
}
```

### 4) Snapshot cloning (unchanged)
- Commands must not borrow state by reference.
- Business state should be made clone-friendly via `Arc<...>`, `Ustr`, small `Copy` fields, etc.
- UI-affine state (e.g. `TextureHandle`) remains UI-owned and mutated only on UI thread.

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

## Phase 3 — Full Async Migration (AGGRESSIVE - NO BACKWARD COMPAT)

> This phase converts everything to async. No sync APIs preserved.

### 3.1 Add `tokio` + `tokio_util` dependencies

In `collects/states/Cargo.toml`:
```toml
tokio = { version = "1", features = ["rt", "sync", "macros"] }
tokio_util = { version = "0.7", features = ["rt"] }
```

> Note: The `async-trait` crate is no longer needed since Rust 1.75+ supports async fn in traits natively.
> We use `impl Future<Output = ...>` return type syntax instead.

### 3.2 Implement TaskHandle + CancellationToken

Add to `collects/states`:
- `TaskHandle` struct with `CancellationToken` from `tokio_util::sync`
- `TaskId` type (newtype over `u64` or `TypeId + generation`)
- `JoinSet` for managing spawned tasks

### 3.3 Update StateCtx for task management

```rust
impl StateCtx {
    /// Spawns async task, auto-cancels previous task for same compute type
    pub fn spawn_task<T: Compute>(&mut self, future: impl Future + Send + 'static) -> TaskHandle;
    
    /// Cancel specific task
    pub fn cancel_task(&mut self, handle: &TaskHandle);
    
    /// Cancel all tasks and await completion (for shutdown)
    pub async fn shutdown(&mut self);
}
```

### 3.4 Convert `Compute` trait to async

> Note: Using `impl Future<Output = ...>` return type (Rust 1.75+) instead of `#[async_trait]` macro.

```rust
pub trait Compute: Debug + Any + SnapshotClone + Send + Sync {
    fn compute(&self, deps: Dep, updater: Updater, cancel: CancellationToken) -> impl Future<Output = ()> + Send;
    fn deps(&self) -> ComputeDeps;
    fn as_any(&self) -> &dyn Any;
    fn assign_box(&mut self, new_self: Box<dyn Any + Send>);
}
```

### 3.5 Convert `Command` trait to async

> Note: Using `impl Future<Output = ...>` return type (Rust 1.75+) instead of `#[async_trait]` macro.

```rust
pub trait Command: Debug + Any + Send + Sync {
    fn run(&self, snap: CommandSnapshot, updater: Updater, cancel: CancellationToken) -> impl Future<Output = ()> + Send;
}
```

---

## Phase 4 — Replace ehttp with reqwest

> reqwest is async-native and works with tokio. ehttp uses callbacks.

### 4.1 Update dependencies

In `collects/business/Cargo.toml`:
```toml
# Remove
ehttp = { ... }

# Add
reqwest = { version = "0.12", default-features = false, features = ["json", "rustls-tls"] }
```

### 4.2 Migrate all HTTP calls

Convert all `ehttp::fetch(request, |result| { ... })` to:
```rust
async fn fetch_api_status(cancel: CancellationToken) -> Result<ApiResponse, Error> {
    tokio::select! {
        _ = cancel.cancelled() => Err(Error::Cancelled),
        result = reqwest::get(url).await => {
            // handle result
        }
    }
}
```

### 4.3 Update affected computes/commands
- `ApiStatus` compute
- `InternalApiStatus` compute  
- `LoginCommand`
- `ValidateTokenCommand`
- `CreateUserCommand`
- `RefreshInternalUsersCommand`

---

## Phase 5 — flume async mode

> Switch flume channels to async send/recv for better integration.

### 5.1 Update Updater to use async flume

```rust
// Current (sync)
pub fn set<T: Compute + 'static>(&self, value: T) {
    self.sender.send(UpdateMessage::Compute(...)).unwrap();
}

// New (async)
pub async fn set<T: Compute + 'static>(&self, value: T) {
    self.sender.send_async(UpdateMessage::Compute(...)).await.unwrap();
}
```

### 5.2 Update sync_computes to async

```rust
impl StateCtx {
    pub async fn sync_computes(&mut self) {
        while let Ok(msg) = self.receiver.try_recv() {
            // ... apply updates
        }
    }
}
```

---

## Phase 6 — Rule checks / enforcement

### 6.1 Ban mutable-from-ref patterns for commands
Add CI checks (or deny-lint conventions) to prevent:
- `Dep::state_mut`
- any command implementation calling `state_ctx.state_mut()` (commands should not have a `StateCtx` anyway in the new signature)

### 6.2 Keep UI-only mutation explicit
Allow `state_ctx.state_mut::<T>()` / `state_ctx.update::<T>()`:
- only in `collects/ui/**`
- only for UI-affine state like `TextureHandle` and text edit bindings

### 6.3 Document “Send-safe state” boundary
Any state intended to be updated from async completion must:
- implement `State::assign_box(...)`
- be `Send`
- not contain egui-affine values

---

## Migration checklist — Full Async Architecture

### Phase 1-2 (COMPLETED)
- [x] Add snapshot types to `collects/states`
- [x] Update `Command` trait signature to accept snapshots
- [x] Update `StateCtx::dispatch` or replace with queue-friendly API
- [x] Remove or restrict `Dep::state_mut`
- [x] Implement UI command queue and flush once per frame
- [x] Migrate `ToggleApiStatusCommand` to confirm pipeline

### Phase 3 — Full Async Migration
- [x] Add `tokio`, `tokio_util` dependencies to `collects/states`
- [x] Implement `TaskHandle` with `CancellationToken`
- [x] Implement `TaskId` type
- [x] Add `JoinSet` to `StateCtx` for task management
- [x] Add `StateCtx::spawn_task<T>()` method
- [x] Add `StateCtx::cancel_task()` method
- [x] Add `StateCtx::shutdown()` async method
- [ ] Convert `Compute` trait to async
- [ ] Convert `Command` trait to async
- [ ] Update all existing computes to async
- [ ] Update all existing commands to async

### Phase 4 — Replace ehttp with reqwest
- [ ] Remove `ehttp` dependency from `collects/business`
- [ ] Add `reqwest` dependency with async features
- [ ] Migrate `ApiStatus` compute HTTP calls
- [ ] Migrate `InternalApiStatus` compute HTTP calls
- [ ] Migrate `LoginCommand` HTTP calls
- [ ] Migrate `ValidateTokenCommand` HTTP calls
- [ ] Migrate `CreateUserCommand` HTTP calls
- [ ] Migrate `RefreshInternalUsersCommand` HTTP calls

### Phase 5 — flume async mode
- [ ] Update `Updater::set()` to async
- [ ] Update `Updater::set_state()` to async
- [ ] Update `StateCtx::sync_computes()` to async
- [ ] Update UI frame loop to use async sync

### Phase 6 — Rule checks / enforcement
- [ ] Add CI checks to ban `Dep::state_mut`
- [ ] Add CI checks to enforce async trait usage
- [ ] Document Send-safe state boundary

---

## Notes / Known UI-affine exceptions
Some state intentionally includes `egui::TextureHandle` (non-Send), e.g. internal users QR texture and image preview. These must remain UI-only and must not be updated via async Updater state messages.

Commands should never depend on these UI-affine states; instead, commands should return data (bytes/URLs/etc.) and UI should build textures from that in-frame.

---