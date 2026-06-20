# Contributing to LilithOS

Thanks for wanting to build the AI-native desktop with us. This guide is the
front door: it tells you how to get a change merged. The *rules of the road*
(naming, architecture, design language) live in [`.jarvis/`](./.jarvis/) and
this document points you at the right file instead of repeating it.

> **Read first:** [`.jarvis/jarvis-core-context.md`](./.jarvis/jarvis-core-context.md)
> — what LilithOS is and is not. A PR that fights the project's identity
> won't merge no matter how clean the code is.

---

## TL;DR

1. Fork (or branch, if you have write access) — **never commit to `main`**.
2. Branch name: `feature/<short-name>`, `fix/<short-name>`, or `adr/<NNNN>-<slug>`.
3. Make the change. Every new module gets a `module.md`. Every new action
   goes through the Action Bus.
4. `cargo fmt --all` + `cargo clippy --workspace --all-targets -- -D warnings`
   must be clean. Tests for touched crates must pass.
5. Open a PR against `main`. Fill in the template. CI must be green.
6. A maintainer reviews and merges.

---

## Ground rules (non-negotiable)

These come straight from [`CLAUDE.md`](./CLAUDE.md) and the skills in
[`.jarvis/skills/`](./.jarvis/skills/). They are enforced in review:

1. **Every new module gets a `module.md`.** No exceptions. Template:
   [`.jarvis/standards/module-contracts.md`](./.jarvis/standards/module-contracts.md).
2. **No file named `utils`, `helpers`, `misc`, `common`, or `temp`.** Name
   things for what they do. See
   [`.jarvis/standards/naming.md`](./.jarvis/standards/naming.md).
3. **All AI-triggerable actions go through the Action Bus.** No daemon is
   called directly to bypass permission gating + audit. See
   [`.jarvis/architecture/action-bus.md`](./.jarvis/architecture/action-bus.md).
4. **No dangerous action (delete, `terminal.execute`, …) without user
   confirmation.** See [`.jarvis/skills/ai-safety.md`](./.jarvis/skills/ai-safety.md).
5. **Every abstraction justifies its existence in one sentence.** If you
   can't explain a module in two minutes, it's too complex. See
   [`.jarvis/skills/anti-bullshit-engineering.md`](./.jarvis/skills/anti-bullshit-engineering.md).
6. **UI follows the design language:** animations ≤ 250 ms ease-out;
   glassmorphism blur 8–20 px, opacity 0.6–0.85, never on text-heavy
   surfaces. See [`.jarvis/skills/jarvis-design-language.md`](./.jarvis/skills/jarvis-design-language.md).

If a PR violates one of these, the fix is to change the PR — not the rule.
Rules change through an ADR (see below), not a one-off exception.

---

## Workflow

### 1. Branch

`main` is always releasable and **protected** — it builds and publishes the
OS image to ghcr on every push (the OTA channel; see
[ADR 0023](./.jarvis/decisions/0023-ota-updates-via-ghcr.md)). Nobody commits
to it directly, maintainers included. Work on a branch:

```
feature/<short-name>     a new capability
fix/<short-name>         a bug fix
adr/<NNNN>-<slug>        an architecture decision (see below)
```

### 2. Make the change

- Match the surrounding code — its naming, comment density, and idioms.
- Keep the change focused. One concern per PR; a fix and a refactor are two PRs.
- New module? Create the directory (`snake_case`), write its `module.md`,
  register any new actions in the Action Bus schema with explicit required
  permissions, and add it to
  [`.jarvis/standards/folder-structure.md`](./.jarvis/standards/folder-structure.md).
- Touching behaviour that a `module.md` documents? Update the `module.md` in
  the same PR. Docs that lie are worse than no docs.

### 3. Run the checks locally

The same checks CI runs (`.github/workflows/ci.yml`). Run them before you push
— a red PR is a slow PR:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo build --workspace
cargo test -p <the-crate-you-touched>     # or --workspace
```

Shell / greeter / lock are Qt and don't build in CI's Ubuntu Rust job — they
build in the image pipeline. If you touch them, build locally (Qt 6.5+):

```bash
cmake -S shell/jarvis-shell -B /tmp/shell-build -G Ninja -DCMAKE_BUILD_TYPE=Release
cmake --build /tmp/shell-build -j
```

### 4. Open the PR

- Target `main`.
- Title in the same style as the commit (see below).
- Fill in the PR template: what changed, why, how you tested it, and which
  `module.md` / ADR you updated.
- Link the issue or ADR it implements.

### 5. Review + merge

A maintainer reviews. CI must be green and at least one approving review is
required before merge. Maintainers squash-merge so `main` history stays one
commit per logical change.

---

## Commits

- **One logical change per commit.** Prefer a new commit over amending a
  pushed one.
- **Conventional-style prefix:** `feat(scope): …`, `fix(scope): …`,
  `docs: …`, `ci: …`, `refactor(scope): …`, `style: …`. The scope is the
  module (`fix(updater): …`, `feat(wm): …`).
- **Body explains *why*, not just *what*.** The diff already shows what
  changed; the message should say what was broken and why this fixes it.
- **Never** `--no-verify`, `--no-gpg-sign`, or force-push shared branches
  unless explicitly agreed. If a hook fails, fix the cause.

---

## Architecture decisions (ADRs)

Significant or hard-to-reverse changes — a new daemon, a new IPC boundary, a
dependency that's hard to drop, anything that changes a contract — need an
**ADR before implementation**. We do **not** use a separate RFC process; the
ADR *is* the proposal and the record.

1. Copy the format of an existing entry in
   [`.jarvis/decisions/`](./.jarvis/decisions/) (e.g.
   [`0023-ota-updates-via-ghcr.md`](./.jarvis/decisions/0023-ota-updates-via-ghcr.md)).
2. Number it next in sequence: `.jarvis/decisions/<NNNN>-<short-title>.md`.
3. Sections: **Context · Decision · Consequences · Alternatives rejected.**
4. Open it as its own `adr/<NNNN>-<slug>` PR (or as the first commit of the
   feature PR) so the decision is reviewed before the code.

Check the existing ADRs before re-deciding something — many questions are
already answered there.

---

## What to work on

- **Current goals:** [`.jarvis/contexts/current-goals.md`](./.jarvis/contexts/current-goals.md)
- **Open problems:** [`.jarvis/contexts/active-problems.md`](./.jarvis/contexts/active-problems.md)
- **Out of scope (don't):** [`.jarvis/contexts/known-limitations.md`](./.jarvis/contexts/known-limitations.md)

Opening an issue before a large PR saves everyone time — describe the change
and let a maintainer sanity-check the direction before you build it.

---

## Reporting bugs

Open an issue with: what you did, what you expected, what happened, and the
environment (VM or hardware, `bootc status` output if it's an OS-level
issue). Logs live under `~/.jarvis/logs/` and `/tmp/labwc-autostart.log` —
attach the relevant tail.

---

## License

The project license is still TBD (see [`README.md`](./README.md)). By
contributing you agree your contributions will be released under whatever
license the project adopts. Until then: look, learn, fork, don't ship as
your own product.
