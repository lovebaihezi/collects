# Send-safe state boundary

This doc defines which states can be safely updated via async completion channels (`Updater::set_state()`) and which cannot.

If you are adding a new `State` or modifying existing states, read this doc.

---

## Terms

- **Send-safe state**: A `State` whose fields are all `Send`. It can be safely updated from async completions via `Updater::set_state()`.
- **UI-affine state**: A `State` that contains non-`Send` types (e.g., `egui::TextureHandle`). It must only be mutated on the UI thread via `StateCtx::state_mut()` or `StateCtx::update()`.

---

## Core rules

### 1) Send-safe states can use `Updater::set_state()`

A state is Send-safe if all of the following are true:
- All fields implement `Send`
- The state implements `SnapshotClone` with `clone_boxed()` returning `Some(...)`
- The state implements `State::assign_box()` using `state_assign_impl()`

These states can be updated from async command completions:
```rust
// In a Command's run() method:
updater.set_state(MyState { ... });
```

### 2) UI-affine states must NOT use `Updater::set_state()`

A state is UI-affine if it contains any non-`Send` types. Common examples:
- `egui::TextureHandle` (renders GPU textures)
- `egui::Context` internal state
- Other platform-specific handles

UI-affine states:
- Should implement `SnapshotClone` with the default (returns `None`)
- Should NOT implement `State::assign_box()` (keep the default panic)
- Must be mutated only on the UI thread via `StateCtx::state_mut()` or `StateCtx::update()`

### 3) Commands should return data, not UI resources

If a command needs to provide data that will become a UI resource:
1. Command returns raw data (bytes, URLs, decoded pixels) via `Updater::set()`
2. UI code converts the data to UI resources on the UI thread
3. UI stores the resource in UI-affine state via `StateCtx::state_mut()`

---

## Implementation patterns

### Send-safe state (can be updated via Updater)

```rust
/// Configuration state - all fields are Send.
#[derive(Debug, Clone)]
pub struct BusinessConfig {
    pub api_base_url: String,
    pub cf_authorization: Option<String>,
}

impl SnapshotClone for BusinessConfig {
    fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
        Some(Box::new(self.clone()))  // ✓ Returns Some
    }
}

impl State for BusinessConfig {
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
    
    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        state_assign_impl(self, new_self);  // ✓ Implements assign_box
    }
}
```

### UI-affine state (must NOT be updated via Updater)

```rust
/// UI state containing non-Send TextureHandle.
#[derive(Default)]
pub struct InternalUsersState {
    pub users: Vec<InternalUserItem>,
    pub qr_texture: Option<TextureHandle>,  // ✗ Non-Send type!
    // ... other fields
}

// UI-affine: default SnapshotClone returns None
impl SnapshotClone for InternalUsersState {}

impl State for InternalUsersState {
    fn as_any(&self) -> &dyn Any { self }
    fn as_any_mut(&mut self) -> &mut dyn Any { self }
    
    // ✗ No assign_box implementation - default panics
    // This prevents accidental use of Updater::set_state()
}
```

### Command returning data for UI to convert

```rust
impl Command for GetUserQrCommand {
    fn run(&self, snap: CommandSnapshot, updater: Updater, _cancel: CancellationToken)
        -> Pin<Box<dyn Future<Output = ()> + Send>>
    {
        Box::pin(async move {
            // Command fetches raw QR data
            let qr_bytes = fetch_qr_code().await;
            
            // ✓ Update Send-safe compute with raw data
            updater.set(QrCodeCompute {
                status: QrCodeStatus::Loaded { otpauth_url },
            });
            
            // ✗ Do NOT try to create TextureHandle here
            // ✗ Do NOT try to update InternalUsersState here
        })
    }
}

// In UI code:
fn render_qr_modal(ctx: &mut StateCtx, egui_ctx: &egui::Context) {
    let qr_compute = ctx.cached::<QrCodeCompute>();
    if let QrCodeStatus::Loaded { otpauth_url } = &qr_compute.status {
        // ✓ UI creates texture from raw data on UI thread
        let texture = create_qr_texture(egui_ctx, otpauth_url);
        
        // ✓ UI stores texture in UI-affine state via state_mut
        ctx.state_mut::<InternalUsersState>().qr_texture = Some(texture);
    }
}
```

---

## Checklist (when adding a new State)

- [ ] Does the state contain any non-`Send` types?
  - **No** → Implement as Send-safe (implement `SnapshotClone::clone_boxed()` returning `Some`, implement `assign_box`)
  - **Yes** → Implement as UI-affine (default `SnapshotClone`, no `assign_box`)
- [ ] If UI-affine, is the state only mutated on the UI thread?
- [ ] If commands need to provide data for this state, do they return raw data via a Send-safe compute?

---

## Common mistakes

| Mistake | Why it's wrong | Fix |
|---------|----------------|-----|
| `Updater::set_state()` on UI-affine state | Will panic or cause UB | Use `StateCtx::state_mut()` on UI thread |
| Creating `TextureHandle` in async command | Non-Send type created off UI thread | Return raw bytes, create texture on UI thread |
| Implementing `assign_box` for UI-affine state | Allows unsafe async updates | Remove implementation, keep default panic |
| `SnapshotClone::clone_boxed()` returning `Some` for UI-affine state | Implies state is Send-safe | Return `None` (default impl) |

---

## Testing guidance

When testing Send-safe states:
- Unit tests can verify `SnapshotClone::clone_boxed()` returns `Some`
- Integration tests can verify commands update state via `Updater::set_state()`

When testing UI-affine states:
- Verify `SnapshotClone::clone_boxed()` returns `None`
- Verify state is only mutated via direct `StateCtx` methods in UI code
