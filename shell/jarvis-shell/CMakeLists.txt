cmake_minimum_required(VERSION 3.21)

project(jarvis-shell
    VERSION 0.1.0
    DESCRIPTION "Jarvis OS desktop shell (Phase 1a — toplevel window)"
    LANGUAGES CXX
)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)

# Enable QML compile-time tooling and warnings.
set(CMAKE_AUTOUIC OFF)

find_package(Qt6 6.5 REQUIRED COMPONENTS Core Quick Qml DBus QuickControls2)

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
    src/updater_bridge.cpp
    src/updater_bridge.h
    src/voice_bridge.cpp
    src/voice_bridge.h
    src/settings_bridge.cpp
    src/settings_bridge.h
)

# Qt's qmltype registration generator includes our headers by short name.
target_include_directories(jarvis-shell PRIVATE src)

# Theme is `pragma Singleton` — tell the QML toolchain so importing
# `Jarvis.Shell` exposes Theme as a singleton object.
set_source_files_properties(qml/Theme.qml PROPERTIES
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
)

target_link_libraries(jarvis-shell PRIVATE
    Qt6::Core
    Qt6::Quick
    Qt6::Qml
    Qt6::DBus
    Qt6::QuickControls2
)

if(LayerShellQt_FOUND)
    target_compile_definitions(jarvis-shell PRIVATE JARVIS_HAVE_LAYER_SHELL)
    target_link_libraries(jarvis-shell PRIVATE LayerShellQt::Interface)
endif()

# Useful in the Qt Creator + tooling story; harmless at command-line build.
set_target_properties(jarvis-shell PROPERTIES
    WIN32_EXECUTABLE OFF
    MACOSX_BUNDLE OFF
)
