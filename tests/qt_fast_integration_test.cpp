#include <QtTest>
#include <QDir>
#include <QTemporaryDir>

#include "include/throne_fast_core.h"
#include "include/database/DatabaseManager.h"
#include "include/database/GroupsRepo.h"
#include "include/database/IpListsRepo.h"
#include "include/database/ProfilesRepo.h"
#include "include/database/entities/Group.h"
#include "include/database/entities/Profile.h"
#include "include/scanner/IpListParse.h"
#include "include/ui/utils/DataViewHtmlGenerator.h"

namespace Configs {
    struct ProfilesRepoTestAccess {
        static std::unordered_map<int, std::shared_ptr<Profile>> loadChunk(
            const ProfilesRepo &repo, const QList<int> &ids) {
            return repo.loadProfilesByIdsChunk(ids);
        }
    };
}

class QtFastIntegrationTest : public QObject {
    Q_OBJECT

private slots:
    void testCompactCidrParsingAndParity() {
        struct Case {
            const char *input;
            const char *expected;
        };
        const Case cases[] = {
            {"1.2.3.4", "1.2.3.4"},
            {"1.2.3.4/32", "1.2.3.4"},
            {"192.168.1.99/24", "192.168.1.0/24"},
            {"2001:0db8:0000:0000::1/128", "2001:db8::1"},
            {"[2001:db8::1]/64", "2001:db8::/64"},
            {"[2001:db8::1/64]", "2001:db8::/64"},
            {"fe80::1%eth0/64", "fe80::/64"},
            {"::ffff:192.0.2.5/120", "192.0.2.0/24"},
            {"0.0.0.0/0", ""},
            {"::/0", ""},
            {"010.0.0.1", ""},
            {"1.2.3", ""},
        };
        for (const auto &c : cases) {
            const QString gotQt = Scanner::NormalizeCidr(QString::fromUtf8(c.input));
            const std::string gotRust = throne::fast::normalize_cidr_compact(c.input);
            QCOMPARE(gotQt, QString::fromUtf8(c.expected));
            QCOMPARE(gotQt.toStdString(), gotRust);
        }

        const QByteArray listText =
            "# comment\n"
            "192.168.1.5-192.168.1.10\n"
            "192.168.1.5\n"
            "1.1.1.1:443\n"
            "1.1.1.1:443\n"
            "[2001:db8::1]:8443\n";
        const auto parsed = Scanner::ParseIpListText(listText, 0);
        QVERIFY(parsed.rejected == 0);
        QCOMPARE(parsed.duplicates, 2);
        QCOMPARE(parsed.entries.size(), 6);
        QCOMPARE(parsed.entries[0].cidr, QStringLiteral("192.168.1.5"));
        QCOMPARE(parsed.entries[1].cidr, QStringLiteral("192.168.1.6/31"));
        QCOMPARE(parsed.entries[2].cidr, QStringLiteral("192.168.1.8/31"));
        QCOMPARE(parsed.entries[3].cidr, QStringLiteral("192.168.1.10"));
        QCOMPARE(parsed.entries[4].port, 443);
        QCOMPARE(parsed.entries[5].port, 8443);

        const auto deduped = Scanner::EntriesFromCidrs(
            {QStringLiteral("1.1.1.1/32"), QStringLiteral("1.1.1.1"), QStringLiteral("2.2.2.0/24")},
            443);
        QCOMPARE(deduped.size(), 2);
        QCOMPARE(deduped[0].cidr, QStringLiteral("1.1.1.1"));
        QCOMPARE(deduped[1].cidr, QStringLiteral("2.2.2.0/24"));
    }

    void testProfileFilterCorrectness() {
        auto makeProf = [](int id, const QString &name, const QString &server, int port) {
            auto p = Configs::ProfilesRepo::NewProfile(QStringLiteral("socks"));
            p->id = id;
            p->outbound->name = name;
            p->name = name;
            p->outbound->server = server;
            p->outbound->server_port = port;
            return p;
        };

        auto a1 = makeProf(1, QStringLiteral("Alpha"), QStringLiteral("1.1.1.1"), 1080);
        auto a2 = makeProf(2, QStringLiteral("AlphaDup"), QStringLiteral("1.1.1.1"), 1080);
        auto b1 = makeProf(3, QStringLiteral("Beta"), QStringLiteral("2.2.2.2"), 1080);

        QList<std::shared_ptr<Configs::Profile>> uniqFirst;
        Configs::ProfileFilter::Uniq({a1, a2, b1}, uniqFirst, false, true);
        QCOMPARE(uniqFirst.size(), 2);
        QCOMPARE(uniqFirst[0]->id, 1);
        QCOMPARE(uniqFirst[1]->id, 3);

        QList<std::shared_ptr<Configs::Profile>> uniqLast;
        Configs::ProfileFilter::Uniq({a1, a2, b1}, uniqLast, true, true);
        QCOMPARE(uniqLast.size(), 2);
        QCOMPARE(uniqLast[0]->id, 2);
        QCOMPARE(uniqLast[1]->id, 3);

        QList<std::shared_ptr<Configs::Profile>> outSrc, outDst;
        Configs::ProfileFilter::Common({a1, b1}, {a2}, outSrc, outDst, true);
        QCOMPARE(outSrc.size(), 1);
        QCOMPARE(outDst.size(), 1);
        QCOMPARE(outSrc[0]->id, 1);
        QCOMPARE(outDst[0]->id, 2);

        QList<std::shared_ptr<Configs::Profile>> onlySrc;
        Configs::ProfileFilter::OnlyInSrc({a1, b1}, {a2}, onlySrc, true);
        QCOMPARE(onlySrc.size(), 1);
        QCOMPARE(onlySrc[0]->id, 3);

        QList<std::shared_ptr<Configs::Profile>> onlyPtr;
        Configs::ProfileFilter::OnlyInSrc_ByPointer({a1, b1}, {a1}, onlyPtr);
        QCOMPARE(onlyPtr.size(), 1);
        QCOMPARE(onlyPtr[0], b1);
    }

