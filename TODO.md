# TODO: Refactor legacy async egui-memory patterns (ctx.memory_mut)

This file tracks remaining places where UI code uses `egui::Context` memory (`ctx.memory_mut` / `mem.data.insert_temp/get_temp/remove`) as an **async message bus**.

## Goal

Migrate each legacy flow to the repo's reference pattern:

- Async work runs inside a **business `Command`**
- Async lifecycle/results stored in a **business `Compute`**
- UI:
  - **reads** via `ctx.cached::<Compute>()`
  - **writes** via `ctx.dispatch::<Command>()`
- Chatty inputs use **UI-local hybrid drafts**:
  - seed once from business data when modal opens/selection changes
  - commit on submit via a command
- Remove the polling bridge (`poll_*_responses`) once all async flows are migrated.

---

## Anti-pattern (what to remove)

- Callback writes results into egui memory:
  - `ctx.memory_mut(|mem| mem.data.insert_temp(egui::Id::new("..."), value))`
- UI update loop polls egui memory:
  - `ctx.memory(|mem| mem.data.get_temp::<T>(egui::Id::new("...")))`
- Then removes it:
  - `ctx.memory_mut(|mem| mem.data.remove::<T>(egui::Id::new("...")))`

This pattern is:
- stringly-typed (`"action_success"`, `"internal_users_response"`, …)
- frame-order coupled
- harder to test
- not aligned with "Command updates Compute"

---

## Status: Completed slices

- ✅ Internal Users: Refresh/List users now uses `RefreshInternalUsersCommand` → `InternalUsersListUsersCompute`
  - No longer uses `ctx.memory_mut` temps for `"internal_users_response"` / `"internal_users_error"`
- ✅ Internal Users: Actions migrated off egui-memory temps
  - Business: added `InternalUsersActionCompute` + typed `InternalUsersActionState/Kind`
  - Commands: `UpdateUsernameCommand`, `UpdateProfileCommand`, `DeleteUserCommand`, `RevokeOtpCommand`, `GetUserQrCommand`
  - UI: modals + inline QR now dispatch commands and read compute (no `"action_*"` / `"*_response"` temp IDs)
  - UI drafts: replaced per-keystroke business mutation (`edit_*_input`) with UI-local drafts seeded on open
- ✅ Internal Users: Remove `poll_internal_users_responses`
  - Deleted the polling bridge function entirely
  - Modals now detect Success/Error states directly from `InternalUsersActionCompute`
  - Added `ResetInternalUsersActionCommand` to reset compute to Idle after handling results
  - UI handles side effects directly: close action, trigger refresh, reset compute
  - Removed all exports and call sites for `poll_internal_users_responses`
- ✅ Internal Users: Simplify state update pattern
  - UI now uses `state_ctx.update::<T>(|s| ...)` for synchronous state mutations with auto dirty propagation
  - Commands use `Updater::set_state::<T>()` for async state updates (when state is Send-safe)
  - Removed unnecessary workflow commands that only existed to wrap state mutation calls
  - UI directly calls: `update::<InternalUsersState>(|s| s.start_action())`, `.close_action()`, `.toggle_otp_visibility()`, etc.
  - Updated `Updater` to support both `set::<Compute>()` and `set_state::<State>()` via `UpdateMessage` enum
  - Added `State::assign_box()` trait method for states that can be updated via Updater
  - States with non-Send types (e.g., `egui::TextureHandle`) cannot use `Updater::set_state()` - use `update()` or `state_mut` in UI
  - Un-ignored 14 integration tests that were blocked by the refactor

---

## Notes / guardrails

- Prefer `Ustr` for usernames/identifiers that are frequently cloned/compared.
- Avoid outcome strings like `"user_deleted"`; use enums.
- Do not introduce new `ctx.memory_mut`-based async transports. Use Commands + Computes.
- It is OK to use UI-local state for draft text and widget-only ephemeral UI behavior.
- **State update patterns:**
  - UI code: use `state_ctx.update::<T>(|s| ...)` for synchronous state mutations (auto dirty propagation)
  - UI code: use `state_ctx.state_mut::<T>()` for read-only access or binding to widgets
  - Commands: use `Updater::set_state::<T>()` for async callbacks (state must be Send-safe)
  - States with non-Send types (e.g., `egui::TextureHandle`) must be mutated via `update()` or `state_mut` in UI