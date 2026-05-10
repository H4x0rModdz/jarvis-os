# UI Patterns

## Component Architecture

Every UI component must be:

- Self-contained: no side effects outside its declared interface
- Reusable: no hard-coded app-specific logic
- Named after what it visually represents
- Documented with its props/properties

## Design System Components (QML)

All design system components live in `shell/components/` and are prefixed `Jarvis`:

```
JarvisGlassPanel     ← base glass surface
JarvisButton         ← interactive button
JarvisIconButton     ← icon-only button
JarvisTextField      ← text input
JarvisCard           ← content card
JarvisAvatar         ← user/app avatar
JarvisBadge          ← notification badge
JarvisProgressRing   ← circular progress
JarvisTooltip        ← hover tooltip
JarvisContextMenu    ← right-click menu
```

Never create one-off styled elements in screen files. Extract to a component.

## Glass Panel Rules

```qml
JarvisGlassPanel {
    // Required
    blurRadius: 16           // 8-20 range only
    backgroundOpacity: 0.72  // 0.6-0.85 range only
    
    // Optional
    borderOpacity: 0.15      // subtle edge highlight
    shadowElevation: 2       // 0=none, 1=subtle, 2=standard, 3=raised
}
```

## Animation Patterns

### Window Open
```qml
NumberAnimation on scale { from: 0.96; to: 1.0; duration: 200; easing.type: Easing.OutCubic }
NumberAnimation on opacity { from: 0; to: 1; duration: 180; easing.type: Easing.OutCubic }
```

### Window Close
```qml
NumberAnimation on scale { from: 1.0; to: 0.96; duration: 160; easing.type: Easing.InCubic }
NumberAnimation on opacity { from: 1; to: 0; duration: 150; easing.type: Easing.InCubic }
```

### Panel Slide In (from bottom)
```qml
NumberAnimation on y { from: parent.height; to: 0; duration: 220; easing.type: Easing.OutCubic }
```

Never use `Easing.OutBounce` or `Easing.OutElastic` — these feel cheap and unprofessional.

## Spacing System

```
xs:  4px
sm:  8px
md:  12px
lg:  16px
xl:  24px
2xl: 32px
3xl: 48px
```

All padding and margins should use these values, never arbitrary pixel values.

## Typography Scale

```
caption:  11px
body:     13px
label:    13px (medium weight)
title:    16px
heading:  20px
display:  28px
```

## State Communication

Visual states must be communicated through more than just color:

| State | Color | Additional Indicator |
|---|---|---|
| Focused | Accent color border | Subtle glow |
| Disabled | 40% opacity | Cursor: not-allowed |
| Error | Red tint | Icon + tooltip |
| Loading | Neutral | Spinner or pulse |
| Success | Green flash | Checkmark icon |

## Responsive Behavior

- Taskbar: adapts to screen width, collapses app labels below 1280px
- Control center: fixed 360px width panel
- Notification panel: fixed 380px width
- Windows: minimum 320x240 enforced by window manager

## Accessibility Requirements

- All interactive elements reachable via Tab key
- Focus indicator: 2px solid accent color, always visible
- Touch targets: minimum 44x44px
- Never remove outline styling with `outline: none` without a visible replacement
