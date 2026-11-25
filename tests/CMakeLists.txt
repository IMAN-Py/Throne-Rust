cmake_minimum_required(VERSION 3.20)
project(ThroneFastTests LANGUAGES C CXX)
if (MSVC)
    set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreaded$<$<CONFIG:Debug>:Debug>")
endif ()

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

get_filename_component(THRONE_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/.." ABSOLUTE)

find_package(Qt6 REQUIRED COMPONENTS Core Gui Widgets Network Test)
find_package(OpenSSL REQUIRED)
find_package(Threads REQUIRED)

set(THRONE_FAST_CORE_DLL "${THRONE_ROOT}/throne_fast_core/target/release/throne_fast_core.dll" CACHE FILEPATH "")
set(THRONE_FAST_CORE_LIB "${THRONE_ROOT}/throne_fast_core/target/release/throne_fast_core.dll.lib" CACHE FILEPATH "")
set(THRONE_SRSLIST_DIR "${THRONE_ROOT}/../_throne_sdk" CACHE PATH "")

configure_file(
    "${THRONE_ROOT}/3rdparty/simple-protobuf/CMakeLists.txt"
    "${CMAKE_CURRENT_BINARY_DIR}/.spb_check"
    COPYONLY
)
add_subdirectory("${THRONE_ROOT}/3rdparty/simple-protobuf" "${CMAKE_CURRENT_BINARY_DIR}/spb")
spb_protobuf_generate(OUT_VAR TEST_PROTO_FILES PROTOS "${THRONE_ROOT}/core/gen/libcore.proto" IMPORT_DIRS "${THRONE_ROOT}")

add_library(test_myproto STATIC ${TEST_PROTO_FILES})
target_include_directories(test_myproto PUBLIC "${CMAKE_CURRENT_BINARY_DIR}")
target_link_libraries(test_myproto PUBLIC spb-proto)

file(WRITE "${CMAKE_CURRENT_BINARY_DIR}/NkrVersion.h" "#pragma once\n#define NKR_VERSION \"1.4.2-rust\"\n")

# cpp_fast_core_test
add_executable(cpp_fast_core_test
    "${CMAKE_CURRENT_SOURCE_DIR}/cpp_fast_core_test.cpp"
)
target_include_directories(cpp_fast_core_test PRIVATE "${THRONE_ROOT}/include")
target_link_libraries(cpp_fast_core_test PRIVATE "${THRONE_FAST_CORE_LIB}")

set(QT_TEST_PROD_SOURCES
    "${THRONE_ROOT}/3rdparty/SQLiteCpp/src/Backup.cpp"
    "${THRONE_ROOT}/3rdparty/SQLiteCpp/src/Column.cpp"
    "${THRONE_ROOT}/3rdparty/SQLiteCpp/src/Database.cpp"
    "${THRONE_ROOT}/3rdparty/SQLiteCpp/src/Exception.cpp"
    "${THRONE_ROOT}/3rdparty/SQLiteCpp/src/Savepoint.cpp"
    "${THRONE_ROOT}/3rdparty/SQLiteCpp/src/sqlite3.c"
    "${THRONE_ROOT}/3rdparty/SQLiteCpp/src/Statement.cpp"
    "${THRONE_ROOT}/3rdparty/SQLiteCpp/src/Transaction.cpp"
    "${THRONE_ROOT}/3rdparty/monocypher/monocypher.c"

    "${THRONE_ROOT}/src/scanner/IpListParse.cpp"

    "${THRONE_ROOT}/src/database/Database.cpp"
    "${THRONE_ROOT}/src/database/DatabaseManager.cpp"
    "${THRONE_ROOT}/src/database/GroupsRepo.cpp"
    "${THRONE_ROOT}/src/database/IpListsRepo.cpp"
    "${THRONE_ROOT}/src/database/IpScansRepo.cpp"
    "${THRONE_ROOT}/src/database/MarkersRepo.cpp"
    "${THRONE_ROOT}/src/database/OtpProfilesRepo.cpp"
    "${THRONE_ROOT}/src/database/ProfilesRepo.cpp"
    "${THRONE_ROOT}/src/database/RoutesRepo.cpp"
    "${THRONE_ROOT}/src/database/SettingsRepo.cpp"
    "${THRONE_ROOT}/src/database/SettingsSchema.cpp"
    "${THRONE_ROOT}/src/database/TrafficStatsRepo.cpp"

    "${THRONE_ROOT}/src/database/entities/EndpointSource.cpp"
    "${THRONE_ROOT}/src/database/entities/Group.cpp"
    "${THRONE_ROOT}/src/database/entities/IpScan.cpp"
    "${THRONE_ROOT}/src/database/entities/OtpProfile.cpp"
    "${THRONE_ROOT}/src/database/entities/Profile.cpp"
    "${THRONE_ROOT}/src/database/entities/RouteProfile.cpp"
    "${THRONE_ROOT}/src/database/entities/RouteRule.cpp"

    "${THRONE_ROOT}/src/configs/common/DialFields.cpp"
    "${THRONE_ROOT}/src/configs/common/DnsTunnel.cpp"
    "${THRONE_ROOT}/src/configs/common/Outbound.cpp"
    "${THRONE_ROOT}/src/configs/common/Protocols.cpp"
    "${THRONE_ROOT}/src/configs/common/QUICFields.cpp"
    "${THRONE_ROOT}/src/configs/common/ResolverSource.cpp"
    "${THRONE_ROOT}/src/configs/common/TLS.cpp"
    "${THRONE_ROOT}/src/configs/common/multiplex.cpp"
    "${THRONE_ROOT}/src/configs/common/transport.cpp"
    "${THRONE_ROOT}/src/configs/common/utils.cpp"
    "${THRONE_ROOT}/src/configs/common/xrayMultiplex.cpp"
    "${THRONE_ROOT}/src/configs/common/xrayStreamSetting.cpp"

    "${THRONE_ROOT}/src/configs/outbounds/anyTLS.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/autoselector.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/custom.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/dnstt.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/http.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/hysteria.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/masque.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/masterdns.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/mieru.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/naive.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/openconnect.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/openvpn.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/shadowsocks.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/shadowtls.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/snell.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/socks.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/ssh.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/tailscale.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/trojan.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/trusttunnel.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/tuic.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/vless.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/vmess.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/wireguard.cpp"
    "${THRONE_ROOT}/src/configs/outbounds/xrayVless.cpp"

    "${THRONE_ROOT}/src/global/Common.cpp"
    "${THRONE_ROOT}/src/global/Configs.cpp"
    "${THRONE_ROOT}/src/global/CountryHelper.cpp"
    "${THRONE_ROOT}/src/global/DeviceDetailsHelper.cpp"
    "${THRONE_ROOT}/src/global/LocalNetwork.cpp"
    "${THRONE_ROOT}/src/global/LogBuffer.cpp"
    "${THRONE_ROOT}/src/global/Logger.cpp"
    "${THRONE_ROOT}/src/global/Notice.cpp"
    "${THRONE_ROOT}/src/global/OTP.cpp"
    "${THRONE_ROOT}/src/global/OtpPlaceholder.cpp"
    "${THRONE_ROOT}/src/global/Utils.cpp"
    "${THRONE_ROOT}/src/global/Validation.cpp"
    "${THRONE_ROOT}/src/global/VpnCredentialOverride.cpp"

    "${THRONE_ROOT}/src/engine/EngineEvents.cpp"
    "${THRONE_ROOT}/src/engine/Lifecycle.cpp"

    "${THRONE_ROOT}/src/ui/utils/DataViewHtmlGenerator.cpp"
    "${THRONE_ROOT}/src/ui/setting/ThemeManager.cpp"
    "${THRONE_ROOT}/include/ui/setting/ThemeManager.hpp"
    "${THRONE_ROOT}/include/engine/EngineEvents.h"
)

if (WIN32)
    list(APPEND QT_TEST_PROD_SOURCES
        "${THRONE_ROOT}/src/sys/windows/WinVersion.cpp"
    )
endif ()

add_executable(qt_fast_integration_test
    "${CMAKE_CURRENT_SOURCE_DIR}/qt_fast_integration_test.cpp"
    "${CMAKE_CURRENT_SOURCE_DIR}/qt_test_stubs.cpp"
    ${QT_TEST_PROD_SOURCES}
)

target_include_directories(qt_fast_integration_test PRIVATE
    "${THRONE_ROOT}"
    "${CMAKE_CURRENT_BINARY_DIR}"
    "${THRONE_SRSLIST_DIR}"
    "${THRONE_ROOT}/3rdparty/SQLiteCpp/include"
)

target_compile_definitions(qt_fast_integration_test PRIVATE
    NOMINMAX
    WIN32_LEAN_AND_MEAN
    UNICODE
    _UNICODE
    QT_MESSAGELOGCONTEXT
    SQLITE_ENABLE_COLUMN_METADATA
)

if (MSVC)
    set_target_properties(qt_fast_integration_test PROPERTIES UNITY_BUILD ON UNITY_BUILD_BATCH_SIZE 16)
    set_source_files_properties(
        "${THRONE_ROOT}/3rdparty/SQLiteCpp/src/sqlite3.c"
        "${THRONE_ROOT}/3rdparty/monocypher/monocypher.c"
        PROPERTIES SKIP_UNITY_BUILD_INCLUSION ON
    )
    target_compile_options(qt_fast_integration_test PRIVATE /utf-8 /bigobj)
endif ()

target_link_libraries(qt_fast_integration_test PRIVATE
    Qt6::Core
    Qt6::Gui
    Qt6::Widgets
    Qt6::Network
    Qt6::Test
    OpenSSL::Crypto
    OpenSSL::SSL
    Threads::Threads
    test_myproto
    "${THRONE_FAST_CORE_LIB}"
)

if (WIN32)
    target_link_libraries(qt_fast_integration_test PRIVATE ws2_32 iphlpapi crypt32 winmm advapi32 user32 shell32 ole32)
endif ()

add_custom_command(TARGET cpp_fast_core_test POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_if_different "${THRONE_FAST_CORE_DLL}" "$<TARGET_FILE_DIR:cpp_fast_core_test>/throne_fast_core.dll"
)
add_custom_command(TARGET qt_fast_integration_test POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_if_different "${THRONE_FAST_CORE_DLL}" "$<TARGET_FILE_DIR:qt_fast_integration_test>/throne_fast_core.dll"
)
