#include "greeter_state.h"

namespace {
constexpr const char* kKeyUsername = "username";
constexpr const char* kKeyMode = "modeIndex";
constexpr const char* kDefaultUsername = "jarvis";
}

GreeterState::GreeterState(QObject* parent)
    : QObject(parent)
    , m_settings(QSettings::IniFormat, QSettings::UserScope,
                 QStringLiteral("Jarvis"),
                 QStringLiteral("jarvis-greeter"))
{
    m_username = m_settings.value(kKeyUsername,
                                  QStringLiteral(kDefaultUsername)).toString();
    m_modeIndex = m_settings.value(kKeyMode, 0).toInt();
    // Clamp — the mode count is 3 in V1; if the file came from a future
    // build with more modes we shouldn't crash, just fall back.
    if (m_modeIndex < 0 || m_modeIndex > 2) m_modeIndex = 0;
}

void GreeterState::setUsername(const QString& v)
{
    if (v == m_username) return;
    m_username = v;
    emit changed();
}

void GreeterState::setModeIndex(int v)
{
    if (v == m_modeIndex) return;
    m_modeIndex = v;
    emit changed();
}

void GreeterState::persist()
{
    m_settings.setValue(kKeyUsername, m_username);
    m_settings.setValue(kKeyMode, m_modeIndex);
    m_settings.sync();
}
