#pragma once

#include <QObject>
#include <QString>
#include <QDBusInterface>
#include <qqmlintegration.h>

/// Direct Action Bus client for UI surfaces that already know which action
/// they want to invoke (the app launcher clicking an entry, future control
/// panels, etc).  Lilith remains the right path for natural-language input,
/// but going through the LLM/regex pipeline to dispatch a tool the user
/// just clicked would be wasteful.
class ActionBusBridge : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON

public:
    explicit ActionBusBridge(QObject* parent = nullptr);

    /// Dispatch an action by name with a JSON-string of parameters.
    /// Returns synchronously via the `dispatchFinished` signal.
    Q_INVOKABLE void dispatch(const QString& action, const QString& paramsJson);

signals:
    void dispatchFinished(const QString& action,
                          const QString& replyJson,
                          bool success);
    void errorOccurred(const QString& message);

private:
    QDBusInterface* m_iface = nullptr;
};
