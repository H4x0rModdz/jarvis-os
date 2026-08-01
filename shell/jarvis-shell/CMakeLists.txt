cmake_minimum_required(VERSION 3.21)

project(jarvis-shell
    VERSION 0.1.0
    DESCRIPTION "LilithOS desktop shell (Phase 1a — toplevel window)"
    # C is needed too: the wlr-foreign-toplevel protocol's wl_interface
    # definitions are generated as a C source (see below).
    LANGUAGES C CXX
)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)

# Enable QML compile-time tooling and warnings.
set(CMAKE_AUTOUIC OFF)

find_package(Qt6 6.5 REQUIRED COMPONENTS Core Gui Quick Qml DBus QuickControls2 WaylandClient Quick3D)

# LayerShellQt is built out-of-tree against KDE's repo (see
# .jarvis-os/memory) and installed alongside Qt. The find_package call below
# resolves when CMAKE_PREFIX_PATH points at that Qt prefix; without it the
# shell still builds and runs as a regular toplevel (Phase 1a fallback).
find_package(LayerShellQt QUIET)
if(LayerShellQt_FOUND)
    message(STATUS "LayerShellQt found — building Phase 1b layer-shell variant")
else()
    message(STATUS "LayerShellQt NOT found — building Phase 1a toplevel variant")
endif()

qt_standard_project_setup(REQUIRES 6.5)

qt_add_executable(jarvis-shell
    src/main.cpp
    src/lilith_bridge.cpp
    src/lilith_bridge.h
    src/permission_bridge.cpp
    src/permission_bridge.h
    src/action_bus_bridge.cpp
    src/action_bus_bridge.h
    src/desktop_apps_model.cpp
    src/desktop_apps_model.h
    src/icon_image_provider.cpp
    src/icon_image_provider.h
    src/escape_closer.cpp
    src/escape_closer.h
    src/updater_bridge.cpp
    src/updater_bridge.h
    src/voice_bridge.cpp
    src/voice_bridge.h
    src/settings_bridge.cpp
    src/settings_bridge.h
    src/notifications_bridge.cpp
    src/notifications_bridge.h
    src/network_bridge.cpp
    src/network_bridge.h
    src/power_bridge.cpp
    src/power_bridge.h
    src/system_stats_bridge.cpp
    src/system_stats_bridge.h
    src/media_bridge.cpp
    src/media_bridge.h
    src/bluetooth_bridge.cpp
    src/bluetooth_bridge.h
    src/audio_bridge.cpp
    src/audio_bridge.h
    src/display_bridge.cpp
    src/display_bridge.h
    src/foreign_toplevel.cpp
    src/foreign_toplevel.h
    src/window_control_service.cpp
    src/window_control_service.h
)

# Qt's qmltype registration generator includes our headers by short name.
target_include_directories(jarvis-shell PRIVATE src)

# Generate the wlr-foreign-toplevel-management client bindings (qtwayland
# scanner) and compile them into the shell. This drives the dock's
# running-app awareness — see src/foreign_toplevel.* and ADR 0024.
# Generates the C++ wrappers AND a C protocol source carrying the
# wl_interface definitions. That C file only compiles because the project
# enables the C language above — without it CMake silently skips the file and
# the link fails with "undefined reference to *_interface".
qt6_generate_wayland_protocol_client_sources(jarvis-shell
    FILES ${CMAKE_CURRENT_SOURCE_DIR}/protocols/wlr-foreign-toplevel-management-unstable-v1.xml
)

# Theme and ShellBus are `pragma Singleton` — tell the QML toolchain so
# importing `Jarvis.Shell` exposes them as singleton objects. (ShellBus
# carries cross-surface UI intents between the top-bar / dock roots.)
set_source_files_properties(qml/Theme.qml qml/ShellBus.qml PROPERTIES
    QT_QML_SINGLETON_TYPE TRUE
)

qt_add_qml_module(jarvis-shell
    URI Jarvis.Shell
    VERSION 1.0
    QML_FILES
        qml/Main.qml
        qml/Bar.qml
        qml/BarMenuButton.qml
        qml/Clock.qml
        qml/LilithInput.qml
        qml/StatusIndicator.qml
        qml/Theme.qml
        qml/ApprovalDialog.qml
        qml/ApprovalButton.qml
        qml/Launcher.qml
        qml/AppCell.qml
        qml/UpdaterSplash.qml
        qml/MicButton.qml
        qml/SettingsPanel.qml
        qml/BarGearButton.qml
        qml/NotificationToast.qml
        qml/NotificationDrawer.qml
        qml/BarBellButton.qml
        qml/BarWifiButton.qml
        qml/BarBluetoothButton.qml
        qml/BarBatteryIndicator.qml
        qml/BluetoothPanel.qml
        qml/ConnectivityPanel.qml
        qml/DisplayPanel.qml
        qml/FirstBootWizard.qml
        qml/GlassPanel.qml
        qml/LilithPopup.qml
        qml/Desktop.qml
        qml/DesktopIcon.qml
        qml/HudGraph.qml
        qml/HudGauge.qml
        qml/ShellBus.qml
        qml/TopBar.qml
        qml/JarvisMenu.qml
        qml/AboutDialog.qml
        qml/Dock.qml
        qml/DockIcon.qml
        qml/LilithOrb.qml
        qml/LilithAvatar.qml
        qml/LilithAvatarView.qml
)

target_link_libraries(jarvis-shell PRIVATE
    Qt6::Core
    Qt6::Gui
    Qt6::Quick
    Qt6::Qml
    Qt6::DBus
    Qt6::QuickControls2
    Qt6::Quick3D
    Qt6::WaylandClient
)

# QWaylandClientExtension is exported from the public WaylandClient lib, but
# on some Qt builds its header sits behind the private module target. Link it
# only when the distro actually ships that target — otherwise the public
# component above is enough and a hard dependency would break the build.
if(TARGET Qt6::WaylandClientPrivate)
    target_link_libraries(jarvis-shell PRIVATE Qt6::WaylandClientPrivate)
endif()

if(LayerShellQt_FOUND)
    target_compile_definitions(jarvis-shell PRIVATE JARVIS_HAVE_LAYER_SHELL)
    target_link_libraries(jarvis-shell PRIVATE LayerShellQt::Interface)
endif()

# Useful in the Qt Creator + tooling story; harmless at command-line build.
set_target_properties(jarvis-shell PROPERTIES
    WIN32_EXECUTABLE OFF
    MACOSX_BUNDLE OFF
)
