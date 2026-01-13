# Autopsy Report: OTP Countdown Timer & Stale Code Display

**Date:** 2025-01  
**Affected Area:** Internal Users Panel (`ui/src/widgets/internal/users/`)  
**Severity:** User-visible regression  
**Status:** Fixed

---

## Summary

Two user-visible bugs occurred in the Internal Users panel:

1. **OTP countdown timer not auto-decreasing** — The "time remaining" display stayed frozen instead of counting down in real-time.
2. **Stale OTP code flash on reveal** — When clicking "Reveal", users saw an old/incorrect OTP code briefly before the correct one appeared.

Both bugs passed existing tests and CI, indicating gaps in our integration test coverage and architectural guardrails.

---

## Timeline of the Bug

1. Initial implementation added `calculate_time_remaining()` to compute live countdown based on elapsed time since last fetch.
2. OTP reveal flow was added with on-demand `GetUserOtpCommand` to fetch fresh codes.
3. Tests passed; code shipped.
4. Users reported: time doesn't count down, wrong code flashes on reveal.

---

## Root Cause Analysis

### Bug 1: Time Not Auto-Decreasing

**Symptom:** The `time_remaining` value computed correctly but never visually updated.

**Root Cause:** **Misunderstanding of egui's reactive rendering model.**

egui operates in "reactive" mode by default:
- `App::update()` is only called when egui believes a repaint is needed
- Repaints are triggered by: user interaction, explicit `request_repaint()`, or running animations
- **No user interaction = no repaint = no `update()` call = Time state never advances**

The code correctly updated `Time` state inside `update()`:

```rust
// In app.rs update()
if current_second != new_second {
    self.state.ctx.update::<Time>(|t| {
        *t.as_mut() = now;
    });
}
```

**The mistake:** Assuming `update()` runs continuously. It only runs when triggered. The Time update was inside the function that wasn't being called.

**Correct mental model:**
```
┌─────────────────────────────────────────────────────────────┐
│                     egui Event Loop                         │
│                                                             │
│   [User Input] ──┐                                          │
│                  ├──► request_repaint() ──► update() called │
│   [Animation] ───┘                                          │
│                                                             │
│   No trigger = update() NOT called = Time frozen            │
└─────────────────────────────────────────────────────────────┘
```

**Fix:** Explicitly request continuous repaints when OTPs are revealed:

```rust
if has_revealed_otps || otp_fetch_in_flight {
    ui.ctx().request_repaint_after(Duration::from_millis(100));
}
```

---

### Bug 2: Stale OTP Code Flash on Reveal

**Symptom:** When revealing OTP, users saw an incorrect code for ~1 frame before the correct one appeared.

**Root Cause:** **Incomplete state machine handling in UI rendering.**

The OTP display logic was:

```rust
// BEFORE (buggy)
let mut otp_code: &str = &data.user.current_otp;  // ← Default to stale list-users data

if let Some(compute) = state_ctx.cached::<InternalUsersActionCompute>() {
    match compute.state() {
        InternalUsersActionState::Otp { code, .. } => {
            otp_code = code;  // ← Only override when Otp state reached
        }
        _ => {}  // ← InFlight state falls through to stale default!
    }
}
```

**The mistake:** Not handling the `InFlight` state explicitly. The state machine has these transitions:

```
Click Reveal
    │
    ▼
┌─────────┐     ┌──────────┐     ┌─────────┐
│  Idle   │ ──► │ InFlight │ ──► │   Otp   │
└─────────┘     └──────────┘     └─────────┘
    │               │                 │
    │               │                 │
    ▼               ▼                 ▼
  (stale)        (stale)          (fresh)
  WRONG!         WRONG!           CORRECT
```

The code showed the stale `data.user.current_otp` during `Idle` and `InFlight` states.

**Fix:** Explicitly handle `InFlight` to show a loading spinner:

```rust
// AFTER (fixed)
match compute.state() {
    InternalUsersActionState::InFlight { kind: GetUserOtp, user }
        if user == this_user =>
    {
        is_loading = true;  // ← Show spinner, not stale code
    }
    InternalUsersActionState::Otp { code, .. } => {
        otp_code = Some(code);
    }
    _ => {}
}

if is_loading {
    render_spinner();
} else {
    render_otp_code(otp_code.unwrap_or(stale_fallback));
}
```

---

## Why Existing Tests Didn't Catch This

### Gap 1: No Time-Based UI Update Tests

**Current state:** Tests step frames manually but don't test that UI updates *over time* without user interaction.

**Missing test pattern:**
```rust
#[test]
fn test_otp_countdown_updates_over_time() {
    let harness = setup_with_revealed_otp();
    
    // Get initial time remaining
    let initial = query_time_remaining(&harness);
    
    // Advance virtual time by 5 seconds (no user interaction)
    harness.advance_time(Duration::from_secs(5));
    harness.run();  // Should auto-repaint
    
    let after_5s = query_time_remaining(&harness);
    assert_eq!(after_5s, initial - 5, "Countdown should decrease over time");
}
```

