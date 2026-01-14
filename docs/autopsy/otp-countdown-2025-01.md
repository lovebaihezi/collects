# Autopsy Report: OTP Countdown Timer & Stale Code Display

**Date:** 2025-01  
**Affected Area:** Internal Users Panel (`ui/src/widgets/internal/users/`)  
**Severity:** User-visible regression  
**Status:** Fixed (all four bugs resolved)

---

## Summary

Four user-visible bugs occurred in the Internal Users panel:

1. **OTP countdown timer not auto-decreasing** — The "time remaining" display stayed frozen instead of counting down in real-time. ✅ FIXED
2. **Stale OTP code flash on reveal** — When clicking "Reveal", users saw an old/incorrect OTP code briefly before the correct one appeared. ✅ FIXED
3. **Hidden OTP countdown not starting** — When all OTPs are hidden, the countdown timer doesn't advance because no repaints are requested. ✅ FIXED
4. **Hidden OTP not refreshing at zero** — When a hidden OTP's countdown reaches zero, it doesn't trigger a refresh, so revealing it later shows a stale code. ✅ FIXED

All bugs passed existing tests and CI, indicating significant gaps in our integration test coverage and architectural guardrails.

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

**Note:** This fix introduced Bug #3 (see below).

---

### Bug 3: Hidden OTP Countdown Not Starting

**Symptom:** When all OTPs are hidden, the countdown timer doesn't advance. If a user reveals an OTP after it's been hidden for a while, the displayed time is stale.

**Root Cause:** **Overly narrow repaint condition.**

The fix for Bug #1 added:

```rust
let has_revealed_otps = !state_ctx
    .state::<InternalUsersState>()
    .revealed_otps
    .is_empty()
    && state_ctx
        .state::<InternalUsersState>()
        .revealed_otps
        .values()
        .any(|&revealed| revealed);

if has_revealed_otps || otp_fetch_in_flight {
    ui.ctx().request_repaint_after(Duration::from_millis(100));
}
```

**The mistake:** Only requesting repaints when OTPs are *revealed*. When all OTPs are hidden:
- No repaint is requested
- Time state doesn't advance
- When user reveals an OTP later, the countdown shows stale time

**Example failure scenario:**
```
1. User loads panel, sees "User A" with "25s remaining" (hidden)
2. User waits 10 seconds doing nothing (all OTPs hidden, no repaints)
3. User clicks "Reveal" on User A
4. UI shows "25s remaining" (WRONG! Should be ~15s)
```

**Fix options:**

Option A: **Always request repaints when users have OTPs** (even if hidden)
```rust
let has_users_with_otps = !users.is_empty();
if has_users_with_otps || otp_fetch_in_flight {
    ui.ctx().request_repaint_after(Duration::from_millis(100));
}
```

Option B: **Force refresh when revealing hidden OTP**
```rust
// In toggle logic:
if was_hidden && now_revealed {
    // Fetch fresh OTP data since time may be stale
    enqueue_get_otp_command(username);
}
```

**Recommendation:** Option A is more accurate and simpler. The CPU cost of 10 repaints/second is negligible when the panel is open.

---

### Bug 4: Hidden OTP Not Refreshing at Zero

**Symptom:** When a hidden OTP's countdown reaches zero, it doesn't trigger a refresh. Revealing it later shows a stale OTP code from the previous cycle.

**Root Cause:** **Stale-check only examines revealed OTPs.**

The auto-refresh logic in `request_refresh_if_otp_stale()`:

```rust
let any_stale = users.iter().any(|user| {
    let username = Ustr::from(&user.username);
    let is_revealed = state.is_otp_revealed_at(&username, now);
    is_revealed && state.is_otp_stale(user.time_remaining, now)
    // ↑ Only checks revealed OTPs!
});
```

**The mistake:** Assuming we only care about staleness for visible OTPs. But hidden OTPs still cross 30-second boundaries, and users expect fresh codes when they reveal them.

**Example failure scenario:**
```
1. User has "User A" with OTP hidden, 5s remaining
2. 10 seconds pass (OTP crossed boundary, but refresh not triggered because hidden)
3. User clicks "Reveal" on User A
4. UI shows OLD OTP code from previous cycle (WRONG!)
```

**Why this is subtle:** Bug #3 might mask this! If Time is frozen due to no repaints, `is_otp_stale()` might not detect staleness even for revealed OTPs.

**Fix options:**

Option A: **Check all OTPs for staleness** (hidden or not)
```rust
let any_stale = users.iter().any(|user| {
    state.is_otp_stale(user.time_remaining, now)
    // Remove is_revealed check entirely
});
```

Option B: **Fetch fresh OTP on reveal if potentially stale**
```rust
// In toggle logic:
if was_hidden && now_revealed {
    let time_remaining = /* get from user data */;
    if state.is_otp_stale(time_remaining, now) {
        enqueue_get_otp_command(username);
    }
}
```

**Recommendation:** Option A is cleaner and ensures the user list stays fresh regardless of visibility state. The API cost is one refresh per 30 seconds per user, which is acceptable.

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

## Why Existing Tests Didn't Catch These Issues

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

### Gap 4: No Hidden State Behavior Tests

**Current state:** Tests focus on revealed/visible OTPs, not hidden ones.

**Missing test patterns:**

