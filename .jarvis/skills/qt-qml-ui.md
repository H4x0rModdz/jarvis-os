# Qt/QML Expert Skill

## Goal

Write Qt/QML that is clean, performant, and maintainable — not "JavaScript schizophrenia glued to spatial XML".

## Principles

- Components are small, focused, and reusable
- State management is explicit and centralized where needed
- Animations are GPU-accelerated and declarative
- QML is UI only — business logic lives in C++/Rust backends
- Never mix UI and business logic in the same QML file

## Component Rules

```qml
// GOOD: focused, reusable, named semantically
JarvisGlassPanel {
    id: notificationPanel
    width: 320
    visible: notificationModel.hasUnread
}

// BAD: everything in one massive Item block
Item {
    // 500 lines of mixed state, animations, and logic
}
```

## State Management

- Use `StateGroup` or `State` for UI states, not boolean spaghetti
- Keep application state in C++/Rust `QObject` models, not QML properties
- Avoid deep property bindings chains — they cause silent performance regressions
- Use `Connections` explicitly rather than relying on implicit signal connections

## Animation Pipeline

- All animations must use `Behavior` or `Animation` elements — no JavaScript `setTimeout`
- Prefer `NumberAnimation` with `easing.type: Easing.OutCubic`
- Use `Animator` types (OpacityAnimator, ScaleAnimator) for GPU-thread animations
- Layer animations that run together in a `ParallelAnimation`

```qml
// GOOD
Behavior on opacity {
    NumberAnimation { duration: 200; easing.type: Easing.OutCubic }
}

// BAD
onVisibleChanged: {
    opacity = 0
    // some timer hack
}
```

## Performance Rules

- Enable `layer.enabled: true` only on frequently animated items
- Avoid `clip: true` unless necessary — it forces a new layer
- Never use `anchors` and `x/y` positioning on the same item
- Prefer `ListView` with delegate recycling over `Repeater` for large lists
- Use `Image.asynchronous: true` for all non-critical images

## Wayland Best Practices

- Never assume window decorations — Jarvis OS controls them via the compositor
- Use `Window.visibility` instead of platform-specific hacks
- Avoid `Qt.WindowFullScreen` direct calls — route through the window manager action

## File Structure

```
components/
  JarvisGlassPanel.qml
  JarvisButton.qml
  JarvisTaskbar.qml
screens/
  DesktopScreen.qml
  LockScreen.qml
models/
  NotificationModel.qml
```

## What Never Goes in QML

- Network requests
- File I/O
- AI inference calls
- Business logic decisions
- Permission checks
