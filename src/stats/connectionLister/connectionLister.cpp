#include <QThread>
#include <QDateTime>
#include <QObject>
#include <core/gen/libcore.pb.h>
#include <include/api/RPC.h>
#include "include/database/DatabaseManager.h"
#include "include/global/Utils.hpp"
#include "include/stats/StatsSink.hpp"
#include <include/stats/connections/connectionLister.hpp>
#include "include/stats/traffic/TrafficStatsManager.hpp"
#include "include/global/LocalNetwork.hpp"

#include <algorithm>
#include <unordered_map>



namespace Stats
{
    // Guards out-of-band ForceUpdate() polls: a tiny byte delta over a tiny interval reads as a spike.
    static constexpr qint64 kSpeedSampleMinMs = 500;

    // Off-view polls still run: they keep the per-app traffic stats sampled.
    static constexpr unsigned long kActivePollMs = 1000;
    static constexpr unsigned long kRelaxedPollMs = 5000;

    ConnectionLister* connection_lister = new ConnectionLister();

    void ConnectionLister::ForceUpdate()
    {
        QMutexLocker wlk(&waitMu_);
        forced_ = true;
        waitCond_.wakeAll();
    }


    void ConnectionLister::Loop()
    {
        while (true)
        {
            if (stop) return;

            bool forced = false;
            {
                QMutexLocker wlk(&waitMu_);
                if (!forced_) waitCond_.wait(&waitMu_, inView_.load() || apiWatch_.load() ? kActivePollMs : kRelaxedPollMs);
                forced = forced_;
                forced_ = false;
            }

            if (stop) return;
            // A forced poll is a user action on the table, so it runs even while stats are off.
            if (!forced && (suspend || !Configs::dataManager->settingsRepo->enable_stats)) continue;

            mu.lock();
            update(forced || inView_.load(), apiWatch_.load());
            mu.unlock();
        }
    }

    void ConnectionLister::SetInView(bool inView)
    {
        const bool was = inView_.exchange(inView);
        if (inView && !was)
        {
            QMutexLocker wlk(&waitMu_);
            waitCond_.wakeAll();
        }
    }

    void ConnectionLister::SetApiWatch(bool watch)
    {
        const bool was = apiWatch_.exchange(watch);
        if (watch && !was)
        {
            QMutexLocker wlk(&waitMu_);
            waitCond_.wakeAll();
        }
    }

    void ConnectionLister::SetApiSink(ApiSink sink)
    {
        apiSink_ = std::move(sink);
    }

    QString EndpointHost(const QString& endpoint)
    {
        if (endpoint.startsWith('['))
        {
            const auto close = endpoint.indexOf(']');
            return close > 0 ? endpoint.mid(1, close - 1) : endpoint.mid(1);
        }
        const auto colon = endpoint.lastIndexOf(':');
        return colon < 0 ? endpoint : endpoint.left(colon);
    }

    void ConnectionLister::SetOutboundNames(const QHash<QString, QString>& names)
    {
        QMutexLocker lk(&namesMu_);
        outboundNames_ = names;
    }

    namespace
    {
        static const QString s_netTcp = QStringLiteral("tcp");
        static const QString s_netUdp = QStringLiteral("udp");
        static const QString s_protoHttp = QStringLiteral("http");
        static const QString s_protoTls = QStringLiteral("tls");
        static const QString s_protoQuic = QStringLiteral("quic");
        static const QString s_protoDns = QStringLiteral("dns");
        static const QString s_unknown = QStringLiteral("Unknown");

        inline QString internNetwork(const std::string& net)
        {
            if (net == "tcp") return s_netTcp;
            if (net == "udp") return s_netUdp;
            if (net.empty()) return QString();
            return QString::fromStdString(net);
        }

        inline QString internProtocol(const std::string& proto)
        {
            if (proto.empty()) return QString();
            if (proto == "http") return s_protoHttp;
            if (proto == "tls") return s_protoTls;
            if (proto == "quic") return s_protoQuic;
            if (proto == "dns") return s_protoDns;
            return QString::fromStdString(proto);
        }

