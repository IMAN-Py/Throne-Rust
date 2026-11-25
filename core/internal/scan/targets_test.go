package scan

import (
	"testing"

	"ThroneCore/internal/fastcore"
)

func TestFastBatchDifferentialEquivalence(t *testing.T) {
	if !fastcore.Available() {
		t.Fatalf("throne_fast_core.dll not loaded; set THRONE_FAST_CORE_DLL for differential verification")
	}

	specs := []struct {
		name string
		spec Spec
	}{
		{
			name: "shuffled_mixed_v4_v6_multiport",
			spec: Spec{
				Entries: []Entry{
					{CIDR: "104.16.0.0/24", Port: 443},
					{CIDR: "162.159.192.0/28", Port: 0},
					{CIDR: "2606:4700::/32", Port: 443},
					{CIDR: "2a06:98c1::/64", Port: 8443},
					{CIDR: "invalid-cidr", Port: 443},
				},
				DefaultPorts:     []int32{443, 2053, 2083, 8443},
				PortMode:         PortModeMerge,
				MaxHostsPerEntry: 256,
				Shuffle:          true,
				Seed:             0x9e3779b97f4a7c15,
			},
		},
		{
			name: "unshuffled_exact",
			spec: Spec{
				Entries: []Entry{
					{CIDR: "1.1.1.0/29", Port: 443},
					{CIDR: "2606:4700:4700::1111/128", Port: 8443},
				},
				PortMode: PortModeIgnore,
				Shuffle:  false,
			},
		},
		{
			name: "sampled_1_host_from_slash_24",
			spec: Spec{
				Entries: []Entry{
					{CIDR: "172.64.0.0/16", Port: 443},
				},
				MaxHostsPerEntry: 17,
				Shuffle:          true,
				Seed:             123456789,
			},
		},
	}

	for _, tc := range specs {
		t.Run(tc.name, func(t *testing.T) {
			targets := NewTargets(tc.spec)
			fb := targets.NewFastBatch(128)
			defer fb.Close()
			if fb.plan == nil {
				t.Fatalf("expected non-nil FastBatch plan")
			}

			total := targets.Total()
			if total == 0 {
				t.Fatalf("expected non-zero targets")
			}

			const chunk = 137
			buf := make([]Target, chunk)
			for start := uint64(0); start < total; start += chunk {
				n := fb.FillBatch(start, buf)
				wantN := int(min(uint64(chunk), total-start))
				if n != wantN {
					t.Fatalf("start=%d: got n=%d, want %d", start, n, wantN)
				}
				for i := 0; i < n; i++ {
					want := targets.At(start + uint64(i))
					if buf[i] != want {
						t.Fatalf("mismatch at pos=%d: rust=(%v:%d) go=(%v:%d)",
							start+uint64(i), buf[i].Addr, buf[i].Port, want.Addr, want.Port)
					}
				}
			}

			if n := fb.FillBatch(total, buf); n != 0 {
				t.Fatalf("out-of-bounds FillBatch returned %d, want 0", n)
			}
		})
	}
}

func BenchmarkTargetsGoAt(b *testing.B) {
	spec := Spec{
		Entries: []Entry{
			{CIDR: "104.16.0.0/16", Port: 443},
			{CIDR: "172.64.0.0/16", Port: 443},
			{CIDR: "2606:4700::/32", Port: 443},
		},
		DefaultPorts:     []int32{443, 2053, 2083, 8443},
		PortMode:         PortModeMerge,
		MaxHostsPerEntry: 65536,
		Shuffle:          true,
		Seed:             0x42424242,
	}
	targets := NewTargets(spec)
	total := targets.Total()
	b.ReportAllocs()
	b.ResetTimer()
	var sum uint64
	for i := 0; i < b.N; i++ {
		t := targets.At(uint64(i) % total)
		sum += uint64(t.Port)
	}
	_ = sum
}

func BenchmarkTargetsRustBatch1024(b *testing.B) {
	if !fastcore.Available() {
		b.Skip("throne_fast_core.dll not loaded")
	}
	spec := Spec{
		Entries: []Entry{
			{CIDR: "104.16.0.0/16", Port: 443},
			{CIDR: "172.64.0.0/16", Port: 443},
			{CIDR: "2606:4700::/32", Port: 443},
		},
		DefaultPorts:     []int32{443, 2053, 2083, 8443},
		PortMode:         PortModeMerge,
		MaxHostsPerEntry: 65536,
		Shuffle:          true,
		Seed:             0x42424242,
	}
	targets := NewTargets(spec)
	fb := targets.NewFastBatch(1024)
	defer fb.Close()
	buf := make([]Target, 1024)
	total := targets.Total()
	b.ReportAllocs()
	b.ResetTimer()
	var sum uint64
	for i := 0; i < b.N; {
		remain := b.N - i
		if remain > 1024 {
			remain = 1024
		}
		start := uint64(i) % total
		if start+uint64(remain) > total {
			start = 0
		}
		n := fb.FillBatch(start, buf[:remain])
		for j := 0; j < n; j++ {
			sum += uint64(buf[j].Port)
		}
		i += n
	}
	_ = sum
}
