#include <QList>
#include <QString>
#include <memory>

#include "include/api/RPC.h"
#include "include/configs/ParseDiagnostics.h"

class QWidget;

bool Windows_IsInAdmin() { return false; }
void Windows_QWidget_SetForegroundWindow(QWidget *) {}
bool UrlScheme_AutoRegisterByDefault() { return false; }

namespace API {
    bool Client::IsConnected() const { return false; }
    PrivilegeInfo Client::Privileges(bool *rpcOK, int) const {
        if (rpcOK) *rpcOK = false;
        return {};
    }
}

namespace Scanner::DefaultIpLists {
    int Ensure(const QString &) { return -1; }
    int EnsureAll() { return 0; }
    bool MigrateWarpMasqueRanges() { return false; }
    bool MigrateListSeedsNov2026() { return false; }
}

namespace Configs {
    class Profile;
    class openconnect;
    class openvpn;

    bool IsDnsTunnelType(const QString &) { return false; }
    bool CanBeAuxEndpoint(const std::shared_ptr<Profile> &) { return false; }
    QList<int> AuxEndpointInnerHops(int) { return {}; }
    QString DisplayEffectiveAddress(const std::shared_ptr<Profile> &) { return {}; }
    QString DnsAddressListProblem(const QString &) { return {}; }
    int ActiveRouteId() { return -1; }
    bool ParseOpenConnectProfile(const QString &, openconnect &) { return false; }
    bool ParseOvpnConfig(const QString &, openvpn &) { return false; }

    namespace ProcessRules {
        QString KnownSpelling(const QString &, const QString &) { return {}; }
    }

    namespace ParseDiag {
        Scope::Scope() : previous(nullptr) {}
        Scope::~Scope() = default;
        QList<Issue> Scope::take() { return {}; }
        bool Active() { return false; }
        void Warn(const QString &, const QString &, const QString &) {}
        void Error(const QString &, const QString &, const QString &) {}
    }
}
