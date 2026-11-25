//go:build !windows

package fastcore

const ExpectedABIVersion uint32 = 0x00020001

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

type ScanTarget struct {
	Addr   [16]byte
	Port   uint16
	Family uint8
	Pad    [5]byte
}

type XdnsResult struct {
	Matched uint8
	RCode   uint8
	HasTXT  uint8
	Pad     uint8
}

type ScanPlan struct{}

func Available() bool { return false }

func XXHash64([]byte, uint64) (uint64, bool) { return 0, false }

func MatchXDNSReply([]byte, uint16, []byte) (XdnsResult, bool) { return XdnsResult{}, false }

func NewScanPlan([]ScanRangeDesc, []uint16, uint64, bool, uint64) *ScanPlan { return nil }

func (*ScanPlan) Close() {}

func (*ScanPlan) FillBatch(uint64, []ScanTarget) int { return 0 }
