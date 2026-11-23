# Daily-driver plan

> How LilithOS gets from "impressive demo" to "the OS on my work machine".
> Written 2026-11-23. Supersedes the ordering (not the content) of the
> wishlist in `known-limitations.md` § Daily-driver feature backlog.

## The bar

**Done means:** one person uses LilithOS as their only OS for two working
weeks — browser, editor, terminal, calls with screen share, documents, music,
suspend/resume, an external monitor — and never has to open a terminal to
repair the desktop, and never thinks "this looks unfinished".

Everything below is ordered by that bar, not by how interesting it is to build.
A phase is finished when its exit test passes on real hardware, not when its
PRs are merged.

## Where we are (evidence, not impressions)

Works: the AI loop end to end (typed, voice, hotword, tool chains), OTA with
rollback, Flatpak apps, lock screen, notifications, Wi-Fi/Bluetooth/audio
panels, GPU rendering on NVIDIA.

Checked in the image on 2026-11-23 and **missing**:

| Gap | Why it blocks daily use |
|---|---|
| ~~The ISO installer wiped every connected disk (unattended `clearpart --all`)~~ — fixed 2026-11-23, guarded in CI; needs a VM install test with two disks | Data loss on the first real install |
| Shell is started once from the labwc autostart, nothing restarts it | A shell crash leaves no bar and no dock until re-login |
| `rc.xml` defines `<keyboard>` without `<default />` | labwc then drops its built-in bindings: no Alt+Tab, no Super+arrow snapping |
| No keyboard-layout setting anywhere (XKB defaults = US) | A pt-BR ABNT2 keyboard types the wrong characters |
| No polkit authentication agent | Any app asking for admin rights fails silently |
| No keyring / Secret Service | Browsers, editors and git cannot store credentials |
| No system tray (StatusNotifier host) | Chat, VPN, sync and Steam clients are unreachable once minimised |
| Xwayland is installed but the autostart documents "no working DISPLAY" | X11-only apps (Steam, many Wine apps, some Electron apps) do not start |
| Dev default login `jarvis/jarvis` + sshd password auth enabled in the image | Unacceptable on a machine holding real data |
| No software centre | Installing an app means asking Lilith or using a terminal |
| No display-layout persistence, night light, power profiles, clipboard history | Small, daily papercuts on a laptop + external monitor |
| No dev-container tool (distrobox/toolbox) | On an immutable OS that is how you install a toolchain |
| Window geometry / snapping / workspaces actions are stubs | Deferred to "the Smithay compositor" (P001) since Phase 4 |
| No real backdrop blur | The whole design language depends on it; labwc cannot do it |

## Phase 0 — Foundation (in flight)

1. **Shell performance** — done locally (branch `perf/shell-hitches`): idle
   shell 43% → 0% of a core. To ship, then measure on the RTX 3060 (P005).
2. **Reproducible image, Secure Boot-capable NVIDIA** — ADR 0033, done
   locally; the NVIDIA layer is build-verified, not boot-verified.
3. **Compositor** — decided for now (ADR 0034, § Compositor below).

**Exit test:** the image boots on the real machine with Secure Boot on, the
desktop is GPU-rendered, and dragging a window is smooth.

## Phase 1 — Usable for a working day

Ordered so that each step removes a reason to reach for another OS.

### 1a. Things that are simply broken (days)

