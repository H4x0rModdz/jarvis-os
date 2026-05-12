cmake_minimum_required(VERSION 3.21)

project(jarvis-lock-window
    VERSION 0.1.0
    DESCRIPTION "Jarvis OS session lock — Qt overlay window driven by jarvis-lock daemon"
    LANGUAGES CXX
)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC OFF)

find_package(Qt6 6.5 REQUIRED COMPONENTS Core Quick Qml DBus)
find_package(LayerShellQt QUIET)
if(LayerShellQt_FOUND)
    message(STATUS "LayerShellQt found — lock window will use Overlay layer + Exclusive keyboard")
else()
    message(WARNING
        "LayerShellQt NOT found — lock window will fall back to a regular floating "
        "toplevel which a user can Alt+Tab away from. Ship a layer-shell-capable "
        "Qt build for any production use.")
endif()

qt_standard_project_setup(REQUIRES 6.5)

qt_add_executable(jarvis-lock-window
    src/main.cpp
    src/lock_client.cpp
    src/lock_client.h
)

target_include_directories(jarvis-lock-window PRIVATE src)

set_source_files_properties(qml/Theme.qml PROPERTIES
    QT_QML_SINGLETON_TYPE TRUE
)

qt_add_qml_module(jarvis-lock-window
    URI Jarvis.Lock
    VERSION 1.0
    QML_FILES
        qml/Main.qml
        qml/Theme.qml
)

target_link_libraries(jarvis-lock-window PRIVATE
    Qt6::Core
    Qt6::Quick
    Qt6::Qml
    Qt6::DBus
)

if(LayerShellQt_FOUND)
    target_compile_definitions(jarvis-lock-window PRIVATE JARVIS_HAVE_LAYER_SHELL)
    target_link_libraries(jarvis-lock-window PRIVATE LayerShellQt::Interface)
endif()

# Share the branding assets with the greeter so the lock screen visually
# matches login. Same `qrc:/branding/` prefix the greeter uses.
qt_add_resources(jarvis-lock-window "branding"
    PREFIX "/branding"
    BASE "${CMAKE_SOURCE_DIR}/../.."
    FILES
        "${CMAKE_SOURCE_DIR}/../../jarvis-os-default-icon.png"
        "${CMAKE_SOURCE_DIR}/../../jarvis-op-default-wallpaper.png"
)

set_target_properties(jarvis-lock-window PROPERTIES
    WIN32_EXECUTABLE OFF
    MACOSX_BUNDLE OFF
)