```rust
#[test]
fn test_hidden_otp_countdown_stays_accurate() {
    let harness = setup_with_user_otp_hidden();
    
    let initial = query_time_remaining(&harness);
    
    // Advance time 10 seconds while OTP is hidden
    harness.advance_time(Duration::from_secs(10));
    harness.run();
    
    // Reveal the OTP
    harness.click_button("Reveal");
    harness.complete_pending_commands();
    harness.run();
    
    let after_reveal = query_time_remaining(&harness);
    
    // Should show accurate time, not stale time from before wait
    assert_eq!(
        after_reveal,
        initial - 10,
        "Hidden OTP countdown should advance accurately"
    );
}

#[test]
fn test_hidden_otp_refreshes_when_cycle_expires() {
    let harness = setup_with_user_otp_hidden_5s_remaining();
    
    // Advance time past the 30-second cycle boundary (10 seconds)
    harness.advance_time(Duration::from_secs(10));
    harness.run();
    
    // Should have triggered auto-refresh even though OTP was hidden
    assert!(harness.api_calls_include("GET /internal/users"));
    
    // Reveal the OTP
    harness.click_button("Reveal");
    harness.complete_pending_commands();
    harness.run();
    
    // Should show FRESH code from new cycle, not stale code
    let displayed_code = query_otp_code(&harness);
    assert_eq!(
        displayed_code,
        "NEW_CYCLE_CODE",
        "Should show fresh OTP from new cycle"
    );
}
```

**Why it's hard:** Tests need to:
- Control time advancement independently of user interaction
- Track whether repaints were requested
- Verify refresh triggers even when UI elements are hidden
- Distinguish between "stale but not yet refreshed" vs "fresh from new cycle"

---

## Architectural Gaps Identified

### 1. No Continuous Update Contract

**Problem:** Nothing in `state-model.md` or code enforces that time-sensitive UIs must request repaints.

**Additional problem:** The repaint condition is easy to make too narrow (only revealed OTPs) when it should be broader (all OTPs in list).

**Recommendation:** Add to `state-model.md`:

```markdown
## Time-sensitive UI updates

If a widget displays values that change over time (countdowns, timers, animations):

1. The widget MUST call `ctx.request_repaint_after(duration)` while active
2. Consider carefully what "active" means:
   - Does hidden data also need accurate time? (Usually YES)
   - Should repaints continue for background state? (Usually YES if visible on reveal)
3. Tests MUST verify:
   - Repaint request is made when data is visible
   - Repaint request is made when data is hidden but time-sensitive
   - Time advances correctly in both cases
4. Consider using a `TimeSensitive` marker trait or helper
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
    
    /// Check if specific repaint interval was requested
    fn was_repaint_requested_after(&self, duration: Duration) -> bool;
}
```

### 4. No Hidden State Coverage Requirements

**Problem:** Tests naturally focus on visible UI elements, missing bugs in hidden/background state.

**Recommendation:**
- Add test coverage requirements for hidden states
- Lint or review checklist: "If state has visibility toggle, test both states"
- Document pattern: "Test reveal-after-delay scenarios"

Example checklist item:
```markdown
- [ ] Tested visible state behavior
- [ ] Tested hidden state behavior
- [ ] Tested reveal-after-hidden behavior
- [ ] Tested hide-reveal-hide cycle
```

---

## Action Items

### Immediate (initial fix)
- [x] Fix continuous repaint for OTP countdown
- [x] Fix loading state handling for OTP reveal
- [x] Add `OTPs` to typos allowlist

### Second Fix (hidden OTP bugs)
- [x] Fix repaint condition to include hidden OTPs (Bug #3)
- [x] Fix stale-check to include hidden OTPs (Bug #4)
- [ ] Add integration test for hidden OTP countdown accuracy
- [ ] Add integration test for hidden OTP refresh at cycle boundary

### Follow-up (future work)
- [ ] Add integration test for countdown auto-update behavior (visible)
- [ ] Add integration test for loading state during OTP fetch
- [ ] Add integration test verifying fresh vs stale OTP source
- [ ] Document continuous-update pattern with visibility considerations in `state-model.md`
- [ ] Consider exhaustive match lint for action state enums
- [ ] Add test harness helpers for intermediate state testing
- [ ] Add test harness helper for repaint tracking
- [ ] Add hidden state coverage to test checklists

---

## Lessons Learned

1. **egui's reactive model is opt-in continuous:** Default is "update only on interaction." Any auto-updating UI must explicitly request repaints.

2. **State machines need exhaustive UI handling:** Every state (`Idle`, `InFlight`, `Success`, `Error`) must have explicit UI representation. Using `_ => {}` hides bugs.

3. **"Works on my machine" ≠ "Works over time":** Manual testing naturally involves mouse movement (triggering repaints). Automated tests and idle users don't.

4. **Stale data is invisible in happy-path tests:** If stale and fresh data look similar, tests pass but users see glitches. Tests need deliberately different values to catch staleness bugs.

5. **Visibility conditions are deceptively narrow:** "Request repaints when revealed" feels correct but breaks accuracy for hidden data. Time-sensitive state often needs updates regardless of visibility.

6. **Hidden state is untested by default:** Tests naturally focus on visible UI elements. Hidden state bugs require deliberate "reveal-after-delay" test scenarios.

7. **Fixing one edge case can create another:** Bug #1 fix (request repaint when revealed) created Bug #3 (don't request when hidden). Need to think through the full state space, not just the reported symptom.

---

## References

- Initial fix PR: `fix/otp-countdown-and-loading-state` (Bugs #1 and #2)
- Second fix: Same session (Bugs #3 and #4)
- Related docs: `docs/ai/state-model.md`, `docs/ai/testing.md`
- egui repaint docs: https://docs.rs/egui/latest/egui/struct.Context.html#method.request_repaint
- Code file: `ui/src/widgets/internal/users/panel.rs`