#include <include/database/entities/Profile.h>

#include <QJsonDocument>
#include <QJsonObject>
#include <QSet>
#include <deque>
#include <unordered_map>
#include <vector>

#include "include/throne_fast_core.h"
#include "include/global/Configs.hpp"

namespace Configs
{
    Profile::Profile(Configs::outbound *outbound, const QString &type_)
    {
        if (!type_.isEmpty()) this->type = type_;


        if (outbound != nullptr) {
            this->outbound = std::shared_ptr<Configs::outbound>(outbound);
        }
    }

    void Profile::ClearTestResults() {
        test_country.clear();
        ip_out.clear();
        service_checks.clear();
        latency = 0;
        latency_at = 0;
        dl_speed.clear();
        ul_speed.clear();
    }

    void Profile::SetLatency(int ms) {
        latency = ms;
        // 0 means "never measured", so a reset must clear the stamp rather than record now.
        latency_at = ms == 0 ? 0 : QDateTime::currentSecsSinceEpoch();
    }

    QString Profile::DisplayLatency() const {
        if (latency == kLatencyConnectOnly) return QObject::tr("Connect OK");
        if (latency < 0) return QObject::tr("Unavailable");
        return latency > 0 ? QStringLiteral("%1 ms").arg(latency) : QString();
    }

    QString Profile::DisplayExitIP() const {
        QString result;
        if (!test_country.isEmpty()) result = CountryCodeToFlag(test_country);
        if (!ip_out.isEmpty()) {
            if (!result.isEmpty()) result += ' ';
            result += ip_out;
        }
        return result.isEmpty() ? result : UNICODE_LRO + result;
    }

    QString Profile::DisplaySpeed() const {
        QString result;
        if (!dl_speed.isEmpty()) result = QStringLiteral("↓") + dl_speed;
        if (!ul_speed.isEmpty()) {
            if (!result.isEmpty()) result += ' ';
            result += QStringLiteral("↑") + ul_speed;
        }
        return result;
    }

    QString Profile::DisplayTestResult() const {
        QString result = DisplayLatency();
        const auto append = [&result](const QString &value) {
            if (value.isEmpty()) return;
            if (!result.isEmpty()) result += ' ';
            result += value;
        };
        append(DisplayExitIP());
        append(DisplaySpeed());
        return result;
    }

    qint64 Profile::ServiceCheckedAt() const {
        if (service_checks.isEmpty()) return 0;
        return QJsonDocument::fromJson(service_checks.toUtf8()).object()["at"].toInteger();
    }

    QColor Profile::DisplayLatencyColor() const {
        if (latency == kLatencyConnectOnly) {
            return Qt::darkCyan;
        } else if (latency < 0) {
            return Qt::darkGray;
        } else if (latency > 0) {
            if (latency <= 100) {
                return Qt::darkGreen;
            } else if (latency <= 300)
            {
                return Qt::darkYellow;
            } else {
                return Qt::red;
            }
        } else {
            return {};
        }
    }

    QString Profile::DisplayTraffic() const {
        if (traffic_downlink + traffic_uplink == 0) return "";
        return UNICODE_LRO + QString("%1↑ %2↓").arg(ReadableSize(traffic_uplink), ReadableSize(traffic_downlink));
    }

    void Profile::ResetTraffic() {
        traffic_downlink = 0;
        traffic_uplink = 0;
    }

        QString ProfileFilter_ent_key(const std::shared_ptr<Configs::Profile> &ent, bool ignoreMetadata) {
        auto key = ent->outbound->ExportJsonLink(ignoreMetadata);
        return key;
    }

    QString ProfileFilter_ent_identity_key(const std::shared_ptr<Configs::Profile> &ent) {
        auto obj = ent->outbound->ExportIdentity();
        return ent->type + "|" + QString::fromUtf8(QJsonDocument(obj).toJson(QJsonDocument::Compact));
    }

    namespace {
        struct FastProfileKey {
            uint64_t hash;
            QString key;

            bool operator==(const FastProfileKey &other) const noexcept {
                return hash == other.hash && key == other.key;
            }
        };

        struct FastProfileKeyHash {
            size_t operator()(const FastProfileKey &k) const noexcept {
                return static_cast<size_t>(k.hash);
            }
        };

        inline FastProfileKey makeFastKey(QString keyStr) {
            const uint64_t h = throne_fast_xxhash64(
                reinterpret_cast<const uint8_t*>(keyStr.utf16()),
                static_cast<size_t>(keyStr.size()) * sizeof(char16_t),
                0);
            return FastProfileKey{h, std::move(keyStr)};
        }
    }

