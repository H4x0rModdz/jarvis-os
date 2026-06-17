#include "icon_image_provider.h"

#include <QIcon>
#include <QPixmap>

IconImageProvider::IconImageProvider() : QQuickImageProvider(QQuickImageProvider::Pixmap) {}

QPixmap IconImageProvider::requestPixmap(const QString& id, QSize* size, const QSize& requestedSize)
{
    // QML passes the requested render size; fall back to a crisp 64px when
    // the Image hasn't been laid out yet (sourceSize unset).
    const int dim = qMax(qMax(requestedSize.width(), requestedSize.height()), 64);

    QIcon icon = QIcon::fromTheme(id);

    // Last-resort generic glyph so at least *something* recognisable shows
    // for an app whose own icon the theme lacks. Still themeable — it's a
    // standard freedesktop name.
    if (icon.isNull()) {
        icon = QIcon::fromTheme(QStringLiteral("application-x-executable"));
    }

    // Genuinely nothing to draw → null pixmap. The QML Image goes to
    // Image.Error and its monogram fallback takes over.
    if (icon.isNull()) {
        if (size) *size = QSize(dim, dim);
        return {};
    }

    QPixmap pm = icon.pixmap(QSize(dim, dim));
    if (size) *size = pm.isNull() ? QSize(dim, dim) : pm.size();
    return pm;
}
