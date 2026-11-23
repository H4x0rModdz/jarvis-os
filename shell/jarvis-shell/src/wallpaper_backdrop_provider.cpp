#include "wallpaper_backdrop_provider.h"

#include <QColor>
#include <QFile>
#include <QImage>
#include <QImageReader>
#include <QStandardPaths>

namespace {
QString wallpaperPath()
{
    const QString dev = qEnvironmentVariable("JARVIS_WALLPAPER");
    if (!dev.isEmpty() && QFile::exists(dev)) return dev;

    const QString user =
        QStandardPaths::writableLocation(QStandardPaths::GenericDataLocation)
        + QStringLiteral("/jarvis/wallpaper");
    if (QFile::exists(user)) return user;

    return QStringLiteral("/usr/share/backgrounds/jarvis/wallpaper.jpg");
}

/// Push colours away from grey. Blurring averages a photo towards mud; real
/// frosted glass looks *more* saturated than what is behind it, not less.
void saturate(QImage& img, float amount)
{
    for (int y = 0; y < img.height(); ++y) {
        auto* line = reinterpret_cast<QRgb*>(img.scanLine(y));
        for (int x = 0; x < img.width(); ++x) {
            float h, s, l, a;
            QColor::fromRgb(line[x]).getHslF(&h, &s, &l, &a);
            line[x] = QColor::fromHslF(h, qMin(1.0f, s * amount), l, a).rgb();
        }
    }
}
} // namespace

WallpaperBackdropProvider::WallpaperBackdropProvider()
    : QQuickImageProvider(QQuickImageProvider::Image)
{
}

QImage WallpaperBackdropProvider::requestImage(const QString&, QSize* size, const QSize&)
{
    // Decode straight to a small size (JPEG can do this while decoding, so a
    // 4K wallpaper never has to be held in memory at full resolution).
    QImageReader reader(wallpaperPath());
    reader.setAutoTransform(true);
    const QSize full = reader.size();
    if (full.isValid() && full.width() > 0) {
        reader.setScaledSize(full.scaled(384, 384, Qt::KeepAspectRatio));
    }
    QImage img = reader.read();
    if (img.isNull()) {
        // No wallpaper readable: a neutral dark sheet, so glass still reads
        // as a surface instead of going transparent.
        img = QImage(8, 8, QImage::Format_RGB32);
        img.fill(QColor(28, 28, 32));
        if (size) *size = img.size();
        return img;
    }
    img = img.convertToFormat(QImage::Format_RGB32);

    // Blur = shrink far below the detail scale, then let smooth upscaling
    // spread each texel back out. Two hops through a 24 px-wide image leave
    // only the large colour masses; the GPU's bilinear stretch to screen
    // size finishes the job. No convolution, a few milliseconds, once.
    const QSize tiny = img.size().scaled(24, 24, Qt::KeepAspectRatio);
    img = img.scaled(tiny, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);
    saturate(img, 1.6f);
    img = img.scaled(tiny * 8, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);

    if (size) *size = img.size();
    return img;
}