    void testProfilesRepoBatchAndHydration() {
        QTemporaryDir dir;
        QVERIFY(dir.isValid());
        const QString dbPath = dir.filePath(QStringLiteral("test_throne.db"));
        Configs::DatabaseManager mgr(dbPath.toStdString());
        Configs::dataManager = &mgr;
        struct ResetDataManager {
            ~ResetDataManager() { Configs::dataManager = nullptr; }
        } resetGuard;

        auto group = Configs::GroupsRepo::NewGroup();
        group->name = QStringLiteral("Default");
        QVERIFY(mgr.groupsRepo->AddGroup(group));

        QList<std::shared_ptr<Configs::Profile>> batch;
        for (int i = 0; i < 40; ++i) {
            auto p = Configs::ProfilesRepo::NewProfile(QStringLiteral("socks"));
            p->outbound->name = QStringLiteral("Node-%1").arg(i);
            p->name = p->outbound->name;
            p->outbound->server = QStringLiteral("10.0.0.%1").arg(i + 1);
            p->outbound->server_port = 1080 + i;
            p->latency = 25 + i;
            batch.append(p);
        }
        QVERIFY(mgr.profilesRepo->AddProfileBatch(batch, group->id));

        QList<int> ids;
        for (const auto &p : batch) {
            QVERIFY(p->id >= 0);
            ids.append(p->id);
        }

        // Verify idx_profiles_gid exists
        auto idxQ = mgr.getDatabase().query(
            "SELECT name FROM sqlite_master WHERE type='index' AND name='idx_profiles_gid'");
        QVERIFY(idxQ && idxQ->executeStep());

        // Direct row hydration bypassing identityMap
        const auto rawChunk = Configs::ProfilesRepoTestAccess::loadChunk(*mgr.profilesRepo, ids);
        QCOMPARE(static_cast<int>(rawChunk.size()), 40);
        for (int i = 0; i < 40; ++i) {
            auto it = rawChunk.find(ids[i]);
            QVERIFY(it != rawChunk.end());
            QCOMPARE(it->second->name, QStringLiteral("Node-%1").arg(i));
            QCOMPARE(it->second->outbound->server, QStringLiteral("10.0.0.%1").arg(i + 1));
            QCOMPARE(it->second->outbound->server_port, 1080 + i);
            QCOMPARE(it->second->latency, 25 + i);
        }

        const auto loaded = mgr.profilesRepo->GetProfileBatch(ids);
        QCOMPARE(loaded.size(), 40);
        for (int i = 0; i < 40; ++i) {
            QCOMPARE(loaded[i]->id, ids[i]);
            QCOMPARE(loaded[i]->name, QStringLiteral("Node-%1").arg(i));
        }

        // IpListsRepo deduplication with Rust xxHash64 key
        auto list = Configs::IpListsRepo::NewIpList();
        list->name = QStringLiteral("CF Test");
        QList<Configs::IpListEntry> entries{
            {QStringLiteral("1.1.1.0/24"), 443, 10, 0},
            {QStringLiteral("1.1.1.0/24"), 443, 20, 0},
            {QStringLiteral("1.1.1.0/24"), 8443, 15, 0},
        };
        QVERIFY(mgr.ipListsRepo->AddIpList(list));
        QVERIFY(mgr.ipListsRepo->ReplaceEntries(list->id, entries));
        const auto savedEntries = mgr.ipListsRepo->GetEntries(list->id);
        QCOMPARE(savedEntries.size(), 2);
    }

    void testDataViewHtmlGeneratorSeq() {
        DataViewHtmlGenerator gen;
        const auto c1 = gen.build();
        gen.setAutoSelectorStatus(QStringLiteral("Selected"), QStringLiteral("Fastest node"));
        const auto c2 = gen.build();
        gen.setPendingRestartReasons({QStringLiteral("DNS changed")});
        const auto c3 = gen.build();
        QVERIFY(c1.seq < c2.seq);
        QVERIFY(c2.seq < c3.seq);
        QVERIFY(!c3.html.isEmpty());
    }
};

QTEST_MAIN(QtFastIntegrationTest)
#include "qt_fast_integration_test.moc"
