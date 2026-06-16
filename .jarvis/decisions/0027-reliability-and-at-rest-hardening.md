# 0027 — Reliability + at-rest data hardening (Arc 3)

Status: accepted
Date: 2026-06-02

## Context

Moving from "impressive demo" to "daily driver" means the desktop must not
fall apart when a piece fails, must stay usable when the AI model is down,
and must treat personal data sensibly. Four items, two open problems
(P002, P003).

## Decision

1. **Service resilience — already in place, now the standard.** All nine
   `jarvis-*` user units ship `Restart=on-failure` + `RestartSec=2`, so a
   crashed daemon (shell, Lilith, action-bus, …) comes back on its own.
   Every new daemon unit must keep this. No change needed; recorded so it
   isn't regressed.

2. **Graceful AI degradation (resolves P002).** When Ollama is unreachable
   or returns an error, Lilith now replies with an honest *"modelo de IA
   offline — comandos diretos ainda funcionam"* message instead of the
   misleading *"não entendi o comando"* (which blamed the user's phrasing).
   The rule-based intent parser already runs before any LLM call, so direct
   commands ("abrir firefox", "minimizar", "tira um print") keep working
   fully offline. Graceful degradation is a UX contract, not just an error
   code.

3. **At-rest data = LUKS, not app-level encryption (resolves P003).** The
   at-rest boundary is **full-disk encryption (LUKS) at install**, NOT
   app-level SQLCipher on the SQLite stores. On an autologin single-user
   box the encryption key would have to live locally (no user secret to
   derive it from), so app-level crypto is security theatre while adding
   real key-management complexity. Instead, as defense-in-depth, the Lilith
   stores (`facts.db`, `lilith.db`) are chmod'd `0600` and their parent dir
   `0700` on creation (best-effort, Unix-only). Enabling LUKS in the
   installer is tracked with the installer work, not here.

4. **Default password stays a documented dev convenience (deferred).**
   `jarvis/jarvis` + SSH password auth are kept because `tools/dev-deploy.sh`
   relies on them. On an autologin box the password only gates sudo / SSH /
   lock, and LUKS (item 3) is the real at-rest protection. **A production /
   release build MUST replace this** with a runtime-prompted or hashed
   password and key-only SSH. Flagged loudly so it isn't shipped by
   accident.

## Consequences

- Crashed daemons self-heal; the AI being down degrades to direct-command
  mode with an honest message; conversation/fact DBs are owner-only.
- P002 and P003 move to resolved (update `contexts/active-problems.md` in
  the docs pass).
- The known-weak default password is an accepted, documented risk for dev
  images only — a release checklist item, not a silent default.
- Verification: items 2 + 3 are covered by `ci.yml` (Rust build + tests,
  incl. the Ollama-offline test). Item 1 is observable on a VM.

## Alternatives rejected

- **App-level SQLCipher / encrypted memory** — adds a dependency + key
  management for little real protection on an autologin device; LUKS covers
  the actual threat (powered-off disk theft).
- **Force-expire the default password now** — would break the dev-deploy
  SSH loop; the dev-vs-prod split is the right place to fix it.
