# Autopsy Reports

This directory contains post-mortem analyses of significant bugs that escaped testing and reached production or late-stage development.

## Purpose

Autopsy reports serve as:

1. **Learning resources** — Understand root causes and failure modes
2. **Test coverage guides** — Identify gaps in testing strategy
3. **Architectural insights** — Surface systemic issues that enable bugs
4. **Onboarding material** — Help new contributors understand past mistakes

## When to write an autopsy

Create an autopsy report when:

- A user-visible bug escaped all tests and CI
- The bug reveals a gap in testing methodology or architectural guardrails
- The failure mode is non-obvious or could recur in different contexts
- Multiple related bugs stem from the same root cause

**Don't write an autopsy for:**
- Simple typos or one-off mistakes
- Bugs caught by code review before merge
- Issues with clear test coverage that just weren't written yet

## Report structure

Each autopsy should include:

### 1. Summary
- Date and affected area
- Severity and status
- High-level description of what went wrong

### 2. Timeline
- How the bug was introduced
- When it was discovered
- Key events in between

### 3. Root cause analysis
- Technical explanation of the bug
- Mental model failures or misconceptions
- Code examples showing the mistake

### 4. Why tests didn't catch it
- Specific gaps in test coverage
- Missing test patterns
- Testing infrastructure limitations

### 5. Architectural gaps
- Systemic issues that enabled the bug
- Missing guardrails or contracts
- Design patterns that could prevent similar bugs

### 6. Action items
- Immediate fixes
- Follow-up work needed
- Long-term improvements

### 7. Lessons learned
- Key takeaways for the team
- Mental models to internalize
- Patterns to watch for

## Naming convention

Use the format: `<component>-<issue-key>-YYYY-MM.md`

Examples:
- `otp-countdown-2025-01.md`
- `database-migration-2025-03.md`
- `state-sync-race-2025-06.md`

## Index of reports

- [`otp-countdown-2025-01.md`](./otp-countdown-2025-01.md) — OTP countdown timer bugs (4 related issues)
  - egui reactive rendering model
  - State machine completeness
  - Hidden state testing gaps
  - Time-sensitive UI patterns

---

## How to use these reports

### For AI assistants
When working on related code areas, reference the relevant autopsy to:
- Understand past failure modes
- Check if new code could reintroduce similar bugs
- Apply lessons learned to current work

### For code reviewers
Use autopsy reports to:
- Validate that similar bugs can't occur
- Ensure test coverage addresses known gaps
- Check for architectural improvements mentioned in action items

### For new contributors
Read autopsy reports in your area of focus to:
- Learn domain-specific gotchas
- Understand team's testing philosophy
- See examples of root cause analysis

---

**Note:** These reports are living documents. If new information emerges or related bugs are found, update the existing report rather than creating a new one.