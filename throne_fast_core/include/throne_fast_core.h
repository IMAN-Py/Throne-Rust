#pragma once

/**
 * @file throne_fast_core.h
 * @brief High-Performance Acceleration Core C-ABI and C++20 Interface for Throne-Fast.
 *
 * Provides zero-allocation IPv4 Radix Trie route matching, fast streaming 64-bit
 * FNV-1a hashing, and benchmark utilities backed by the native Rust crate
 * `throne_fast_core`.
 *
 * ============================================================================
 * LINKAGE & MSVC COMPATIBILITY:
 * ============================================================================
 * - When consuming this header on Windows from dynamic builds, `THRONE_FAST_API`
 *   evaluates to `__declspec(dllimport)`.
 * - The calling target MUST link against the import library `throne_fast_core.dll.lib`
 *   in CMake:
 *     target_link_libraries(Throne PRIVATE "${THRONE_FAST_TARGET_DIR}/throne_fast_core.dll.lib")
 * - If linking statically, define `THRONE_FAST_STATIC` before including this header.
 * - If compiling the export DLL (e.g. C bridge), define `THRONE_FAST_CORE_EXPORTS`.
 *
 * ============================================================================
 * THREAD-SAFETY & CONCURRENCY CONTRACT:
 * ============================================================================
 * 1. Concurrent Reads (Querying):
 *    - `throne_fast_route_match()` and `throne::fast::FastRouteEngine::match()`
 *      are COMPLETELY REENTRANT and THREAD-SAFE for concurrent invocations across
 *      arbitrary threads.
 *    - The underlying Rust traversal borrows `&self` and performs zero memory
 *      allocations, avoiding cache contention and lock overhead.
 *
 * 2. Concurrent Writes (Mutation):
 *    - `throne_fast_route_add()` and `throne::fast::FastRouteEngine::addPrefix()`
 *      mutate internal trie nodes (`&mut self` in Rust).
 *    - Calling `addPrefix()` concurrently with other `addPrefix()` calls, or
 *      concurrently with `match()` calls, constitutes a DATA RACE and is NOT safe.
 *    - External Synchronization: If routing tables are updated dynamically at runtime
 *      while worker threads are serving traffic, the caller MUST synchronize using
 *      a reader-writer lock (e.g. `std::shared_mutex` with `std::shared_lock` for
 *      `match()` and `std::unique_lock` for `addPrefix()`, or Qt's `QReadWriteLock`).
 *    - Immutable Pattern (Recommended): Fully populate routes during initialization
 *      or on a dedicated thread, and then share the engine across worker threads
 *      as read-only without locks.
 *
 * 3. Lifecycle / Destruction:
 *    - `throne_fast_route_free()`, `reset()`, and `~FastRouteEngine()` must only be
 *      called when no reader or writer thread is accessing the engine instance.
 *
 * 4. Stateless Functions:
 *    - `throne_fast_hash()` / `throne::fast::hashBuffer()` and
 *      `throne_fast_version()` / `throne::fast::getVersion()` are stateless, pure,
 *      and completely thread-safe.
 * ============================================================================
 */

