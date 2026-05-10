# Jarvis Philosophy

## The Core Beliefs

This skill exists to prevent the project from drifting into complexity theater, over-engineering, and mission creep.

When in doubt, return to these.

## What Jarvis OS Is

- An AI-native desktop operating system
- Linux-based, open source, desktop-first
- A platform where humans and AI cooperate naturally
- Compatibility-oriented (Windows apps should feel native)
- Built for real people doing real work

## What Jarvis OS Is NOT

- A Windows clone
- Another Linux skin
- A telemetry-driven surveillance platform
- Enterprise architecture theater in a desktop shell
- A research project that never ships
- An excuse to implement every cool idea

## The Hierarchy of Values

When values conflict, resolve them in this order:

1. **User safety & control** — always first
2. **Usability** — does it actually work for real people?
3. **Performance** — does it feel fast and responsive?
4. **Readability** — can engineers understand it?
5. **Elegance** — is it well-designed?
6. **Cleverness** — last resort, often a trap

## Engineering Beliefs

- Simplicity > technical exhibitionism
- Clarity > excessive abstraction
- Fluidity > gimmicks
- AI as collaboration, not replacement
- Transparency always
- Open source first
- Performance matters
- UX matters
- Legibility matters

## Design Beliefs

- The interface should feel alive without being distracting
- Beauty and function are not in conflict
- Animations exist to communicate, not to show off
- A user working 8 hours a day must not feel fatigued by our UI
- Accessibility is not optional

## AI Beliefs

- AI is a native system component, not a bolt-on
- AI must be transparent about what it's doing and why
- AI must never act without appropriate user awareness
- AI collaboration amplifies users — it does not replace them
- Local-first AI wherever possible

## When You're Tempted to Add Complexity

Ask:

1. Does this solve a real, existing problem?
2. Does this make things simpler for users or engineers?
3. Can this be explained in 30 seconds?
4. Would removing this make anything worse?

If you can't answer all four clearly — don't add it.

## The "Would Linus Read This" Test

Not about code style.
About clarity.

If a senior engineer reading the code would ask "why does this exist?" — it needs either a clear reason or a deletion.

## Anti-Patterns This Philosophy Prevents

- "Let's add 14 enterprise patterns and 83 abstract interfaces to open a window"
- "We need a ManagerFactoryProviderAdapterStrategy for this button"
- "Let's add telemetry — anonymized of course" (no)
- "Let's support 7 plugin systems before we have one working feature"
- "Let's over-document everything to prove we're serious engineers"
- "Let's use microservices for the desktop compositor"
