# TODO: Refactor legacy async egui-memory patterns (ctx.memory_mut)

This file tracks remaining places where UI code uses `egui::Context` memory (`ctx.memory_mut` / `mem.data.insert_temp/get_temp/remove`) as an **async message bus**.

## Goal

Migrate each legacy flow to the repo’s reference pattern:

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

---

## Remaining legacy refactors (in priority order)

### 1) Internal Users: Migrate action/result plumbing off egui memory

**Files**
- `ui/src/widgets/internal/users/modals.rs`
- `ui/src/widgets/internal/users/panel.rs` (still polls action temps)

**Current egui-memory IDs used**
- `"action_error"`: String error message
- `"action_success"`: String outcome (`"user_deleted"`, `"username_updated"`, `"profile_updated"`)
- `"user_qr_code_response"`: String `otpauth_url` (QR data)
- `"revoke_otp_response"`: String `otpauth_url`

**What to build (business)**
- `InternalUsersActionCompute`
  - strongly typed: `Idle | InFlight { kind, user } | Success { kind, user, data? } | Error { kind, user, message }`
- One command per operation (manual-only; async in command; `Updater::set()` updates compute):
  - `UpdateUsernameCommand`
  - `UpdateProfileCommand`
  - `DeleteUserCommand`
  - `RevokeOtpCommand`
  - (optional) `GetUserQrCommand` if QR fetch is separate

**UI changes**
- Remove writes to egui memory in callbacks
- Remove polling branches in `poll_internal_users_responses` for those IDs
- Render modal loading/success/error directly from `InternalUsersActionCompute`
- On success:
  - close modal via a command (business workflow state change)
  - trigger refresh (ideally from the command that succeeded)

**Hybrid drafts (UI-local)**
- Replace business fields:
  - `InternalUsersState.edit_username_input`
  - `InternalUsersState.edit_nickname_input`
  - `InternalUsersState.edit_avatar_url_input`
- With UI-local drafts seeded from selected user:
  - seed once when opening `EditUsername/EditProfile` for a given username
  - commit on submit by dispatching the appropriate command with final values
  - do not update business state on every keystroke

**Benefits**
- Eliminates `"action_*"` magic strings and egui temp IDs
- Deletes the poller entirely once all actions are migrated
- Makes actions typed and testable

---

### 2) Internal Users: Remove direct business state mutation from UI

Even after async compute migration, UI currently calls methods like:
- `state_ctx.state_mut::<InternalUsersState>().start_action(...)`
- `.toggle_otp_visibility(...)`
- `.open_create_modal()`, `.close_action()`, etc.

**Target**
- UI dispatches workflow commands instead:
  - `OpenInternalUsersActionCommand(UserAction)`
  - `CloseInternalUsersActionCommand`
  - `ToggleOtpVisibilityCommand { username: Ustr }`
  - `OpenCreateUserModalCommand`, `CloseCreateUserModalCommand`, etc.

---

### 3) Internal Users: Remove `poll_internal_users_responses`

Once both:
- list refresh uses compute (done)
- all actions use `InternalUsersActionCompute` (pending)

…then `poll_internal_users_responses` should be deleted.

---

## Notes / guardrails

- Prefer `Ustr` for usernames/identifiers that are frequently cloned/compared.
- Avoid outcome strings like `"user_deleted"`; use enums.
- Do not introduce new `ctx.memory_mut`-based async transports. Use Commands + Computes.
- It is OK to use UI-local state for draft text and widget-only ephemeral UI behavior.