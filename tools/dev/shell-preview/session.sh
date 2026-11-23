#!/bin/sh
# Runs INSIDE the preview container (see ../shell-preview.sh). Builds the
# shell from /src into /build, then starts labwc with the shell in it.
#   MODE=shot  headless output, one screenshot to /out/$NAME.png, then exit
#   MODE=live  labwc as a window on the host's Wayland (WSLg), until closed
set -u
: "${MODE:=shot}" "${NAME:=shell}" "${RES:=2560x1080}" "${SETTLE:=6}"

if [ ! -f /build/CMakeCache.txt ]; then
    cmake -S /src/shell/jarvis-shell -B /build -DCMAKE_BUILD_TYPE=Release > /out/configure.log 2>&1
fi
if ! cmake --build /build -j"$(nproc)" > /out/build.log 2>&1; then
    echo "BUILD FAILED — see .preview/build.log"
    grep -n "error" /out/build.log | head -30
    exit 1
fi

export XDG_RUNTIME_DIR=/tmp/xdg
mkdir -p "$XDG_RUNTIME_DIR" /tmp/labwc-cfg
chmod 700 "$XDG_RUNTIME_DIR" 2>/dev/null || true
# The session's real window rules + titlebar layout, but not its autostart.
cp /src/iso/assets/labwc/rc.xml /tmp/labwc-cfg/rc.xml
mkdir -p /usr/share/themes/LilithOS
cp -r /src/iso/assets/labwc/theme /usr/share/themes/LilithOS/labwc

export WLR_RENDERER=pixman LIBGL_ALWAYS_SOFTWARE=1 WLR_LIBINPUT_NO_DEVICES=1
if [ "$MODE" = live ]; then
    # The host compositor, mounted under a name labwc will not try to claim
    # for its own socket.
    export WLR_BACKENDS=wayland WAYLAND_DISPLAY=host-wayland
    OUTPUT=WL-1
else
    export WLR_BACKENDS=headless
    OUTPUT=HEADLESS-1
fi
export MODE NAME RES SETTLE OUTPUT

cat > /tmp/inner.sh <<'INNER'
#!/bin/sh
sleep 1
wlr-randr --output "$OUTPUT" --custom-mode "$RES" 2>> /out/session.log
sleep 1
swaybg -m fill -i /src/home-bg.jpg >/dev/null 2>&1 &
# No daemons run here, so skip the first-boot wizard that would otherwise
# cover the desktop waiting for a Settings service that never answers.
export QT_QPA_PLATFORM=wayland JARVIS_SKIP_FIRST_BOOT=1 JARVIS_WALLPAPER=/src/home-bg.jpg
export JARVIS_ICON_THEME="${JARVIS_ICON_THEME:-Papirus-Dark}" QT_FORCE_STDERR_LOGGING=1
/build/jarvis-shell > /out/shell.log 2>&1 &
SHELL_PID=$!
# A couple of ordinary windows so the bar/dock are seen in context.
if [ "${PREVIEW_APPS:-1}" = 1 ]; then
    sleep 2
    foot -T "Terminal" -W 96x26 sh -c 'printf "LilithOS preview\n"; exec sh' >/dev/null 2>&1 &
fi
if [ "$MODE" = shot ]; then
    sleep "$SETTLE"
    [ -n "${PREVIEW_HOOK:-}" ] && sh -c "$PREVIEW_HOOK" >> /out/session.log 2>&1
    grim "/out/$NAME.png" 2>> /out/session.log
    kill $SHELL_PID 2>/dev/null
    sleep 1
    pkill labwc
else
    wait $SHELL_PID
    pkill labwc
fi
INNER
chmod +x /tmp/inner.sh

dbus-run-session -- labwc -C /tmp/labwc-cfg -s /tmp/inner.sh > /out/labwc.log 2>&1
if [ -s /out/shell.log ]; then
    echo "--- shell log (QML warnings/errors):"
    head -40 /out/shell.log
fi
[ "$MODE" = shot ] && ls -la "/out/$NAME.png" 2>/dev/null
exit 0