    void ProfileFilter::Uniq(const QList<std::shared_ptr<Profile>> &in,
                             QList<std::shared_ptr<Profile>> &out,
                             bool keep_last, bool ignoreMetadata) {
        std::unordered_map<FastProfileKey, size_t, FastProfileKeyHash> seen;
        seen.reserve(static_cast<size_t>(in.size()));
        std::vector<std::shared_ptr<Profile>> entries;
        entries.reserve(static_cast<size_t>(in.size()));

        for (const auto &ent : in) {
            FastProfileKey key = makeFastKey(ProfileFilter_ent_key(ent, ignoreMetadata));
            auto it = seen.find(key);
            if (it != seen.end()) {
                if (keep_last) {
                    entries[it->second].reset();
                    entries.push_back(ent);
                    it->second = entries.size() - 1;
                }
            } else {
                entries.push_back(ent);
                seen.emplace(std::move(key), entries.size() - 1);
            }
        }

        out.reserve(out.size() + static_cast<qsizetype>(entries.size()));
        for (auto &entry : entries) {
            if (entry) out.append(std::move(entry));
        }
    }

    void ProfileFilter::Common(const QList<std::shared_ptr<Profile>> &src,
                               const QList<std::shared_ptr<Profile>> &dst,
                               QList<std::shared_ptr<Profile>> &outSrc,
                               QList<std::shared_ptr<Profile>> &outDst,
                               bool ignoreMetadata) {
        std::unordered_map<FastProfileKey, std::deque<std::shared_ptr<Profile>>, FastProfileKeyHash> srcByKey;
        srcByKey.reserve(static_cast<size_t>(src.size()));

        for (const auto &ent : src) {
            srcByKey[makeFastKey(ProfileFilter_ent_key(ent, ignoreMetadata))].push_back(ent);
        }
        // A src entry can be claimed once: handing the same one to every dst duplicate collapses N identical servers into one profile (#1775).
        for (const auto &ent : dst) {
            auto it = srcByKey.find(makeFastKey(ProfileFilter_ent_key(ent, ignoreMetadata)));
            if (it == srcByKey.end() || it->second.empty()) continue;
            outDst += ent;
            outSrc += std::move(it->second.front());
            it->second.pop_front();
        }
    }

    void ProfileFilter::OnlyInSrc(const QList<std::shared_ptr<Profile>> &src,
                                  const QList<std::shared_ptr<Profile>> &dst,
                                  QList<std::shared_ptr<Profile>> &out,
                                  bool ignoreMetadata) {
        std::unordered_map<FastProfileKey, bool, FastProfileKeyHash> dstKeys;
        dstKeys.reserve(static_cast<size_t>(dst.size()));

        for (const auto &ent : dst) {
            dstKeys.emplace(makeFastKey(ProfileFilter_ent_key(ent, ignoreMetadata)), true);
        }
        for (const auto &ent : src) {
            if (dstKeys.find(makeFastKey(ProfileFilter_ent_key(ent, ignoreMetadata))) == dstKeys.end()) {
                out += ent;
            }
        }
    }

    void ProfileFilter::OnlyInSrc_ByPointer(const QList<std::shared_ptr<Profile>> &src,
                                            const QList<std::shared_ptr<Profile>> &dst,
                                            QList<std::shared_ptr<Profile>> &out) {
        QSet<const Profile *> dstSet;
        dstSet.reserve(dst.size());
        for (const auto &ent : dst) {
            dstSet.insert(ent.get());
        }
        for (const auto &ent : src) {
            if (!dstSet.contains(ent.get())) out += ent;
        }
    }

    void ProfileFilter::ChangedByIdentity(QList<std::shared_ptr<Profile>> &src,
                                          QList<std::shared_ptr<Profile>> &dst,
                                          QList<std::shared_ptr<Profile>> &changedSrc,
                                          QList<std::shared_ptr<Profile>> &changedDst) {
        std::unordered_map<FastProfileKey, std::deque<std::shared_ptr<Profile>>, FastProfileKeyHash> srcByKey;
        srcByKey.reserve(static_cast<size_t>(src.size()));
        for (const auto &ent : src) {
            srcByKey[makeFastKey(ProfileFilter_ent_identity_key(ent))].push_back(ent);
        }

        QSet<Profile *> matchedSrc;
        matchedSrc.reserve(qMin(src.size(), dst.size()));
        QList<std::shared_ptr<Profile>> remainingDst;
        for (const auto &ent : dst) {
            auto it = srcByKey.find(makeFastKey(ProfileFilter_ent_identity_key(ent)));
            if (it == srcByKey.end() || it->second.empty()) {
                remainingDst += ent;
                continue;
            }
            auto srcEnt = std::move(it->second.front());
            it->second.pop_front();
            changedSrc += srcEnt;
            changedDst += ent;
            matchedSrc.insert(srcEnt.get());
        }

        QList<std::shared_ptr<Profile>> remainingSrc;
        remainingSrc.reserve(src.size());
        for (const auto &ent : src) {
            if (!matchedSrc.contains(ent.get())) remainingSrc += ent;
        }
        src = std::move(remainingSrc);
        dst = std::move(remainingDst);
    }
}
