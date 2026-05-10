# ADR 0001: Linux as the Base

## Status
Accepted

## Context

Jarvis OS needs a foundation to build upon. The options were:
1. Build a custom kernel from scratch
2. Use an existing Linux distribution as the base
3. Use another existing OS (FreeBSD, etc.)

## Decision

Use Linux as the base OS layer.

## Reasons

- Building a production-grade kernel requires solving memory management, hardware abstraction, drivers, process scheduling, filesystems, networking, graphics, USB, Bluetooth, GPU support, power management, and security — years of work
- The innovation in Jarvis OS is the AI-native UX, compatibility systems, and automation layer — not kernel scheduling
- Linux has mature hardware support, active development, and a thriving ecosystem
- The Linux ecosystem already provides Wayland, DBus, systemd, PipeWire, Vulkan — all needed components

## Consequences

- Jarvis OS is a Linux distribution at its base, not a fully independent OS
- Custom kernel work is deferred to Phase 4 (experimental research)
- We inherit Linux's hardware driver ecosystem (pro) and its fragmentation (managed via our own base choice)
- GPL/LGPL licensing must be respected throughout

## What This Is NOT

- Jarvis OS is not just a Linux skin
- The innovation layer (AI runtime, compositor, action bus, design system) sits entirely above the kernel
