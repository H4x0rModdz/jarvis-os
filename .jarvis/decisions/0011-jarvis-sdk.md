# ADR 0011: Jarvis SDK — Manifest + DBus Proxy

## Status
Accepted

## Context

Jarvis OS's central promise — "AI is a native OS component, not a
bolt-on" — only fully lands when third-party apps can plug into the
same Action Bus that Lilith and the shell use. Without an SDK, every
new capability requires editing the Jarvis monorepo: any external app
is invisible to Lilith.

The other Linux DEs that ship an "AI assistant" treat third-party apps
as black boxes the assistant can only launch, not control. We've kept
the Action Bus shape open from day one specifically so that doesn't
have to be the case here.

We need to define **how a third-party app declares its actions to the
OS** and **how the Action Bus routes calls into the app**.

## Decision

The Jarvis SDK V1 is two pieces:

1. **Manifest file** — TOML at
   `/usr/share/jarvis/apps/<id>/manifest.toml` (system-installed) or
   `~/.local/share/jarvis/apps/<id>/manifest.toml` (per-user). The
   manifest lists the app's id, metadata, and the actions it exposes
   with JSON Schemas for each.

2. **DBus service** — the app hosts an interface named
   `com.jarvis.app.<id>` with a single method `Dispatch(action: string,
   params_json: string) -> string` returning a JSON envelope. The
   Action Bus proxies every SDK action call through this method.

At Action Bus startup, every manifest under the well-known paths is
parsed and each `[[actions]]` entry registers a generic proxy handler
in the registry. From Lilith's point of view (and any other Action Bus
client) the SDK action is indistinguishable from a built-in — same
`ListActions()`, same `Dispatch()`, same permission gating.

## Reasons

- **Reuses the existing surface.** Lilith picks up new actions for free
  because she queries the Action Bus's catalog. No tools.rs churn per
  third-party app.
- **No dynamic loading.** Plugins via `dlopen` couple apps to our ABI
  and crash blast radius. DBus subprocess isolation gives us crash
  containment and language independence — the app can be in any
  language with DBus bindings, not just Rust.
- **DBus activation works.** Apps that ship a `.service` activation
  file in `/usr/share/dbus-1/services/` get auto-started when the
  Action Bus calls them — the app doesn't have to be running for its
  actions to be available, matching how the rest of the desktop works.
- **TOML manifest, not JSON.** Same reason the rest of the Rust
  ecosystem picked it: comments + better human ergonomics. The
  per-action schema inside the TOML is still standard JSON Schema so
  Lilith forwards it verbatim to Ollama as a tool description.
- **Action namespace rule.** Every action an SDK app declares must be
  prefixed with the app's id (e.g. an app with `id = "notes"` can
  declare `notes.create` but not `mail.send`). The Action Bus rejects
  conflicting registrations at scan time. Prevents a malicious or
  buggy app from impersonating built-in actions like `filesystem.delete`.

## Consequences

- New workspace member `sdk/jarvis-sdk-types` — pure-data crate with
  the Manifest / Action structs and a `load_manifests()` scanner. Used
  by the Action Bus on startup.
- The Action Bus's `build_registry()` gains a second pass that calls
  `load_manifests()` and registers one generic proxy handler per
  declared action.
- New example crate `examples/sdk-hello/` that ships a manifest +
  a tiny DBus service implementing two demo actions. The example is
  baked into the ISO under `/usr/share/jarvis/apps/sdk_hello/` so the
  user can immediately call `sdk_hello.echo` from Lilith and confirm
  the wiring works.
- Permission scopes still apply: SDK apps' actions get classified
  through the same policy in `system/permission/src/policy.rs`.
  Unknown scopes deny by default, so a new app's action prompts for
  approval the first time Lilith calls it.

## V1 vs V2

| Item | V1 (this) | V2 |
|---|---|---|
| Manifest discovery | system + user `~/.local` dirs | + flatpak / OCI app introspection |
| Action invocation | DBus method per action | + `ActionInvocationStream` for actions with streamed progress |
| AI hints | none | `[[actions.ai]]` block — natural-language triggers, examples |
| Validation | schema-shape only | + permission scope declaration in manifest |
| Hot-reload | requires Action Bus restart | watch the well-known dirs + auto-reload |

## Alternatives Considered

- **Dynamic plugin loader (`dlopen` Rust crates).** Rejected: ABI
  coupling, crash blast radius, single-language.
- **HTTP / Unix-socket per app.** Rejected: redundant with DBus,
  which we already use everywhere and which has activation + name
  ownership built-in.
- **Bake everything into Lilith's tool list manually.** Rejected: it's
  exactly the monorepo coupling we're trying to escape.
- **Single shared `com.jarvis.SDK` service apps register against.**
  Rejected: that service becomes a single point of failure, and DBus
  doesn't natively support that pattern. Per-app names cost nothing.
