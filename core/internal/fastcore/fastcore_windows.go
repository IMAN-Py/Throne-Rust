//go:build windows

package fastcore

import (
	"os"
	"path/filepath"
	"sync"
	"syscall"
	"unsafe"

	"golang.org/x/sys/windows"
)

const ExpectedABIVersion uint32 = 0x00020001

// ScanRangeDesc matches #[repr(C)] ThroneFastScanRangeDesc (64 bytes).
type ScanRangeDesc struct {
	Base       [16]byte
	Start      uint64
	End        uint64
	HighKey    uint64
	SamplerKey uint64
	PortOffset uint32
	PortCount  uint32
	Family     uint8
	HostBits   uint8
	Sampled    uint8
	Pad        [5]byte
}

// ScanTarget matches #[repr(C)] ThroneFastScanTarget (24 bytes).
type ScanTarget struct {
	Addr   [16]byte
	Port   uint16
	Family uint8
	Pad    [5]byte
}

// XdnsResult matches #[repr(C)] ThroneFastXdnsResult (4 bytes).
type XdnsResult struct {
	Matched uint8
	RCode   uint8
	HasTXT  uint8
	Pad     uint8
}

type apiProcs struct {
	ready           bool
	handle          windows.Handle
	abiVersion      uintptr
	xxhash64        uintptr
	scanCreate      uintptr
	scanFree        uintptr
	scanFillBatch   uintptr
	dnsMatchXdns    uintptr
	routeCreate     uintptr
	routeFree       uintptr
	routeInsertCIDR uintptr
	routeMatchV4    uintptr
}

var (
	loadOnce sync.Once
	api      apiProcs
)

func initFastCore() {
	candidates := make([]string, 0, 4)
	if envPath := os.Getenv("THRONE_FAST_CORE_DLL"); envPath != "" {
		candidates = append(candidates, envPath)
	}
	if exe, err := os.Executable(); err == nil {
		candidates = append(candidates, filepath.Join(filepath.Dir(exe), "throne_fast_core.dll"))
	}
	candidates = append(candidates, "throne_fast_core.dll")

	var handle windows.Handle
	for _, path := range candidates {
		if path == "" {
			continue
		}
		h, err := windows.LoadLibrary(path)
		if err == nil && h != 0 {
			handle = h
			break
		}
	}
	if handle == 0 {
		return
	}

	resolve := func(name string) (uintptr, bool) {
		addr, err := windows.GetProcAddress(handle, name)
		return addr, err == nil && addr != 0
	}

	var ok bool
	if api.abiVersion, ok = resolve("throne_fast_abi_version"); !ok {
		return
	}
	ver, _, _ := syscall.SyscallN(api.abiVersion)
	if uint32(ver) != ExpectedABIVersion {
		return
	}

	if api.xxhash64, ok = resolve("throne_fast_xxhash64"); !ok {
		return
	}
	if api.scanCreate, ok = resolve("throne_fast_scan_create"); !ok {
		return
	}
	if api.scanFree, ok = resolve("throne_fast_scan_free"); !ok {
		return
	}
	if api.scanFillBatch, ok = resolve("throne_fast_scan_fill_batch"); !ok {
		return
	}
	if api.dnsMatchXdns, ok = resolve("throne_fast_dns_match_xdns_reply"); !ok {
		return
	}
	api.routeCreate, _ = resolve("throne_fast_route_create")
	api.routeFree, _ = resolve("throne_fast_route_free")
	api.routeInsertCIDR, _ = resolve("throne_fast_route_insert_cidr")
	api.routeMatchV4, _ = resolve("throne_fast_route_match_v4")

	api.handle = handle
	api.ready = true
}

func Available() bool {
	loadOnce.Do(initFastCore)
	return api.ready
}

func XXHash64(data []byte, seed uint64) (uint64, bool) {
	if !Available() {
		return 0, false
	}
	if len(data) == 0 {
		r, _, _ := syscall.SyscallN(api.xxhash64, 0, 0, uintptr(seed))
		return uint64(r), true
	}
	r, _, _ := syscall.SyscallN(
		api.xxhash64,
		uintptr(unsafe.Pointer(&data[0])),
		uintptr(len(data)),
		uintptr(seed),
	)
	return uint64(r), true
}

func MatchXDNSReply(pkt []byte, expectedID uint16, expectedName []byte) (XdnsResult, bool) {
	var res XdnsResult
	if !Available() || len(pkt) == 0 || len(expectedName) == 0 {
		return res, false
	}
	rc, _, _ := syscall.SyscallN(
		api.dnsMatchXdns,
		uintptr(unsafe.Pointer(&pkt[0])),
		uintptr(len(pkt)),
		uintptr(expectedID),
		uintptr(unsafe.Pointer(&expectedName[0])),
		uintptr(len(expectedName)),
		uintptr(unsafe.Pointer(&res)),
	)
	if int32(rc) != 0 {
		return res, false
	}
	return res, true
}

type ScanPlan struct {
	ptr uintptr
}

func NewScanPlan(descs []ScanRangeDesc, ports []uint16, total uint64, shuffle bool, seed uint64) *ScanPlan {
	if !Available() || len(descs) == 0 || len(ports) == 0 || total == 0 {
		return nil
	}
	var shuf uintptr
	if shuffle {
		shuf = 1
	}
	ptr, _, _ := syscall.SyscallN(
		api.scanCreate,
		uintptr(unsafe.Pointer(&descs[0])),
		uintptr(len(descs)),
		uintptr(unsafe.Pointer(&ports[0])),
		uintptr(len(ports)),
		uintptr(total),
		shuf,
		uintptr(seed),
	)
	if ptr == 0 {
		return nil
	}
	return &ScanPlan{ptr: ptr}
}

func (p *ScanPlan) Close() {
	if p != nil && p.ptr != 0 && Available() {
		syscall.SyscallN(api.scanFree, p.ptr)
		p.ptr = 0
	}
}

func (p *ScanPlan) FillBatch(startPos uint64, dst []ScanTarget) int {
	if p == nil || p.ptr == 0 || len(dst) == 0 || !Available() {
		return 0
	}
	n, _, _ := syscall.SyscallN(
		api.scanFillBatch,
		p.ptr,
		uintptr(startPos),
		uintptr(len(dst)),
		uintptr(unsafe.Pointer(&dst[0])),
	)
	return int(n)
}
