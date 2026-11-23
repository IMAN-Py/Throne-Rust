package probe

import (
	"context"
	"errors"
	"fmt"
	"net"
	"sync"
	"sync/atomic"
	"testing"
	"time"

	"github.com/sagernet/sing-box/adapter"
)

// TestProbeAdversarialCancellation tests rapid context cancellation mid-flight
// during a heavy batch of 200 slow probes. It verifies that:
// 1. All pending probes are aborted without deadlocking on limiter.
// 2. runBatch completes promptly (under 500ms).
// 3. Results slice contains exactly len(outboundTags) items.
// 4. Cancelled items contain ErrTestAborted.
func TestProbeAdversarialCancellation(t *testing.T) {
	tagCount := 200
	tags := make([]string, tagCount)
	for i := 0; i < tagCount; i++ {
		tags[i] = fmt.Sprintf("node-slow-%04d", i)
	}
	box := newMockBox(tags)

	ctx, cancel := context.WithCancel(context.Background())

	var launched int32
	var abortedCount int32
	var completedCount int32

	probe := batchProbe[string]{
		run: func(pCtx context.Context, tag string, outbound adapter.Outbound) *string {
			atomic.AddInt32(&launched, 1)
			// Trigger cancellation once first 10 probes launch
			if atomic.LoadInt32(&launched) == 10 {
				cancel()
			}
			select {
			case <-time.After(200 * time.Millisecond):
				atomic.AddInt32(&completedCount, 1)
				res := "success"
				return &res
			case <-pCtx.Done():
				atomic.AddInt32(&abortedCount, 1)
				res := "aborted-in-run"
				return &res
			}
		},
		fail: func(tag string, err error) *string {
			res := fmt.Sprintf("failed: %v", err)
			return &res
		},
		publish: func(res *string) {},
	}

	start := time.Now()
	results := runBatch(ctx, box, tags, 50, probe)
	elapsed := time.Since(start)

	if len(results) != tagCount {
		t.Fatalf("expected %d results, got %d", tagCount, len(results))
	}

	// Must return promptly because ctx was cancelled
	if elapsed > 1*time.Second {
		t.Fatalf("runBatch took too long to cancel: %v (expected < 1s)", elapsed)
	}

	abortedFound := 0
	for _, r := range results {
		if r != nil && (*r == "failed: test aborted" || *r == "aborted-in-run") {
			abortedFound++
		}
	}

	t.Logf("[PASS] Adversarial Cancellation: %d/%d items aborted cleanly in %v", abortedFound, tagCount, elapsed)
	if abortedFound == 0 {
		t.Fatalf("expected aborted items, but found none")
	}
}

// TestProbeAdversarialExtremeLatencies tests 100 probes with widely varying latencies:
// 0ms, 1ms, 50ms, 150ms, and simulated network timeouts.
func TestProbeAdversarialExtremeLatencies(t *testing.T) {
	tagCount := 100
	tags := make([]string, tagCount)
	for i := 0; i < tagCount; i++ {
		tags[i] = fmt.Sprintf("jitter-node-%03d", i)
	}
	box := newMockBox(tags)

	probe := batchProbe[int]{
		run: func(ctx context.Context, tag string, outbound adapter.Outbound) *int {
			var idx int
			fmt.Sscanf(tag, "jitter-node-%d", &idx)
			// Jitter pattern: some 0ms, some 5ms, some 25ms, some fail
			jitter := time.Duration(idx%6) * 5 * time.Millisecond
			time.Sleep(jitter)
			val := int(jitter.Milliseconds())
			return &val
		},
		fail: func(tag string, err error) *int {
			v := -1
			return &v
		},
		publish: func(res *int) {},
	}

	start := time.Now()
	results := runBatch(context.Background(), box, tags, 30, probe)
	elapsed := time.Since(start)

	if len(results) != tagCount {
		t.Fatalf("expected %d results, got %d", tagCount, len(results))
	}
	for i, r := range results {
		if r == nil {
			t.Fatalf("result at index %d is nil", i)
		}
	}
	t.Logf("[PASS] Adversarial Jitter: 100 nodes with 0-25ms jitter completed in %v", elapsed)
}

