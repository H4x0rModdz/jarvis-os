cmake_minimum_required(VERSION 3.21)

project(jarvis-greeter
    VERSION 0.1.0
    DESCRIPTION "Jarvis OS login greeter (greetd front-end)"
    LANGUAGES CXX
)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC OFF)

find_package(Qt6 6.5 REQUIRED COMPONENTS Core Quick Qml Network QuickControls2)

qt_standard_project_setup(REQUIRES 6.5)

qt_add_executable(jarvis-greeter
    src/main.cpp
    src/greetd_client.cpp
    src/greetd_client.h
)

target_include_directories(jarvis-greeter PRIVATE src)

# Greeter theme tokens are duplicated locally — the greeter runs as
# the `greeter` user *before* the session is up, so it can't import
# `Jarvis.Shell` (different process, different runtime). The values
# match shell/jarvis-shell/qml/Theme.qml so the two screens look
# continuous when login transitions into the desktop.
set_source_files_properties(qml/Theme.qml PROPERTIES
    QT_QML_SINGLETON_TYPE TRUE
)

qt_add_qml_module(jarvis-greeter
    URI Jarvis.Greeter
    VERSION 1.0
    QML_FILES
        qml/Main.qml
        qml/LoginScreen.qml
        qml/Theme.qml
        qml/StandardMode.qml
        qml/LilithMode.qml
        qml/FocusMode.qml
        qml/components/GlassCard.qml
        qml/components/JarvisLogo.qml
        qml/components/ModeIndicator.qml
        qml/components/AuthIcon.qml
        qml/components/PasswordField.qml
        qml/components/UnlockButton.qml
        qml/components/VoiceWaveform.qml
        qml/components/SuggestionChip.qml
)

target_link_libraries(jarvis-greeter PRIVATE
    Qt6::Core
    Qt6::Quick
    Qt6::Qml
    Qt6::Network
    Qt6::QuickControls2
)

set_target_properties(jarvis-greeter PROPERTIES
    WIN32_EXECUTABLE OFF
    MACOSX_BUNDLE OFF
)