- ~~Shell restarts when it crashes~~ — done (`jarvis-shell-supervise`, a loop in the compositor's environment rather than a systemd unit; see the script for why).
- ~~`<default />` in `rc.xml`~~ — done; verify Alt+Tab, Super+arrows on the machine.
- Keyboard layout: ~~applied from the installer's choice to labwc and the greeter~~ — done; still to do: changing it in Preferences.
- ~~Polkit agent~~ — done with `lxqt-policykit` (reused, not written); restyle later.
- Keyring: `gnome-keyring` (Secret Service) unlocked at login via PAM.
- Xwayland: find out why it is "not working", fix, and test Steam + one Wine
  app + one X11-only Electron app.
- Fonts: Inter now ships; still to add explicitly: a monospace, Noto
  (coverage + emoji).

Also done in 1a (found on the first real-hardware boot, 2026-11-23):
**Wi-Fi could not work at all** — the image had `NetworkManager` but not
`NetworkManager-wifi`, `wpa_supplicant` or Intel firmware; Bluetooth's service
was never enabled; NVIDIA helper units failed loudly on boot when the module
was not loaded; touchpads had no tap-to-click; no edge snapping. All fixed in
the image; all still to be confirmed on the machine.

### 1b. Things every desktop has (1–2 weeks)

- **System tray** in the top bar (StatusNotifierItem host).
- **Window management that exists:** snapping/tiling zones, workspaces, an
  overview, and the `window.*` / `workspace.*` actions made real — through
  the chosen compositor rather than waiting for a custom one.
- **Displays:** arrangement + scale + refresh in `DisplayPanel`, persisted and
  re-applied on hotplug (kanshi or the compositor's own config).
- **Software centre:** a Flathub front end. Reuse one (Bazaar / Discover as a
  Flatpak) before writing our own.
- **Default apps for work:** office suite, mail/calendar, PDF, image, video
  already ship or are one Flatpak away — pin a default set and wire MIME types.
- **Screenshots and recording** bound to keys, with a region selector;
  **screen sharing** verified in Firefox and a Chromium/Electron app.
- **Laptop basics:** power profiles, night light, clipboard history, lid and
  battery behaviour, suspend/resume verified on the new driver (ADR 0033).
- **Dev environment:** distrobox + a documented "how to install a toolchain".

### 1c. Safe to hold real data (1 week)

- Release images ship **no default credentials and no sshd password auth**; the
  dev conveniences move to a `-dev` image tag.
- First-boot: create the user and password (ADR 0029 Phase A is the base),
  offer MOK enrolment when Secure Boot is on, LUKS at install (ADR 0030).
- Automatic Flatpak updates; OTA already exists — surface "what changed".

**Exit test (the real one):** two working weeks on it as the only OS.

## Phase 2 — Design: one coherent look

### Direction

The target is the design language of **macOS 26 "Tahoe" (Liquid Glass)**:
a fully transparent menu bar; a floating glass dock; panels and menus as
translucent material that blurs and tints what is behind it; larger, softer
corner radii; floating toolbars and sidebars; glass and tinted icon styles;
motion that is quick and physical.

### What we can and cannot take

- **Can:** the design *language* — layout, material behaviour, proportions,
  radii, spacing, motion. A look is not protected; plenty of desktops ship one
  like it.
- **Cannot:** Apple's *assets* — the SF Pro / SF Mono fonts (their licence
  limits them to mock-ups of software for Apple platforms), Apple's icons,
  wallpapers, sounds, cursors, the Apple logo and names. Shipping those in an
  OS image is redistribution without a licence, and it is the one way this
  project gets a takedown.
- **So:** independently drawn equivalents that already exist and are GPL —
  vinceliuice's `MacTahoe-gtk-theme` and `MacTahoe-icon-theme` (successors of
  the WhiteSur themes we already ship) — plus **Inter** as the UI typeface and
  our own wallpapers.

### Status

First slice done (ADR 0035), visible with `tools/dev/shell-preview.sh live`:
tokens, the glass component with a blurred-wallpaper backdrop, the
transparent menu bar, the glass dock, the system menu, a Spotlight-style
launcher, correct placement of every popup and dialog, traffic-light window
decorations, and the HUD turned into an opt-in mode. Items 1–3 below are
done; 4–6 are what remains.

### Work

1. **ADR: design language v2** — replaces the "Windows 11 + macOS + KDE" blend
   in `skills/jarvis-design-language.md` with one reference, and states the
   asset rule above.
2. **Tokens first.** `Theme.qml` becomes the single source: materials (ultra
   thin → thick), radii scale, spacing scale, type scale, motion curves.
   Nothing in QML uses a literal colour, radius or duration afterwards.
3. **Real glass.** Backdrop blur behind the bar, dock, menus and panels —
   provided by the compositor (§ Compositor), with a tint + specular edge in
   `GlassPanel`. A "reduce transparency" setting falls back to solid surfaces.
4. **Rebuild the chrome on the tokens:** transparent top bar with proper
   menus; dock with magnification, running dots, drag-to-reorder; Control
   Centre (Wi-Fi, Bluetooth, sound, brightness, focus, media) replacing the
   scattered panels; Notification Centre; Spotlight-style launcher that also
   is Lilith's text entry; Settings as sidebar + panes.
5. **Window decorations** to match (radius, traffic lights, shadow), and GTK/
   Qt app theming (MacTahoe GTK + Kvantum) so apps do not look foreign.
6. **Greeter, lock screen, first-boot wizard, installer branding** on the same
   tokens.
7. **The eDEX HUD** (ADR 0031) does not belong to this language. Keep it as an
   opt-in "command centre" wallpaper mode; the default desktop is a wallpaper.

**Exit test:** someone who has never seen it cannot tell which parts were built
first. No surface uses a colour, radius or animation outside the tokens.

## Phase 3 — What makes it LilithOS rather than a themed Linux

- Lilith on the GPU (CUDA is in the image now) and a better default model per
  hardware tier.
- Lilith can do everything Phase 1 added (snap windows, switch workspace,
  change display, install from the software centre) — each is an Action Bus
  action, so this is mostly free.
- The embodied avatar (ADR 0028) and the remote companion, in that order.

## Compositor

**Hyprland, with labwc as the automatic fallback** — ADR 0036 (2026-11-23),
superseding ADR 0034's shortlist. Implemented but never run: the dev machine
has no GPU. First thing to do with the next image is the checklist at the
end of ADR 0036.

- Hyprland comes from a one-maintainer COPR (not in Fedora); its config is
  Lua since 0.57. Both are standing risks.
- The custom Smithay compositor is research, not the path to features.
- On labwc (VMs, fallback) glass is the blurred wallpaper (ADR 0035); on
  Hyprland the compositor blurs the real content.

## Rules for this plan

- A phase's exit test is run on the real machine. "It builds" is not evidence.
- New daily-driver features are Action Bus actions first, UI second.
- Reuse a maintained component before writing one (tray protocol libraries,
  a software centre, kanshi, a keyring) — the custom part of this OS is Lilith
  and the shell, not the plumbing.