        class StringCache
        {
        public:
            QString getOrInsert(const std::string& str)
            {
                if (str.empty()) return QString();
                auto it = cache_.find(str);
                if (it != cache_.end()) return it->second;
                if (cache_.size() >= 1024) cache_.clear();
                QString qstr = QString::fromStdString(str);
                cache_.emplace(str, qstr);
                return qstr;
            }
        private:
            std::unordered_map<std::string, QString> cache_;
        };
    }

    static ConnectionMetadata metaFromProto(const libcore::ConnectionMetaData& conn, const QString& localLabel,
                                            const QHash<QString, QString>& outboundNames,
                                            StringCache& cache,
                                            QHash<QString, bool>& ownAddressCache)
    {
        ConnectionMetadata c;
        c.id = QString::fromStdString(conn.id.value());
        c.createdAtMs = conn.created_at.value();
        c.dest = QString::fromStdString(conn.dest.value());
        c.upload = conn.upload.value();
        c.download = conn.download.value();
        c.domain = QString::fromStdString(conn.domain.value());
        c.network = internNetwork(conn.network.value());
        c.outbound = cache.getOrInsert(conn.outbound.value());
        c.outboundName = outboundNames.value(c.outbound);
        if (c.outboundName.isEmpty()) c.outboundName = c.outbound;
        c.process = cache.getOrInsert(conn.process.value());
        c.processPath = cache.getOrInsert(conn.process_path.value());
        c.protocol = internProtocol(conn.protocol.value());
        c.closedAtMs = conn.closed_at.value();
        c.rule = cache.getOrInsert(conn.rule.value());
        c.ruleItem = cache.getOrInsert(conn.rule_item.value());
        c.source = QString::fromStdString(conn.source.value());
        // In tun mode our own traffic enters with the tun's (or this machine's LAN) address, so a loopback-only test would label it a LAN client.
        const QString host = EndpointHost(c.source);
        if (host.isEmpty())
        {
            c.sourceLocal = true;
        }
        else
        {
            auto it = ownAddressCache.constFind(host);
            if (it != ownAddressCache.constEnd())
            {
                c.sourceLocal = it.value();
            }
            else
            {
                const bool isOwn = LocalNetwork::IsOwnAddress(host);
                ownAddressCache.insert(host, isOwn);
                c.sourceLocal = isOwn;
            }
        }
        c.sourceDisplay = c.sourceLocal ? localLabel : host;
        return c;
    }

    namespace
    {
        // The core iterates a map, so orderings must be total or equal keys reshuffle every poll.
        template <typename Key>
        void sortLargestFirst(QList<ConnectionMetadata>& list, bool asc, Key key)
        {
            std::sort(list.begin(), list.end(), [&](const ConnectionMetadata& a, const ConnectionMetadata& b)
            {
                const auto& ka = key(a);
                const auto& kb = key(b);
                if (ka == kb) return asc ? a.id > b.id : a.id < b.id;
                return asc ? ka < kb : ka > kb;
            });
        }

        // Text columns read the other way round: unflipped means A→Z, not Z→A.
        template <typename Key>
        void sortSmallestFirst(QList<ConnectionMetadata>& list, bool asc, Key key)
        {
            std::sort(list.begin(), list.end(), [&](const ConnectionMetadata& a, const ConnectionMetadata& b)
            {
                const auto& ka = key(a);
                const auto& kb = key(b);
                if (ka == kb) return asc ? a.id > b.id : a.id < b.id;
                return asc ? ka > kb : ka < kb;
            });
        }

        void sortApiSnapshot(QList<ConnectionMetadata>& list)
        {
            std::sort(list.begin(), list.end(), [](const ConnectionMetadata& a, const ConnectionMetadata& b)
            {
                if ((a.closedAtMs == 0) != (b.closedAtMs == 0)) return a.closedAtMs == 0;
                if (a.createdAtMs != b.createdAtMs) return a.createdAtMs > b.createdAtMs;
                return a.id < b.id;
            });
        }
    }