#ifdef __cplusplus
#include <cstdint>
#include <cstddef>
#include <memory>
#include <string>
#include <utility>
extern "C" {
#else
#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>
#endif

/* Export / Import macro definition */
#if defined(_WIN32) || defined(__CYGWIN__)
  #if defined(THRONE_FAST_STATIC)
    #define THRONE_FAST_API
  #elif defined(THRONE_FAST_CORE_EXPORTS)
    #define THRONE_FAST_API __declspec(dllexport)
  #else
    #define THRONE_FAST_API __declspec(dllimport)
  #endif
#elif defined(__GNUC__) && __GNUC__ >= 4
  #define THRONE_FAST_API __attribute__((visibility("default")))
#else
  #define THRONE_FAST_API
#endif

// ---------------------------------------------------------------------------
// C-ABI Exported Function Declarations
// ---------------------------------------------------------------------------

/**
 * @brief Returns the semantic version string of the acceleration core.
 * @return Null-terminated ASCII string (e.g. "1.4.2-fast").
 * @note Thread-safe. Points to static memory. Do not free.
 */
THRONE_FAST_API const char* throne_fast_version(void);

/**
 * @brief Computes a streaming 64-bit FNV-1a hash over a contiguous byte buffer.
 * @param data Pointer to buffer. If NULL or len is 0, returns 0.
 * @param len Buffer length in bytes.
 * @return 64-bit unsigned hash integer.
 * @note Thread-safe, stateless, zero allocations.
 */
THRONE_FAST_API uint64_t throne_fast_hash(const uint8_t* data, size_t len);

/**
 * @brief Creates a new heap-allocated RouteEngine (32-bit IPv4 Radix Trie).
 * @return Opaque pointer to RouteEngine, or NULL on allocation failure.
 * @note Caller owns the returned engine and must release it with `throne_fast_route_free`.
 */
THRONE_FAST_API void* throne_fast_route_create(void);

/**
 * @brief Frees a previously created RouteEngine.
 * @param engine Opaque pointer to RouteEngine. Safe to pass NULL (no-op).
 * @warning Must not be called while other threads are reading from or writing to `engine`.
 */
THRONE_FAST_API void throne_fast_route_free(void* engine);

/**
 * @brief Inserts an IPv4 network prefix route into the radix trie.
 * @param engine Opaque pointer to RouteEngine.
 * @param ip IPv4 network address in HOST BYTE ORDER (e.g. 0x0A000000 for 10.0.0.0).
 * @param prefix_len CIDR prefix length (0..32). Values > 32 are clamped or rejected.
 * @param tag 16-bit routing tag/identifier associated with this prefix.
 * @return 0 on success, -1 on invalid argument or null engine pointer.
 * @warning NOT thread-safe with concurrent reads or writes. Requires external synchronization.
 */
THRONE_FAST_API int throne_fast_route_add(void* engine, uint32_t ip, uint8_t prefix_len, uint16_t tag);

/**
 * @brief Performs zero-allocation Longest Prefix Match (LPM) for an IPv4 address.
 * @param engine Opaque pointer to RouteEngine.
 * @param ip IPv4 destination address in HOST BYTE ORDER (e.g. 0x0A010203 for 10.1.2.3).
 * @return Matched routing tag (>= 0), or -1 if no prefix matched or engine is NULL.
 * @note Fully thread-safe and reentrant for concurrent callers. Zero heap allocations.
 */
THRONE_FAST_API int throne_fast_route_match(const void* engine, uint32_t ip);

/**
 * @brief Executes an empirical benchmark of N route match operations.
 * @param iterations Number of lookup iterations (e.g. 10,000,000).
 * @return Total elapsed time in seconds, or -1.0 on error.
 */
THRONE_FAST_API double throne_fast_benchmark(uint32_t iterations);

#ifdef __cplusplus
} // extern "C"

// ---------------------------------------------------------------------------
// C++20 RAII Abstraction & Ergonomic Helpers
// ---------------------------------------------------------------------------

namespace throne::fast {

/**
 * @class FastRouteEngine
 * @brief Modern C++ RAII wrapper managing the lifecycle and queries of a Rust RouteEngine.
 *
 * Implements strict move-only semantics (Rule of 5). Copy construction and copy
 * assignment are deleted to prevent accidental double-free of the underlying native handle.
 *
 * Concurrency:
 * - `match()` is thread-safe for concurrent read queries across multiple threads.
 * - `addPrefix()`, `reset()`, and `clear()` require external exclusive synchronization.
 *
 * Endianness:
 * - All IPv4 addresses are represented in HOST BYTE ORDER (e.g. 192.168.1.1 is 0xC0A80101).
 * - Convenience overloads accepting octets (a, b, c, d) are provided.
 */
class FastRouteEngine {
public:
    /**
     * @brief Constructs a new FastRouteEngine instance.
     */
    FastRouteEngine() : m_engine(throne_fast_route_create()) {}

    /**
     * @brief Takes ownership of an existing raw RouteEngine handle.
     * @param engine Opaque pointer to RouteEngine (may be nullptr).
     */
    explicit FastRouteEngine(void* engine) noexcept : m_engine(engine) {}

    /**
     * @brief Destructor. Releases the underlying RouteEngine instance.
     */
    ~FastRouteEngine() noexcept {
        reset();
    }

    // Disable copy semantics
    FastRouteEngine(const FastRouteEngine&) = delete;
    FastRouteEngine& operator=(const FastRouteEngine&) = delete;

    /**
     * @brief Move constructor. Transfers handle ownership.
     */
    FastRouteEngine(FastRouteEngine&& other) noexcept : m_engine(other.release()) {}

    /**
     * @brief Move assignment operator. Releases existing engine and steals from other.
     */
    FastRouteEngine& operator=(FastRouteEngine&& other) noexcept {
        if (this != &other) {
            reset(other.release());
        }
        return *this;
    }

