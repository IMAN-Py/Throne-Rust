package scan

import (
	"net/netip"

	"ThroneCore/internal/fastcore"
)

// FastBatch accelerates batch target generation through throne_fast_core.dll
// and falls back transparently to pure-Go Targets.At when unavailable.
type FastBatch struct {
	targets *Targets
	plan    *fastcore.ScanPlan
	raw     []fastcore.ScanTarget
}

func (t *Targets) NewFastBatch(batchCap int) *FastBatch {
	fb := &FastBatch{targets: t}
	if t == nil || t.total == 0 || len(t.ranges) == 0 || !fastcore.Available() {
		return fb
	}
	if batchCap <= 0 {
		batchCap = 256
	}

	totalPorts := 0
	for i := range t.ranges {
		totalPorts += len(t.ranges[i].ports)
	}
	if totalPorts == 0 {
		return fb
	}

	descs := make([]fastcore.ScanRangeDesc, len(t.ranges))
	ports := make([]uint16, 0, totalPorts)

	for i := range t.ranges {
		r := &t.ranges[i]
		end := t.total
		if i+1 < len(t.ranges) {
			end = t.ranges[i+1].start
		}
		var base [16]byte
		var family uint8 = 6
		if r.base.Is4() {
			family = 4
			v4 := r.base.As4()
			copy(base[:4], v4[:])
		} else {
			base = r.base.As16()
		}
		var sampled uint8
		if r.sampled {
			sampled = 1
		}
		portOffset := uint32(len(ports))
		ports = append(ports, r.ports...)

		descs[i] = fastcore.ScanRangeDesc{
			Base:       base,
			Start:      r.start,
			End:        end,
			HighKey:    r.highKey,
			SamplerKey: r.samplerKey,
			PortOffset: portOffset,
			PortCount:  uint32(len(r.ports)),
			Family:     family,
			HostBits:   uint8(r.hostBits),
			Sampled:    sampled,
		}
	}

	fb.plan = fastcore.NewScanPlan(descs, ports, t.total, t.shuffle, t.seed)
	if fb.plan != nil {
		fb.raw = make([]fastcore.ScanTarget, batchCap)
	}
	return fb
}

func (f *FastBatch) Close() {
	if f != nil && f.plan != nil {
		f.plan.Close()
		f.plan = nil
	}
}

func (f *FastBatch) FillBatch(start uint64, dst []Target) int {
	if f == nil || f.targets == nil || start >= f.targets.total || len(dst) == 0 {
		return 0
	}
	avail := f.targets.total - start
	want := len(dst)
	if uint64(want) > avail {
		want = int(avail)
	}
	if f.plan == nil {
		for i := 0; i < want; i++ {
			dst[i] = f.targets.At(start + uint64(i))
		}
		return want
	}
	if cap(f.raw) < want {
		f.raw = make([]fastcore.ScanTarget, want)
	}
	buf := f.raw[:want]
	n := f.plan.FillBatch(start, buf)
	if n != want {
		for i := 0; i < want; i++ {
			dst[i] = f.targets.At(start + uint64(i))
		}
		return want
	}
	for i := 0; i < n; i++ {
		rt := &buf[i]
		var addr netip.Addr
		if rt.Family == 4 {
			addr = netip.AddrFrom4([4]byte(rt.Addr[:4]))
		} else {
			addr = netip.AddrFrom16(rt.Addr)
		}
		dst[i] = Target{Addr: addr, Port: rt.Port}
	}
	return n
}