    bool SortIsDescending(const ConnectionSort sort, const bool ascending)
    {
        switch (sort)
        {
        case Default:
            return false;
        case ByProcess:
        case ByOutbound:
        case ByProtocol:
        case BySource:
            return ascending;
        default:
            return !ascending;
        }
    }

    void ConnectionLister::update(const bool pushToUi, const bool pushToApi)
    {
        libcore::QueryConnectionsResp resp = API::defaultClient->QueryConnections();
        const qint64 nowMs = QDateTime::currentMSecsSinceEpoch();
        const QString localLabel = QObject::tr("Local");
        QHash<QString, QString> outboundNames;
        {
            QMutexLocker lk(&namesMu_);
            outboundNames = outboundNames_;
        }

        StringCache stringCache;
        QHash<QString, bool> ownAddressCache;

        QList<ConnectionMetadata> rows;
        rows.reserve(static_cast<qsizetype>(resp.active.size()));
        QHash<QString, SpeedSample> newSamples;
        newSamples.reserve(static_cast<qsizetype>(resp.active.size()));

        for (const auto& conn : resp.active)
        {
            auto c = metaFromProto(conn, localLabel, outboundNames, stringCache, ownAddressCache);

            SpeedSample s;
            if (const auto it = speedSamples_.constFind(c.id); it != speedSamples_.constEnd())
            {
                const qint64 dt = nowMs - it->sampledAtMs;
                if (dt >= kSpeedSampleMinMs)
                {
                    qint64 dUp = c.upload - it->upload;
                    qint64 dDown = c.download - it->download;
                    if (dUp < 0) dUp = 0; // counters only grow; guard against any reset
                    if (dDown < 0) dDown = 0;
                    s.upload = c.upload;
                    s.download = c.download;
                    s.sampledAtMs = nowMs;
                    s.upSpeed = dUp * 1000 / dt;
                    s.downSpeed = dDown * 1000 / dt;
                }
                else
                {
                    s = *it; // window too short: keep last rate and baseline
                }
            }
            else
            {
                s.upload = c.upload; // first sighting: seed baseline, no rate yet
                s.download = c.download;
                s.sampledAtMs = nowMs;
            }
            newSamples.insert(c.id, s);
            c.uploadSpeed = s.upSpeed;
            c.downloadSpeed = s.downSpeed;

            rows.append(std::move(c));
        }
        speedSamples_ = newSamples; // drop ids for connections that have closed

        // Credits the live set plus the recently-closed ring (deduped by id), so a connection that opened and closed between polls still counts.
        if (!Configs::dataManager->settingsRepo->disable_traffic_stats)
        {
            QHash<QString, QPair<qint64, qint64>> newLast;
            newLast.reserve(rows.size());
            QSet<QString> currentClosed;
            currentClosed.reserve(static_cast<qsizetype>(resp.closed.size()));

            auto credit = [&](const QString& id, const QString& proc, const QString& procPath, qint64 curUp, qint64 curDown)
            {
                qint64 baseUp = 0, baseDown = 0;
                if (const auto it = lastBytes_.constFind(id); it != lastBytes_.constEnd())
                {
                    baseUp = it->first;
                    baseDown = it->second;
                }
                qint64 dUp = curUp - baseUp;
                qint64 dDown = curDown - baseDown;
                if (dUp < 0) dUp = 0;
                if (dDown < 0) dDown = 0;
                if (dUp == 0 && dDown == 0) return;
                const QString& name = proc.isEmpty() ? s_unknown : proc;
                trafficStatsManager->AddAppDelta(name, procPath, dUp, dDown);
            };

            for (const auto& r : rows)
            {
                credit(r.id, r.process, r.processPath, r.upload, r.download);
                newLast.insert(r.id, {r.upload, r.download});
            }
            for (const auto& cm : resp.closed)
            {
                const QString id = QString::fromStdString(cm.id.value());
                currentClosed.insert(id);
                if (accountedClosed_.contains(id)) continue;
                const QString proc = stringCache.getOrInsert(cm.process.value());
                const QString procPath = stringCache.getOrInsert(cm.process_path.value());
                credit(id, proc, procPath, cm.upload.value(), cm.download.value());
            }
            lastBytes_ = newLast;             // drop evicted / now-closed live ids
            accountedClosed_ = currentClosed; // everything in the ring is accounted
        }

        if (pushToApi)
        {
            QList<ConnectionMetadata> apiRows = rows;
            QSet<QString> openIds;
            openIds.reserve(rows.size());
            for (const auto& row : rows) openIds.insert(row.id);
            for (const auto& conn : resp.closed)
            {
                auto c = metaFromProto(conn, localLabel, outboundNames, stringCache, ownAddressCache);
                // A connection that closed while the core listed it shows up in both lists.
                if (!openIds.contains(c.id)) apiRows.append(std::move(c));
            }
            sortApiSnapshot(apiRows);
            if (apiRows.size() > kSnapshotMaxRows) apiRows.resize(kSnapshotMaxRows);
            runOnUiThread([this, apiRows = std::move(apiRows), nowMs] {
                if (apiSink_) apiSink_(apiRows, nowMs);
            });
        }

        if (!pushToUi) return;

        switch (sort)
        {
        // Oldest first, matching how rows used to accumulate. `asc` is ignored: this means "unsorted".
        case Default:
            sortSmallestFirst(rows, false, [](const ConnectionMetadata& c) { return c.createdAtMs; });
            break;
        case ByDownload:
            sortLargestFirst(rows, asc, [](const ConnectionMetadata& c) { return c.download; });
            break;
        case ByUpload:
            sortLargestFirst(rows, asc, [](const ConnectionMetadata& c) { return c.upload; });
            break;
        case ByTraffic:
            sortLargestFirst(rows, asc, [](const ConnectionMetadata& c) { return c.upload + c.download; });
            break;
        case ByDownloadSpeed:
            sortLargestFirst(rows, asc, [](const ConnectionMetadata& c) { return c.downloadSpeed; });
            break;
        case ByUploadSpeed:
            sortLargestFirst(rows, asc, [](const ConnectionMetadata& c) { return c.uploadSpeed; });
            break;
        case BySpeed:
            sortLargestFirst(rows, asc, [](const ConnectionMetadata& c) { return c.uploadSpeed + c.downloadSpeed; });
            break;
        case ByProcess:
            sortSmallestFirst(rows, asc, [](const ConnectionMetadata& c) -> const QString& { return c.process; });
            break;
        case ByOutbound:
            sortSmallestFirst(rows, asc, [](const ConnectionMetadata& c) -> const QString& { return c.outboundName; });
            break;
        case ByProtocol:
            sortSmallestFirst(rows, asc, [](const ConnectionMetadata& c) -> const QString& { return c.protocol; });
            break;
        case BySource:
            sortSmallestFirst(rows, asc, [](const ConnectionMetadata& c) -> const QString& { return c.sourceDisplay; });
            break;
        }

        runOnUiThread([rows = std::move(rows)] {
            if (auto* sink = CurrentStatsSink()) sink->onConnections(rows);
        });
    }

    void ConnectionLister::stopLoop()
    {
        stop = true;
        QMutexLocker wlk(&waitMu_);
        waitCond_.wakeAll();
    }

    void ConnectionLister::setSort(const ConnectionSort newSort)
    {
        if (sort == newSort) asc = !asc;
        else
        {
            sort = newSort;
            asc = false;
        }
    }

    void ConnectionLister::restoreSort(const ConnectionSort newSort, const bool ascending)
    {
        sort = newSort;
        asc = ascending;
    }

}