**Why it's hard:** `egui_kittest` doesn't naturally simulate "time passing without interaction." Need to:
- Mock/control `Time` state explicitly
- Verify `request_repaint_after()` was called
- Simulate the repaint trigger

### Gap 2: No Loading State Transition Tests

**Current state:** Tests verify final states but not intermediate transitions.

**Missing test pattern:**
```rust
#[test]
fn test_otp_reveal_shows_loading_before_code() {
    let harness = setup_with_user();
    
    // Click reveal
    harness.click_button("Reveal");
    harness.run();
    
    // IMMEDIATELY after click, should show spinner (InFlight state)
    assert!(
        harness.query_by_role("spinner").is_some(),
        "Should show loading spinner while fetching OTP"
    );
    assert!(
        harness.query_by_label_contains("123456").is_none(),
        "Should NOT show any OTP code while loading"
    );
    
    // Complete the mock fetch
    harness.complete_pending_commands();
    harness.run();
    
    // NOW should show the fresh code
    assert!(harness.query_by_label_contains("654321").is_some());
}
```

**Why it's hard:** Need fine-grained control over command execution timing to test intermediate states.

### Gap 3: Stale Data Assertions

**Current state:** Tests don't distinguish between "correct fresh data" and "stale data that happens to look okay."

**Missing test pattern:**
```rust
#[test]
fn test_revealed_otp_is_from_ondemand_fetch_not_list() {
    // Setup: list-users returned OTP "111111"
    // On-demand fetch will return "222222"
    let harness = setup_with_different_list_vs_ondemand_otp();
    
    harness.click_button("Reveal");
    harness.complete_pending_commands();
    harness.run();
    
    // Must show on-demand OTP, not list-users OTP
    assert!(
        harness.query_by_label_contains("222222").is_some(),
        "Should show fresh on-demand OTP"
    );
    assert!(
        harness.query_by_label_contains("111111").is_none(),
        "Should NOT show stale list-users OTP"
    );
}
```

---

## Architectural Gaps Identified

### 1. No Continuous Update Contract

**Problem:** Nothing in `state-model.md` or code enforces that time-sensitive UIs must request repaints.

**Recommendation:** Add to `state-model.md`:

```markdown
## Time-sensitive UI updates

If a widget displays values that change over time (countdowns, timers, animations):
1. The widget MUST call `ctx.request_repaint_after(duration)` while active
2. Tests MUST verify the repaint request is made
3. Consider using a `TimeSensitive` marker trait or helper
```

### 2. No State Machine Completeness Check

**Problem:** When a Compute has multiple states (`Idle`, `InFlight`, `Success`, `Error`), UI code can easily miss handling one.

**Recommendation:** Consider:
- Exhaustive match requirements (remove `_ => {}` catch-all)
- Clippy lint for non-exhaustive enum matches in UI code
- State machine visualization in docs

### 3. No "Intermediate State" Test Helpers

**Problem:** Testing loading/transition states requires manual timing control that's awkward with current harness.

**Recommendation:** Add test utilities:
```rust
impl TestHarness {
    /// Run one frame but don't flush commands yet
    fn step_without_flush(&mut self);
    
    /// Check if repaint was requested
    fn was_repaint_requested(&self) -> bool;
    
    /// Get pending command count
    fn pending_commands(&self) -> usize;
}
```

---

## Action Items

### Immediate (this PR)
- [x] Fix continuous repaint for OTP countdown
- [x] Fix loading state handling for OTP reveal
- [x] Add `OTPs` to typos allowlist

### Follow-up (future work)
- [ ] Add integration test for countdown auto-update behavior
- [ ] Add integration test for loading state during OTP fetch
- [ ] Add integration test verifying fresh vs stale OTP source
- [ ] Document continuous-update pattern in `state-model.md`
- [ ] Consider exhaustive match lint for action state enums
- [ ] Add test harness helpers for intermediate state testing

---

## Lessons Learned

1. **egui's reactive model is opt-in continuous:** Default is "update only on interaction." Any auto-updating UI must explicitly request repaints.

2. **State machines need exhaustive UI handling:** Every state (`Idle`, `InFlight`, `Success`, `Error`) must have explicit UI representation. Using `_ => {}` hides bugs.

3. **"Works on my machine" ≠ "Works over time":** Manual testing naturally involves mouse movement (triggering repaints). Automated tests and idle users don't.

4. **Stale data is invisible in happy-path tests:** If stale and fresh data look similar, tests pass but users see glitches. Tests need deliberately different values to catch staleness bugs.

---

## References

- Fix PR: `fix/otp-countdown-and-loading-state`
- Related docs: `docs/ai/state-model.md`, `docs/ai/testing.md`
- egui repaint docs: https://docs.rs/egui/latest/egui/struct.Context.html#method.request_repaint