# Naming Standards

## Philosophy

Names are the primary documentation. If you have to add a comment to explain a name, the name is wrong.

## Files

```
snake_case for all source files

GOOD:
  window_manager.rs
  voice_command_router.rs
  lilith_memory_store.rs
  app_install_service.rs

BAD:
  WindowManager.rs
  voiceCommandRouter.rs
  utils.rs
  helpers.rs
  misc.rs
  temp.rs
  new_version2_final.rs
```

## Directories

```
snake_case, named after their single responsibility

GOOD:
  voice_pipeline/
  action_bus/
  window_compositor/
  wine_runner/

BAD:
  utils/
  lib/
  common/
  helpers/
  misc/
  stuff/
```

## Rust / C++ Functions

```
GOOD:
  route_voice_command()
  install_flatpak_package()
  apply_window_blur()
  dispatch_action()
  revoke_permission_grant()

BAD:
  process()
  handle()
  run()
  do_thing()
  manage()
  execute()        ← too generic
```

## QML Components

```
PascalCase, prefixed with "Jarvis" for design system components

GOOD:
  JarvisGlassPanel
  JarvisTaskbar
  JarvisNotificationCard
  JarvisButton

BAD:
  panel1
  MyButton
  Component
  GlassThing
```

## DBus Names

```
Reverse domain, PascalCase for interfaces

GOOD:
  com.jarvis.ActionBus
  com.jarvis.Lilith
  com.jarvis.PermissionSystem

Method names: PascalCase
  Dispatch(action: string) -> string
  GrantPermission(scope: string, caller: string)

Signal names: PascalCase
  ActionCompleted(action_id: string, result: string)
```

## Constants

```
SCREAMING_SNAKE_CASE

GOOD:
  MAX_BLUR_RADIUS
  DEFAULT_ANIMATION_DURATION_MS
  PERMISSION_SCOPE_SEPARATOR

BAD:
  maxBlurRadius
  kMaxBlur
  MAX_BLUR
```

## Actions (Action Bus)

```
verb.noun or verb.noun.qualifier — all lowercase with dots

GOOD:
  app.open
  app.install
  file.delete
  window.minimize
  workspace.switch

BAD:
  openApp
  Open_App
  APP_OPEN
  doOpenApplication
```
