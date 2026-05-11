#pragma once

#include <QObject>
#include <QString>
#include <QDBusInterface>
#include <qqmlintegration.h>

/// Bridge between QML and `com.jarvis.Updater`.
///
/// Subscribes to `Progress` + `Completed` signals and exposes the current
/// state as Q_PROPERTYs so a splash window can bind to it. The splash is
/// "active" from the first Progress signal until Completed; after that the
/// splash fades out and the bar takes over.
class UpdaterBridge : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON
    Q_PROPERTY(bool active READ active NOTIFY stateChanged)
    Q_PROPERTY(QString stage READ stage NOTIFY stateChanged)
    Q_PROPERTY(int percent READ percent NOTIFY stateChanged)
    Q_PROPERTY(QString message READ message NOTIFY stateChanged)
    Q_PROPERTY(bool failed READ failed NOTIFY stateChanged)

public:
    explicit UpdaterBridge(QObject* parent = nullptr);

    bool active() const { return m_active; }
    QString stage() const { return m_stage; }
    /// [0, 100] when known; -1 = indeterminate (spinner).
    int percent() const { return m_percent; }
    QString message() const { return m_message; }
    /// True after a Completed(success=false). UI shows a retry button.
    bool failed() const { return m_failed; }

signals:
    void stateChanged();

private slots:
    void onProgress(const QString& stage, int percent, const QString& message);
    void onCompleted(bool success, const QString& message);

private:
    void setState(bool active, const QString& stage, int percent,
                  const QString& message, bool failed);

    QDBusInterface* m_iface = nullptr;
    bool m_active = false;
    bool m_failed = false;
    QString m_stage;
    int m_percent = -1;
    QString m_message;
};
