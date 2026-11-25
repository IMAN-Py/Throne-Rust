package probe

import (
	"context"
	"fmt"
	"sync/atomic"
	"testing"
	"time"

	"github.com/sagernet/sing-box/adapter"
	"github.com/sagernet/sing/service"
)

type stubOutboundManager struct {
	adapter.OutboundManager
	known map[string]bool
}

func (m *stubOutboundManager) Outbound(tag string) (adapter.Outbound, bool) {
	return nil, m.known[tag]
}

type stubBox struct {
	ctx context.Context
	mgr adapter.OutboundManager
}

func (b *stubBox) Context() context.Context          { return b.ctx }
func (b *stubBox) Outbound() adapter.OutboundManager { return b.mgr }

type testResult struct {
	tag string
	ok  bool
	err error
}

func TestRunBatchConcurrencyAndNoArtificialSleep(t *testing.T) {
	known := make(map[string]bool, 150)
	tags := make([]string, 0, 150)
	for i := 0; i < 150; i++ {
		tag := fmt.Sprintf("node-%d", i)
		tags = append(tags, tag)
		if i != 149 {
			known[tag] = true
		}
	}
	mgr := &stubOutboundManager{known: known}
	ctx := service.ContextWith[adapter.OutboundManager](context.Background(), mgr)
	box := &stubBox{ctx: ctx, mgr: mgr}

	var published atomic.Int32
	start := time.Now()
	results := runBatch(context.Background(), box, tags, 32, batchProbe[testResult]{
		run: func(ctx context.Context, tag string, _ adapter.Outbound) *testResult {
			return &testResult{tag: tag, ok: true}
		},
		fail: func(tag string, err error) *testResult {
			return &testResult{tag: tag, ok: false, err: err}
		},
		publish: func(*testResult) {
			published.Add(1)
		},
	})
	elapsed := time.Since(start)

	if len(results) != len(tags) {
		t.Fatalf("got %d results, want %d", len(results), len(tags))
	}
	// Previously 150 * 2ms = 300ms minimum artificial sleep; now should finish in tens of ms.
	if elapsed > 200*time.Millisecond {
		t.Fatalf("batch took %v, expected no per-tag sleep stall", elapsed)
	}
	if results[149].ok || results[149].err == nil {
		t.Fatalf("missing tag expected failure result, got %+v", results[149])
	}
	for i := 0; i < 149; i++ {
		if !results[i].ok || results[i].tag != tags[i] {
			t.Fatalf("idx %d mismatch: %+v", i, results[i])
		}
	}
	if published.Load() != int32(len(tags)) {
		t.Fatalf("published=%d, want %d", published.Load(), len(tags))
	}
}

func TestRunBatchCancellationUnblocksLimiter(t *testing.T) {
	known := map[string]bool{"a": true, "b": true, "c": true, "d": true}
	mgr := &stubOutboundManager{known: known}
	boxCtx := service.ContextWith[adapter.OutboundManager](context.Background(), mgr)
	box := &stubBox{ctx: boxCtx, mgr: mgr}

	ctx, cancel := context.WithCancel(context.Background())
	started := make(chan struct{})
	release := make(chan struct{})

	var results []*testResult
	done := make(chan struct{})
	go func() {
		results = runBatch(ctx, box, []string{"a", "b", "c", "d"}, 1, batchProbe[testResult]{
			run: func(rctx context.Context, tag string, _ adapter.Outbound) *testResult {
				select {
				case started <- struct{}{}:
				default:
				}
				select {
				case <-release:
				case <-rctx.Done():
					return &testResult{tag: tag, err: rctx.Err()}
				}
				return &testResult{tag: tag, ok: true}
			},
			fail: func(tag string, err error) *testResult {
				return &testResult{tag: tag, err: err}
			},
			publish: func(*testResult) {},
		})
		close(done)
	}()

	<-started
	cancel()
	close(release)

	select {
	case <-done:
	case <-time.After(2 * time.Second):
		t.Fatal("runBatch hung after ctx cancel with full semaphore")
	}

	if len(results) != 4 {
		t.Fatalf("got %d results, want 4", len(results))
	}
}
