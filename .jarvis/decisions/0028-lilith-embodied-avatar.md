# 0028 — Lilith embodied avatar (3D companion)

Status: accepted
Date: 2026-06-19

## Context

Lilith today is a glyph orb in the dock (`LilithOrb.qml`) plus a chat popup
(`LilithPopup.qml`). The product ambition is for Lilith to be *embodied* — a
3D presence with a face that reacts, lip-sync when she speaks, and emotional
expression — not just a text box. This is the headline "realista" feature the
project has wanted since the start.

The honest framing before committing: **the hard, expensive part is the art
asset (a rigged 3D model with viseme + emotion blendshapes), not our code.**
The animation *runtime* — the thing that reads system state and drives the
model — is bounded engineering and works the moment a real model is dropped
in. So this ADR commits to building the runtime now, against a placeholder,
with the model as a swappable asset.

Two real constraints shaped the decision:

- **GPU in the VM.** We develop/test in VMware, which gives OpenGL via
  software (llvmpipe). A photoreal model with morph targets at 60fps will
  *not* run there. This is a real ceiling, not a detail.
- **One image build ≈ 40 min.** The 3D rendering path cannot be verified on
  this dev host (no GPU, no asset), so it will need on-device iteration —
  same honest situation as Kvantum theming (ADR-tracked, tuned on hardware).

## Decision

1. **Render with Qt Quick 3D, model format is VRM.** The shell gains a
   `View3D` (new `Qt6::Quick3D` dependency). The avatar model is **VRM** —
   the VTuber-ecosystem standard, which is glTF 2.0 plus a humanoid +
   expression/viseme convention, so models come pre-rigged with the A/I/U/E/O
   visemes and happy/angry/sad/relaxed/surprised expressions we need. Loaded
   at runtime via `RuntimeLoader` (QtQuick3D.Helpers) from
   `~/.local/share/jarvis/avatar/lilith.vrm`.

2. **A procedural fallback so it renders on day one.** With no `.vrm` present
   (the dev situation now), the avatar renders a stylized primitive head built
   from Quick3D primitives that reacts to the same channels. This keeps the
   whole pipeline testable and visible before any art exists — the model is a
   drop-in upgrade, not a prerequisite.

3. **A floating, draggable companion window.** Lilith lives in a frameless,
   transparent, always-on-top window the user can drag to any corner — a
   desktop companion, not a panel. It folds into the conversation when spoken
   to. (Chosen over popup-only / full-screen.)

4. **Three drive channels feed the avatar.** This is the architecture:
   - **State** → idle / listening / thinking / speaking. *Already exists*
     (`VoiceBridge.state` + `LilithBridge.busy`); drives idle pose, listening
     glow, thinking motion, speaking emphasis.
   - **Emotion** → Lilith tags each reply with a coarse emotion
     (neutral / happy / thinking / concerned); a new DBus signal + bridge
     property maps it to expression morphs (or fallback color/animation).
   - **Mouth** → the voice daemon, during TTS playback, emits an audio
     amplitude level; a bridge property drives the jaw/mouth open weight.
     v1 is amplitude-driven lip *flap* (the standard cheap-but-convincing
     trick); phoneme→viseme timing from Piper is the documented upgrade.

5. **Model tiers = asset swap.** `basic` (low-poly / stylized, runs on
   software GL) and `realistic` (high-fidelity, needs a real GPU) are the same
   runtime pointing at different `.vrm` files, selectable in Preferences. No
   separate code path.

## Consequences

- New build dependency `qt6-qtquick3d` in the runtime base (ADR 0021 split),
  and `Qt6::Quick3D` linked in the shell. First build after merge rebuilds the
  runtime base; the avatar window lands the build after (same deferred-base
  rollout as every runtime-base change).
- The emotion + mouth channels are plain Rust + bridge plumbing and **are**
  unit-testable on CI (emotion heuristic; WAV amplitude envelope). The 3D
  rendering is **not** verifiable on this dev host → flagged for on-device
  tuning, like Kvantum.
- `realistic` tier is aspirational on the dev VM; `basic`/fallback is the
  honest day-one target. We are not shipping a slideshow as "realista".
- A real `lilith.vrm` is an art deliverable tracked separately. Until then the
  procedural fallback is what users see.

## Alternatives rejected

- **Live2D / 2D rigged avatar** — lighter, runs great on software GL, polished
  VTuber look. Rejected because the explicit ask is a *3D body*; kept on file
  as the fallback if 3D proves unusable off-GPU.
- **Procedural 2D face only** — quickest, fully ours, lowest "wow". Folded in
  as the Quick3D *fallback* avatar rather than the destination.
- **Bake the model at build time (balsam → QML)** — locks the asset into the
  image and bloats it; `RuntimeLoader` from the data dir lets users (and the
  tier switch) swap models without a rebuild.
- **Phoneme/viseme lip-sync in v1** — more correct, but needs Piper phoneme
  timing capture; amplitude flap is convincing enough to ship first and the
  viseme path is a clean upgrade behind the same `mouthLevel`/morph seam.

## Phasing

1. **Runtime skeleton (this work):** Quick3D in build, floating companion
   window, procedural fallback, state channel wired, emotion + mouth channels
   end-to-end (Rust + bridges), VRM drop-in path.
2. Real VRM asset + viseme/expression morph mapping; Preferences tier switch.
3. Phoneme-timed lip-sync (Piper) replacing amplitude flap behind the same seam.
4. (Far) photoreal "realistic" tier tuned on GPU hardware; body/gestures.

## Related

- ADR 0006 (Qt6/C++ UI, Rust system) — the shell this extends.
- ADR 0009 (voice pipeline — Whisper + Piper) — source of the mouth channel.
- ADR 0021 (ISO build speed — two-base split) — where `qt6-qtquick3d` lands.
- `ai/lilith/module.md`, `system/voice/module.md`.