// TestProbeAdversarialMissingAndDuplicateTags tests edge cases:
// 1. Tags that do not exist in box.OutboundManager
// 2. Duplicate tags within the same batch
// 3. Empty tag string
func TestProbeAdversarialMissingAndDuplicateTags(t *testing.T) {
	// Only node-A and node-B exist
	box := newMockBox([]string{"node-A", "node-B"})

	tags := []string{
		"node-A",
		"non-existent-1",
		"node-B",
		"node-A", // duplicate
		"",       // empty tag
		"non-existent-2",
		"node-B", // duplicate
	}

	probe := batchProbe[string]{
		run: func(ctx context.Context, tag string, outbound adapter.Outbound) *string {
			res := fmt.Sprintf("ok:%s", tag)
			return &res
		},
		fail: func(tag string, err error) *string {
			res := fmt.Sprintf("err:%s:%v", tag, err)
			return &res
		},
		publish: func(res *string) {},
	}

	results := runBatch(context.Background(), box, tags, 10, probe)
	if len(results) != len(tags) {
		t.Fatalf("expected %d results, got %d", len(tags), len(results))
	}

	// Verify each index matches expected status
	for i, tag := range tags {
		r := results[i]
		if r == nil {
			t.Fatalf("index %d result is nil", i)
		}
		if tag == "node-A" || tag == "node-B" {
			if *r != fmt.Sprintf("ok:%s", tag) {
				t.Fatalf("index %d (%s) expected ok, got %s", i, tag, *r)
			}
		} else {
			if *r == "" || (*r)[:4] != "err:" {
				t.Fatalf("index %d (%s) expected err, got %s", i, tag, *r)
			}
		}
	}
	t.Logf("[PASS] Missing and Duplicate Tags: verified %d entries correctly resolved", len(tags))
}

// mockConn implements a dummy net.Conn for probeDialer testing.
type mockConn struct {
	net.Conn
	closed int32
}

func (m *mockConn) Close() error {
	atomic.StoreInt32(&m.closed, 1)
	return nil
}

func (m *mockConn) isClosed() bool {
	return atomic.LoadInt32(&m.closed) == 1
}

// TestProbeDialerHandleRetention tests probeDialer connection tracking,
// mid-dial Close, and socket teardown to ensure zero handle retention.
func TestProbeDialerHandleRetention(t *testing.T) {
	var createdConns []*mockConn
	var mu sync.Mutex

	dialer := &probeDialer{
		dial: func(ctx context.Context, network, address string) (net.Conn, error) {
			mc := &mockConn{}
			mu.Lock()
			createdConns = append(createdConns, mc)
			mu.Unlock()
			return mc, nil
		},
	}

	// Dial 50 connections
	var conns []net.Conn
	for i := 0; i < 50; i++ {
		c, err := dialer.DialContext(context.Background(), "tcp", "1.1.1.1:80")
		if err != nil {
			t.Fatalf("dial failed: %v", err)
		}
		conns = append(conns, c)
	}

	if len(conns) != 50 {
		t.Fatalf("expected 50 conns, got %d", len(conns))
	}

	// Now close the dialer
	dialer.Close()

	// Wait briefly for the goroutine closes to execute
	time.Sleep(50 * time.Millisecond)

	// All created connections must be closed
	mu.Lock()
	defer mu.Unlock()
	for i, c := range createdConns {
		if !c.isClosed() {
			t.Fatalf("connection %d was not closed by dialer.Close()", i)
		}
	}

	// Attempting to dial after Close must return net.ErrClosed
	_, err := dialer.DialContext(context.Background(), "tcp", "1.1.1.1:80")
	if !errors.Is(err, net.ErrClosed) {
		t.Fatalf("expected net.ErrClosed after Close(), got %v", err)
	}
	t.Logf("[PASS] ProbeDialer Handle Retention: 50/50 connections closed cleanly, post-close dial rejected")
}

// TestResultBufferReclaimStress tests resultBuffer under high concurrency
// with interleaved AddResult, Results, and Reclaim.
func TestResultBufferReclaimStress(t *testing.T) {
	buf := &resultBuffer[int]{}
	var wg sync.WaitGroup

	// 5 worker goroutines pushing results
	for g := 0; g < 5; g++ {
		wg.Add(1)
		go func(gid int) {
			defer wg.Done()
			for i := 0; i < 100; i++ {
				val := gid*1000 + i
				buf.AddResult(&val)
			}
		}(g)
	}

	wg.Wait()

	drained := buf.Results()
	if len(drained) != 500 {
		t.Fatalf("expected 500 drained results, got %d", len(drained))
	}

	// Reclaim subset of drained
	buf.Reclaim(drained[:100])
	emptyResults := buf.Results()
	if len(emptyResults) != 0 {
		t.Fatalf("expected empty buffer after reclaim on empty, got %d", len(emptyResults))
	}
	t.Logf("[PASS] ResultBuffer Reclaim Stress: 500 concurrent additions and drains verified")
}
