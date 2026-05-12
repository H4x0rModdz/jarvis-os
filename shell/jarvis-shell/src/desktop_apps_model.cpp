#include "desktop_apps_model.h"

#include <QDir>
#include <QFile>
#include <QStandardPaths>
#include <QTextStream>
#include <QLoggingCategory>

namespace {
Q_LOGGING_CATEGORY(lcApps, "jarvis.shell.apps")
}

DesktopAppsModel::DesktopAppsModel(QObject* parent) : QAbstractListModel(parent)
{
    scan();
    rebuildVisible();
}

int DesktopAppsModel::rowCount(const QModelIndex& parent) const
{
    if (parent.isValid()) return 0;
    return m_visible.size();
}

QHash<int, QByteArray> DesktopAppsModel::roleNames() const
{
    return {
        { NameRole,      "name" },
        { CommentRole,   "comment" },
        { ExecRole,      "exec" },
        { IconRole,      "icon" },
        { DesktopIdRole, "desktopId" },
    };
}

QVariant DesktopAppsModel::data(const QModelIndex& index, int role) const
{
    if (!index.isValid() || index.row() < 0 || index.row() >= m_visible.size()) return {};
    const auto& e = m_all[m_visible[index.row()]];
    switch (role) {
        case NameRole:      return e.name;
        case CommentRole:   return e.comment;
        case ExecRole:      return e.exec;
        case IconRole:      return e.icon;
        case DesktopIdRole: return e.desktopId;
        default:            return {};
    }
}

void DesktopAppsModel::setFilter(const QString& f)
{
    if (m_filter == f) return;
    m_filter = f;
    emit filterChanged();
    beginResetModel();
    rebuildVisible();
    endResetModel();
    emit countChanged();
}

void DesktopAppsModel::rescan()
{
    beginResetModel();
    m_all.clear();
    m_visible.clear();
    scan();
    rebuildVisible();
    endResetModel();
    emit countChanged();
}

void DesktopAppsModel::scan()
{
    QStringList dirs = QStandardPaths::standardLocations(QStandardPaths::ApplicationsLocation);
    qCInfo(lcApps) << "Scanning" << dirs;

    for (const QString& dir : dirs) {
        QDir d(dir);
        const auto files = d.entryList({ QStringLiteral("*.desktop") }, QDir::Files);
        for (const QString& f : files) {
            Entry e;
            if (readEntry(d.filePath(f), e)) {
                e.desktopId = QFileInfo(f).completeBaseName();
                m_all.append(e);
            }
        }
    }

    // Stable alpha-sort by display name; case-insensitive.
    std::sort(m_all.begin(), m_all.end(), [](const Entry& a, const Entry& b) {
        return QString::compare(a.name, b.name, Qt::CaseInsensitive) < 0;
    });

    qCInfo(lcApps) << "Loaded" << m_all.size() << "applications";
}

void DesktopAppsModel::rebuildVisible()
{
    m_visible.clear();
    m_visible.reserve(m_all.size());
    const QString needle = m_filter.trimmed().toLower();
    for (int i = 0; i < m_all.size(); ++i) {
        if (needle.isEmpty() ||
            m_all[i].name.contains(needle, Qt::CaseInsensitive) ||
            m_all[i].comment.contains(needle, Qt::CaseInsensitive) ||
            m_all[i].desktopId.contains(needle, Qt::CaseInsensitive)) {
            m_visible.append(i);
        }
    }
}

bool DesktopAppsModel::readEntry(const QString& path, Entry& out)
{
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly | QIODevice::Text)) return false;

    QTextStream in(&f);
    QString section;
    bool noDisplay = false;
    bool hidden = false;
    QString type;

    while (!in.atEnd()) {
        const QString line = in.readLine().trimmed();
        if (line.isEmpty() || line.startsWith(QChar('#'))) continue;
        if (line.startsWith(QChar('[')) && line.endsWith(QChar(']'))) {
            section = line.mid(1, line.size() - 2);
            continue;
        }
        // We only care about the main [Desktop Entry] block; localized
        // variants like Name[pt_BR]=... and other groups (Desktop Action *)
        // are ignored for Phase 1.
        if (section != QStringLiteral("Desktop Entry")) continue;

        const int eq = line.indexOf(QChar('='));
        if (eq <= 0) continue;
        const QString key = line.left(eq).trimmed();
        const QString val = line.mid(eq + 1).trimmed();

        if (key == QStringLiteral("Name") && out.name.isEmpty())       out.name = val;
        else if (key == QStringLiteral("Comment") && out.comment.isEmpty()) out.comment = val;
        else if (key == QStringLiteral("Exec"))                        out.exec = stripExecTokens(val);
        else if (key == QStringLiteral("Icon"))                        out.icon = val;
        else if (key == QStringLiteral("NoDisplay"))                   noDisplay = (val.toLower() == QStringLiteral("true"));
        else if (key == QStringLiteral("Hidden"))                      hidden = (val.toLower() == QStringLiteral("true"));
        else if (key == QStringLiteral("Type"))                        type = val;
    }

    if (type != QStringLiteral("Application")) return false;
    if (noDisplay || hidden) return false;
    if (out.name.isEmpty() || out.exec.isEmpty()) return false;
    return true;
}

QString DesktopAppsModel::stripExecTokens(const QString& raw)
{
    // Remove %f %F %u %U %d %D %n %N %i %c %k tokens — they expand to a
    // file argument that we don't have when launching from the grid.
    QString s = raw;
    static const QStringList tokens = {
        QStringLiteral(" %f"), QStringLiteral(" %F"),
        QStringLiteral(" %u"), QStringLiteral(" %U"),
        QStringLiteral(" %d"), QStringLiteral(" %D"),
        QStringLiteral(" %n"), QStringLiteral(" %N"),
        QStringLiteral(" %i"), QStringLiteral(" %c"), QStringLiteral(" %k"),
    };
    for (const auto& t : tokens) s.remove(t);
    return s.trimmed();
}
