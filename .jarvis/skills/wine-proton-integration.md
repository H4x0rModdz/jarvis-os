# Wine/Proton Integration

## Goal

Make Windows application compatibility seamless and AI-manageable within LilithOS.

## Stack

- **Wine** — base Windows compatibility layer
- **Proton** — Valve's Wine fork with gaming optimizations
- **DXVK** — DirectX 9/10/11 → Vulkan translation
- **VKD3D-Proton** — DirectX 12 → Vulkan translation
- **Steam Runtime** — container for Proton dependencies

## Prefix Architecture

Each app category gets its own Wine prefix:

```
~/.jarvis/compat/
  prefixes/
    gaming/          ← Steam/Proton games
    productivity/    ← Office, creative tools
    dev-tools/       ← Windows SDKs, IDE tools
    isolated/<hash>/ ← Per-app sandboxed prefixes
```

Never mix gaming and productivity prefixes — DLL conflicts are hard to debug.

## Runner Pipeline

When installing a Windows app:

1. Detect app type (game, productivity, dev tool)
2. Select appropriate runner (Wine stable, Wine GE, Proton, Proton GE)
3. Check DXVK/VKD3D requirements from app metadata
4. Create or reuse appropriate prefix
5. Run installer in sandboxed environment
6. Register app in Jarvis app catalog with compatibility metadata

## AI Integration

Lilith can assist with:

- Detecting the right runner for an app
- Diagnosing Wine errors from logs
- Suggesting missing DLL overrides
- Applying community-sourced compatibility fixes
- Monitoring app performance and suggesting DXVK config tweaks

Lilith must not:
- Auto-install runners without user approval
- Auto-modify system Wine installation
- Bypass sandbox boundaries to "fix" app issues

## DXVK Configuration

```ini
# Per-app dxvk.conf in prefix
d3d11.cachedDynamicResources = vid
d3d11.maxFrameLatency = 1
dxvk.enableAsync = true
```

Async shader compilation must be on by default.
Cache directory: `~/.jarvis/cache/dxvk/<app_hash>/`

## Compatibility Metadata Format

Each registered Windows app stores:

```json
{
  "app_id": "uuid",
  "name": "App Name",
  "exe": "App.exe",
  "runner": "proton-ge-9",
  "prefix": "gaming",
  "dxvk": true,
  "vkd3d": false,
  "install_date": "ISO8601",
  "compatibility_notes": "Needs d3dx9 override",
  "sandbox_level": "standard"
}
```

## Sandboxing

- Wine prefixes run with reduced filesystem access
- No access to `/home` outside the prefix and explicit allowed paths
- Network access is allowed but monitored
- Anti-cheat kernel drivers are explicitly blocked from loading

## Limitations to Communicate to Users

Always be honest about:

- Kernel-level anti-cheat (EAC, BattlEye kernel mode) — not compatible
- Hardware DRM (Denuvo-style kernel components) — may not work
- Apps requiring Windows Update or activation servers — limited support
- Legacy 16-bit applications — not supported
