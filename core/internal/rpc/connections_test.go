package rpc

import (
	"os"
	"path/filepath"
	"strings"
	"sync/atomic"
	"testing"

	"github.com/gofrs/uuid/v5"
	"github.com/sagernet/sing-box/adapter"
	"github.com/sagernet/sing-box/common/trafficcontrol"
)

func TestConnMetaToProtoBasenameMatchesStringsSplit(t *testing.T) {
	paths := []string{
		"",
		"firefox.exe",
		filepath.Join("C:", "Program Files", "App", "app.exe"),
		"trailing" + string(os.PathSeparator),
		string(os.PathSeparator) + "usr" + string(os.PathSeparator) + "bin" + string(os.PathSeparator) + "curl",
	}

	for _, p := range paths {
		spl := strings.Split(p, string(os.PathSeparator))
		want := spl[len(spl)-1]

		meta := &trafficcontrol.TrackerMetadata{
			Upload:   new(atomic.Int64),
			Download: new(atomic.Int64),
			Metadata: adapter.InboundContext{
				ProcessInfo: &adapter.ConnectionOwner{
					ProcessPaths: []string{p},
				},
			},
		}
		gotProto := connMetaToProto(meta, "rule")
		if gotProto.GetProcess() != want || gotProto.GetProcessPath() != p {
			t.Fatalf("path=%q: got process=%q (path=%q), want %q", p, gotProto.GetProcess(), gotProto.GetProcessPath(), want)
		}
	}
}

func TestRuleMatchCachePingPongAndReset(t *testing.T) {
	var cache ruleMatchCache
	id1 := uuid.Must(uuid.NewV4())
	id2 := uuid.Must(uuid.NewV4())

	cache.reset(nil, nil)
	cache.begin(nil)
	cache.conns[id1] = "rule-1"
	cache.seen[id1] = "rule-1"
	cache.seen[id2] = "rule-2"
	cache.end()

	if len(cache.conns) != 2 || cache.active {
		t.Fatalf("expected swapped 2 entries and inactive, got len=%d active=%v", len(cache.conns), cache.active)
	}
	if len(cache.seen) != 0 {
		t.Fatalf("expected seen map cleared after end(), got len=%d", len(cache.seen))
	}

	// Second poll reusing same underlying map capacity without allocations
	cache.begin(nil)
	cache.seen[id1] = "rule-1"
	cache.end()

	if len(cache.conns) != 1 || cache.conns[id1] != "rule-1" {
		t.Fatalf("expected stale id2 pruned via ping-pong swap, got %+v", cache.conns)
	}
	if _, ok := cache.conns[id2]; ok {
		t.Fatalf("unexpected stale id2 retained")
	}
}

func BenchmarkConnMetaToProto(b *testing.B) {
	meta := &trafficcontrol.TrackerMetadata{
		Upload:   new(atomic.Int64),
		Download: new(atomic.Int64),
		Metadata: adapter.InboundContext{
			Network:  "tcp",
			Domain:   "example.com",
			Protocol: "tls",
			ProcessInfo: &adapter.ConnectionOwner{
				ProcessPaths: []string{`C:\Program Files\Mozilla Firefox\firefox.exe`},
			},
		},
	}
	b.ReportAllocs()
	b.ResetTimer()
	for i := 0; i < b.N; i++ {
		_ = connMetaToProto(meta, "domain_suffix:example.com")
	}
}
