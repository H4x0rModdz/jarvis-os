# ADR 0018: Voiceprint V1 — Temporal Envelope, Naive Matcher

## Status
Accepted (V1 scaffold superseded by V2 — see "V2 update" below).

## V2 update (Phase 6)

V2 landed: MFCC + DTW matcher in `voiceprint.rs`. The wire contract
held (`EnrollVoiceprint`, `VerifyVoiceprint`, `score` field,
`threshold` field) — only the body of `extract_features` and
`similarity` moved. V1 rows in the SQLite store are detected at
fetch time and rejected (re-enrollment prompt), so the absence of
a database migration is acceptable: V1 rows were scaffold prints by
definition and never trusted by a shipping service.

The V2 matcher uses:
- 25 ms windows / 10 ms hop, pre-emphasis 0.97
- 26-band mel filterbank (0 — 8 kHz Nyquist)
- 13 MFCC coefficients per frame (DCT-II)
- Unconstrained DTW for sequence alignment
- Similarity = `1 / (1 + dist/30)`; threshold 0.6

The classical MFCC+DTW pipeline discriminates speakers well in the
closed-set, low-noise case. It is **beatable by replay attacks**
(an attacker holding a recording of the enrolled user). Phase 7 may
swap to x-vector / d-vector embeddings via ONNX for proper
anti-spoofing — `pam-jarvis` V2 wiring should treat the score as
useful but not absolutely trusted, mirroring how face-unlock is
typically configured.

The text below was the original V1 ADR. Kept as-is for history.

---


## Context

ADR 0016 picked PAM as the place biometric auth lives in LilithOS,
shipped a PAM_IGNORE scaffold, and deferred the matching itself to
"V2". Phase 5 needs to land enough of the matching path that
downstream surfaces (pam-jarvis V2, settings UI for enrollment,
Lilith tool) can be built against a real DBus contract, not against
a TODO.

The real biometric problem is hard:

- **MFCC + DTW.** Classic approach. Computes per-frame mel-frequency
  cepstral coefficients, aligns sequences via dynamic time warping.
  10× the code we have for whisper integration, and the matcher
  alone is a project.
- **x-vectors / d-vectors.** Embedding network output for speaker
  identification. Best-in-class, but needs an ONNX runtime in the
  daemon and a model file (~10 MB) shipped.
- **Speaker recognition crate.** None in the Rust ecosystem are
  production-quality.

Phase 5's budget can't honestly ship any of these.

## Decision

Ship a **temporal log-RMS envelope** matcher as V1. Document loudly
that it's not a real voiceprint, and define the V2 upgrade contract
so callers (PAM, UI, Lilith) target the right API surface today.

### What V1 actually does

1. Capture N seconds of mono 16 kHz audio (reuses the existing
   capture pipeline).
2. Slice into 100 ms windows.
3. Per window: `log10(1 + RMS(samples))`.
4. Result: a `Vec<f32>` of length `N * 10`.
5. Verify: capture ~2 s, compute envelope, take the shorter of
   stored vs. probe, cosine similarity.
6. Threshold at 0.85 (eyeballed on one dev machine).

### What V1 does NOT do

- It does not capture spectral identity. Two recordings of the same
  phrase by the same speaker score high; a different speaker
  matching pacing also scores high.
- It is not biometric strength. Treat as "noise-floor authentication"
  — a UI affordance to demonstrate the round-trip, not a security
  mechanism that prevents impersonation.

### V2 contract

V2 replaces the body of `extract_features` (and possibly `similarity`)
without touching the DBus surface. The wire contract:

- `EnrollVoiceprint(user, seconds)` → `{ ok, frames? }`
- `VerifyVoiceprint(user)` → `{ ok, score, threshold }`

stays identical. `frames` becomes "frames of MFCC vectors" in V2.
`score` becomes a real similarity metric. `threshold` may move; the
field is exposed precisely so the caller can render it.

Storage schema (`voiceprints(user, features_json, enrolled_at)`)
holds vectors of arbitrary length, so longer V2 vectors slot in
without a migration.

## Consequences

**Good:**
- Daemon ships with a real round-trip — settings UI, Lilith tools,
  pam-jarvis V2 can be built against the actual contract.
- Storage layout (`~/.jarvis/voiceprints.db`, SQLite) is V2-ready.
- Swapping V2's matcher is a single-module change.

**Bad:**
- Anyone reading "voiceprint" expects biometric strength. The
  module.md table makes V1 ↔ V5 clear, but a casual user could be
  misled into trusting the V1 verdict. **No PAM service should be
  configured to trust pam-jarvis V2 against V1 voiceprints in any
  shipping ISO** — that wiring is gated behind the V2 matcher.
- One more SQLite file per user.

## Alternatives Considered

- **Skip voiceprint entirely until Phase 6 has a real matcher.**
  Tempting, but leaves the settings UI / Lilith tools / pam-jarvis
  V2 work all blocked on the same future commit. Decoupling the
  contract from the body now means Phase 6 is just a Rust function
  swap.
- **Stub matcher that always returns 1.0.** Worse than the naive
  envelope: the failure mode is silent (everyone always succeeds),
  and the round-trip looks like it works in dev with no signal
  that it's broken. The naive matcher at least scores enrolled vs.
  cross-speaker differently when the speakers are sufficiently
  different.
