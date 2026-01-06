# Collects

This repository contains multiple Rust crates (UI, services, business logic, state management, scripts) that together make up the Collects application.

This README documents the architectural direction for the UI state model, specifically the migration away from `egui::Context` memory (`ctx.memory_mut`) as an async message bus and toward the repo’s intended **StateCtx + Compute + Command** pattern.

---

## State model: the rule we enforce

### UI must not mutate business state directly

**UI code under `ui/` must not call `state_ctx.state_mut::<BusinessState>()` (or similar) to modify business/domain state.**

Instead:

- UI **reads** via `StateCtx` caching (e.g. `ctx.cached::<T>()`).
- UI **writes** by dispatching **Commands**.
- Async results are stored in **Computes** and updated only via `Updater::set()` inside Commands.

This keeps side effects centralized, state transitions traceable, and enables testable async flows without frame-order coupling.

---

## What is `ctx.memory_mut` and why we’re removing it

In `egui`, `ctx.memory_mut(|mem| ...)` gives mutable access to egui’s internal `Memory`, including `mem.data`, which is an `Id`-keyed storage map.

In this repo, `ctx.memory_mut` has been used as:

- a **temporary storage** for async IO results (e.g. “users list response”, “action error”)
- a **message bus** between async callbacks and the next UI frame
- coupled with a poller function that:
  - reads temps via `ctx.memory(|mem| mem.data.get_temp(...))`
  - writes into app state
  - removes temps via `mem.data.remove(...)`

### Problems with this pattern

- **Stringly-typed IDs** (`"internal_users_response"`, `"action_success"`, etc.) are not enforced by the compiler.
- **Hidden coupling** across widgets and frames (callback writes → poller reads later).
- **Harder to test**: logic is split between callback, memory storage, polling, and state mutation.
- **Not aligned** with “Commands update Computes” rule, and is easy to accidentally expand.

We keep egui memory for legitimate widget-internal UI concerns, but we do *not* use it for domain/workflow async result transport.

---

## Migration strategy: Split workflow State vs async Computes (Hybrid Inputs)

We migrate features (starting with internal-users) to the following model:

### Business State (workflow / selection only)
Business State should contain durable workflow state such as:

- which panel/modal/action is open
- which record/user is selected (use `Ustr` for identifiers)
- UI workflow flags that must persist (if any)

**Business State does not contain chatty draft text** for text edits unless persistence across navigation is required.

### Business Computes (async outcomes)
Async operations are represented by Computes, e.g.:

- `InternalUsersListCompute`: `Idle | Loading | Loaded(Vec<...>) | Error(String)`
- `InternalUsersActionCompute`: `Idle | InFlight { kind, user } | Success { ... } | Error { ... }`

Computes are updated only through Commands via `Updater::set()`.

### UI-local drafts (hybrid inputs)
For chatty inputs (username/nickname/avatar URL), we use **UI-local draft strings** that:

- are **seeded** from business data when a modal opens / selection changes
- are **committed** when the user clicks “Save/Update” (dispatch a command)
- are not written into business state on every keystroke

This reduces command spam and keeps business state free of half-typed values.

---

## Internal-users migration plan (from `ctx.memory_mut` to Compute + Command)

### Current state
Internal-users widgets currently use `ctx.memory_mut` to store:

- list users responses/errors
- action success/error strings
- QR/OTP revoke response payloads

…and a poller copies these into `InternalUsersState`.

### Target state
- List/users and action results flow through **Computes**
- Async work lives in **Commands**
- UI becomes:
  - read: `cached::<Compute>()`
  - write: `dispatch::<Command>()`
- Remove the poller and all `Id`-keyed temp storage.

---

## TODO checklist (ordered)

### Phase 0 — Guardrails
- Add guidance to repo instructions: do not introduce new `ctx.memory_mut` usage in widgets for async transport.
- Prefer typed Computes/Commands for async result storage.

### Phase 1 — Refresh slice (first vertical refactor)
This is the smallest async flow to migrate.

**What**
- Move “Refresh users list” off egui memory temps and into:
  - `InternalUsersListCompute`
  - `InternalUsersRefreshCommand`

**Why**
- Eliminates the most frequent and simplest `ctx.memory_mut` usage.
- Establishes the Compute+Command template for other actions.

**How**
- Business:
  - add `InternalUsersListCompute` with typed status
  - add `InternalUsersRefreshCommand`:
    - sets compute to `Loading`
    - performs fetch
    - sets compute to `Loaded/Error`
- UI:
  - refresh button dispatches `InternalUsersRefreshCommand`
  - table renders from `InternalUsersListCompute`
  - remove list-related:
    - `ctx.memory_mut(... "internal_users_response"/"internal_users_error" ...)`
    - list-related polling/removal logic

**Benefits**
- Removes frame-order dependence for the list.
- Removes two temp `Id` keys.
- Makes list loading/error UI deterministic and testable.

### Phase 2 — Action commands + action compute
Migrate one modal action at a time:

- Update username
- Update profile
- Delete user
- Revoke OTP
- (Optional) Fetch QR code

**What**
- Introduce `InternalUsersActionCompute` and per-action Commands.

**How**
- Each action command:
  - sets `ActionCompute` to `InFlight`
  - runs async API call
  - sets `ActionCompute` to `Success/Error`
  - triggers a refresh (ideally from the command)

**Benefits**
- Delete `"action_success"` / `"action_error"` temp keys
- Remove stringly-typed “outcome” values like `"user_deleted"`
- Centralize side effects; simplify modal rendering

### Phase 3 — Remove `poll_internal_users_responses`
Once all async flows are on Computes:
- delete the poller
- delete all remaining `ctx.memory_mut` usage for internal-users async transport

---

## Definition of done (internal-users)
- No `ctx.memory_mut`/`mem.data.*temp*` used for internal-users async results.
- No poller required for internal-users.
- UI does not directly mutate business state; it dispatches commands.
- Async results are represented by typed Computes updated via Commands.
- Inputs use UI-local drafts seeded from business data, committed on submit.

---

## Notes
- Keep using `Ustr` for identifiers/usernames that are cloned/compared frequently.
- Avoid magic strings for outcomes; prefer enums in business.
- If a draft needs to persist across navigation, promote it to business state but still update it via commands (not direct mutation).