    /**
     * @brief Inserts an IPv4 network prefix with host byte order address.
     * @param ip IPv4 address in host byte order (e.g. 0x0A000000).
     * @param prefix_len CIDR prefix length (0..32). Values > 32 return false.
     * @param tag 16-bit tag identifier.
     * @return true on success, false on invalid parameter or unallocated engine.
     */
    [[nodiscard]] bool addPrefix(uint32_t ip, uint8_t prefix_len, uint16_t tag) {
        if (prefix_len > 32 || !m_engine) {
            return false;
        }
        return throne_fast_route_add(m_engine, ip, prefix_len, tag) == 0;
    }

    /**
     * @brief Inserts an IPv4 network prefix with 4 dotted-quad octets.
     * @param a 1st octet (MSB)
     * @param b 2nd octet
     * @param c 3rd octet
     * @param d 4th octet (LSB)
     * @param prefix_len CIDR prefix length (0..32).
     * @param tag 16-bit tag identifier.
     * @return true on success, false on failure.
     */
    [[nodiscard]] bool addPrefix(uint8_t a, uint8_t b, uint8_t c, uint8_t d, uint8_t prefix_len, uint16_t tag) {
        return addPrefix(makeIp(a, b, c, d), prefix_len, tag);
    }

    /**
     * @brief Matches an IPv4 address against the routing table using Longest Prefix Match.
     * @param ip Destination IPv4 address in host byte order.
     * @return Matched route tag (>= 0), or -1 if no matching route is found.
     * @note Thread-safe for concurrent readers.
     */
    [[nodiscard]] int match(uint32_t ip) const noexcept {
        return m_engine ? throne_fast_route_match(m_engine, ip) : -1;
    }

    /**
     * @brief Matches an IPv4 address specified as 4 octets.
     * @return Matched route tag (>= 0), or -1 if no matching route is found.
     * @note Thread-safe for concurrent readers.
     */
    [[nodiscard]] int match(uint8_t a, uint8_t b, uint8_t c, uint8_t d) const noexcept {
        return match(makeIp(a, b, c, d));
    }

    /**
     * @brief Checks if the underlying engine handle is valid.
     */
    [[nodiscard]] bool isValid() const noexcept {
        return m_engine != nullptr;
    }

    /**
     * @brief Boolean conversion operator.
     */
    explicit operator bool() const noexcept {
        return isValid();
    }

    /**
     * @brief Returns the underlying non-owning raw handle.
     */
    [[nodiscard]] void* get() const noexcept {
        return m_engine;
    }

    /**
     * @brief Releases ownership of the underlying raw handle without freeing it.
     */
    [[nodiscard]] void* release() noexcept {
        void* ptr = m_engine;
        m_engine = nullptr;
        return ptr;
    }

    /**
     * @brief Frees the current engine and optionally takes ownership of a new handle.
     * @param engine New opaque handle, or nullptr.
     */
    void reset(void* engine = nullptr) noexcept {
        if (m_engine) {
            throne_fast_route_free(m_engine);
        }
        m_engine = engine;
    }

    /**
     * @brief Clears and reinitializes the routing table with a fresh engine.
     */
    void clear() {
        reset(throne_fast_route_create());
    }

    /**
     * @brief Helper to construct an IPv4 integer in host byte order from 4 octets.
     */
    [[nodiscard]] static constexpr uint32_t makeIp(uint8_t a, uint8_t b, uint8_t c, uint8_t d) noexcept {
        return (static_cast<uint32_t>(a) << 24) |
               (static_cast<uint32_t>(b) << 16) |
               (static_cast<uint32_t>(c) << 8)  |
                static_cast<uint32_t>(d);
    }

private:
    void* m_engine = nullptr;
};

/**
 * @brief Computes 64-bit FNV-1a hash over raw memory buffer.
 * @param data Buffer pointer.
 * @param len Buffer length in bytes.
 * @return 64-bit unsigned hash.
 * @note Thread-safe, stateless.
 */
[[nodiscard]] inline uint64_t hashBuffer(const void* data, size_t len) noexcept {
    return throne_fast_hash(reinterpret_cast<const uint8_t*>(data), len);
}

/**
 * @brief Returns the module version as a std::string.
 * @return Version string (e.g. "1.4.2-fast").
 * @note Thread-safe.
 */
[[nodiscard]] inline std::string getVersion() {
    const char* v = throne_fast_version();
    return v ? std::string(v) : std::string("unknown");
}

} // namespace throne::fast
#endif
