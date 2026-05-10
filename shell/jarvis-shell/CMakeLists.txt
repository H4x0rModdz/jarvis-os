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

find_package(Qt6 6.4 REQUIRED COMPONENTS Core Quick Qml DBus QuickControls2)

qt_standard_project_setup(REQUIRES 6.4)

qt_add_executable(jarvis-shell
    src/main.cpp
    src/lilith_bridge.cpp
    src/lilith_bridge.h
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
        qml/Clock.qml
        qml/LilithInput.qml
        qml/StatusIndicator.qml
        qml/Theme.qml
)

target_link_libraries(jarvis-shell PRIVATE
    Qt6::Core
    Qt6::Quick
    Qt6::Qml
    Qt6::DBus
    Qt6::QuickControls2
)

# Useful in the Qt Creator + tooling story; harmless at command-line build.
set_target_properties(jarvis-shell PROPERTIES
    WIN32_EXECUTABLE OFF
    MACOSX_BUNDLE OFF
)
