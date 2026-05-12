#pragma once

#include <QAbstractListModel>
#include <QString>
#include <QVector>
#include <qqmlintegration.h>

/// Lists installed graphical applications by scanning the XDG application
/// directories for `.desktop` files.
///
/// Filtering rules (see https://specifications.freedesktop.org/desktop-entry-spec/):
///   - Type must be "Application" (drops Links, Directories)
///   - NoDisplay=true is skipped (entries deliberately hidden from menus)
///   - Hidden=true is skipped (entries marked as removed)
///   - %f / %F / %u / %U / %i / %c / %k tokens are stripped from Exec
///     (we run the command without an associated file/URL)
///
/// Phase 1: one-shot scan at construction. Phase 2 will watch the dirs for
/// changes and refresh incrementally.
class DesktopAppsModel : public QAbstractListModel
{
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QString filter READ filter WRITE setFilter NOTIFY filterChanged)
    Q_PROPERTY(int count READ count NOTIFY countChanged)

public:
    enum Roles {
        NameRole = Qt::UserRole + 1,
        CommentRole,
        ExecRole,
        IconRole,
        DesktopIdRole,
    };

    explicit DesktopAppsModel(QObject* parent = nullptr);

    int rowCount(const QModelIndex& parent = QModelIndex()) const override;
    QVariant data(const QModelIndex& index, int role = Qt::DisplayRole) const override;
    QHash<int, QByteArray> roleNames() const override;

    QString filter() const { return m_filter; }
    void setFilter(const QString& f);

    int count() const { return m_visible.size(); }

    /// Re-walk every application directory and rebuild the model.
    /// Called when the launcher opens so apps installed mid-session
    /// (e.g. a Flatpak Lilith just pulled) appear without needing a
    /// logout.
    Q_INVOKABLE void rescan();

signals:
    void filterChanged();
    void countChanged();

private:
    struct Entry {
        QString name;
        QString comment;
        QString exec;
        QString icon;
        QString desktopId;  // basename of the .desktop file without suffix
    };

    void scan();
    void rebuildVisible();
    static bool readEntry(const QString& path, Entry& out);
    static QString stripExecTokens(const QString& raw);

    QVector<Entry> m_all;
    QVector<int> m_visible;  // indices into m_all
    QString m_filter;
};
