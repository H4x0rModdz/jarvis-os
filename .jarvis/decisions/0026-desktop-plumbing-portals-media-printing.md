# 0026 — Desktop plumbing: portals, removable media, printing

Status: accepted
Date: 2026-06-02

## Context

The shell, daemons and AI loops are rich, but three pieces of unglamorous
desktop plumbing every daily driver needs were missing from the image — and
each one fails *silently*, which is worse than an error:

- **No `xdg-desktop-portal`.** Flatpak apps (Firefox, etc.) have no portal,
  so browser **screen-share / video calls** don't work and native file
  dialogs degrade.
- **No `udisks2`.** Plugging in a USB stick or external disk does nothing —
  it never mounts or appears.
- **No CUPS.** No printing at all.

## Decision

Bake the standard plumbing into the image (no novel mechanism — pick the
backends that fit a wlroots/labwc session):

- **Portals:** `xdg-desktop-portal` + `xdg-desktop-portal-wlr` (ScreenCast /
  Screenshot — labwc speaks `wlr-screencopy`) + `xdg-desktop-portal-gtk`
  (FileChooser, Settings, … — the wlr backend implements neither). Routing
  lives in `/usr/share/xdg-desktop-portal/labwc-portals.conf`, keyed to
  `XDG_CURRENT_DESKTOP=labwc` (set in `labwc/environment`): ScreenCast +
  Screenshot → `wlr`, `default` → `gtk`. The **PipeWire** stack
  (`pipewire`, `wireplumber`, `pipewire-pulseaudio`) is what ScreenCast
  streams over; enabled `--global` as user units.
- **Removable media:** `udisks2` (the mount engine — Dolphin mounts through
  it) + `udiskie --no-tray` from the labwc autostart for automount +
  insert notifications. No tray exists in this session, so udiskie runs
  headless for the behaviour only.
- **Printing:** `cups` + `cups-filters` + `cups-pk-helper` (polkit) +
  `system-config-printer` (add-a-printer GUI). `avahi` so driverless
  (IPP Everywhere) network printers are discovered — CUPS' dnssd backend
  queries Avahi directly, so `nss-mdns` / nsswitch edits aren't needed.
  `cups.socket` is socket-activated (no idle daemon).

## Consequences

- Screen-share/video calls, USB drives and printing work out of the box.
- `udisks2` and `xdg-desktop-portal` are D-Bus activated (no enable);
  `cups.socket` + `avahi-daemon` are enabled; PipeWire user units are
  enabled `--global` but guarded (`|| true`) because exact unit names vary
  by release and the base may already preset them.
- Verification is the **ISO build / a VM** — none of this is exercised by
  the Rust CI (`ci.yml`); only `build-iso.yml` compiles the image.
- Image grows (~portals + CUPS + PipeWire + system-config-printer and its
  GTK deps). Acceptable for a usable desktop.

## Alternatives rejected

- **`xdg-desktop-portal-gnome`/`-kde`** — pull a heavy half-DE; the
  wlr + gtk pair is the minimal combo that covers a wlroots session.
- **Auto-mount via a custom udisks2 caller** — udiskie is the standard,
  maintained tool; writing our own adds risk for no gain.
- **Skip Avahi, rely on manually-added printers** — driverless discovery is
  most of the "it just works" value for printing today.
