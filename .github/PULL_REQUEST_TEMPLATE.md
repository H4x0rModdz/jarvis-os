<!--
Thanks for contributing to Jarvis OS! Fill this in so review is fast.
Read CONTRIBUTING.md first if you haven't.
-->

## What & why

<!-- What does this change do, and what was broken / missing that made it
     necessary? Explain the *why* — the diff already shows the *what*. -->

## How I tested it

<!-- Commands run, manual steps, VM/hardware. "cargo test -p <crate> passes"
     and/or "booted the image and confirmed X". Untested = say so. -->

## Checklist

- [ ] `cargo fmt --all -- --check` is clean
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` is clean
- [ ] Tests for touched crates pass (`cargo test -p <crate>`)
- [ ] New module? It has a `module.md`
- [ ] New AI-triggerable action? It goes through the Action Bus with explicit permissions
- [ ] Touched behaviour a `module.md` documents? Updated that `module.md`
- [ ] Significant/hard-to-reverse change? Added an ADR in `.jarvis/decisions/`
- [ ] No file named `utils` / `helpers` / `misc` / `common` / `temp`

## Related

<!-- Link the issue or ADR this implements, e.g. "Closes #42", "Implements ADR 0024". -->
