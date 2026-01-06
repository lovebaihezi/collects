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
- ✅ Internal Users: Actions migrated off egui-memory temps
  - Business: added `InternalUsersActionCompute` + typed `InternalUsersActionState/Kind`
  - Commands: `UpdateUsernameCommand`, `UpdateProfileCommand`, `DeleteUserCommand`, `RevokeOtpCommand`, `GetUserQrCommand`
  - UI: modals + inline QR now dispatch commands and read compute (no `"action_*"` / `"*_response"` temp IDs)
  - UI drafts: replaced per-keystroke business mutation (`edit_*_input`) with UI-local drafts seeded on open

---

## Remaining legacy refactors (in priority order)

### 2) Internal Users: Remove direct business state mutation from UI

Even after async compute migration, UI still calls methods like:
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

Now that:
- list refresh uses compute (done)
- all actions use `InternalUsersActionCompute` (done)

…`poll_internal_users_responses` should be deleted entirely (and removed from any call sites/exports).

---

## Notes / guardrails

- Prefer `Ustr` for usernames/identifiers that are frequently cloned/compared.
- Avoid outcome strings like `"user_deleted"`; use enums.
- Do not introduce new `ctx.memory_mut`-based async transports. Use Commands + Computes.
- It is OK to use UI-local state for draft text and widget-only ephemeral UI behavior.