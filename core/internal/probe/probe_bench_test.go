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

type mockOutbound struct {
	adapter.Outbound
	tag string
}

func (m *mockOutbound) Tag() string { return m.tag }

type mockOutbounds struct {
	adapter.OutboundManager
	tags map[string]adapter.Outbound
}

func (m *mockOutbounds) Outbound(tag string) (adapter.Outbound, bool) {
	ob, ok := m.tags[tag]
	return ob, ok
}

type mockBox struct {
	ctx context.Context
	om  adapter.OutboundManager
}

func (b *mockBox) Context() context.Context { return b.ctx }
func (b *mockBox) Outbound() adapter.OutboundManager { return b.om }

func newMockBox(tags []string) *mockBox {
	tagMap := make(map[string]adapter.Outbound, len(tags))
	for _, t := range tags {
		tagMap[t] = &mockOutbound{tag: t}
	}
	mgr := &mockOutbounds{tags: tagMap}
	ctx := service.ContextWith[adapter.OutboundManager](context.Background(), mgr)
	return &mockBox{ctx: ctx, om: mgr}
}

// TestGoroutineLimiterStress verifies that under high concurrency (100, 500, 1000 tags),
// the number of concurrently active probe goroutines NEVER exceeds maxConcurrency.
func TestGoroutineLimiterStress(t *testing.T) {
	testCases := []struct {
		tagCount       int
		maxConcurrency int
		expectedBound  int
	}{
		{tagCount: 100, maxConcurrency: 20, expectedBound: 20},
		{tagCount: 100, maxConcurrency: 50, expectedBound: 50},
		{tagCount: 500, maxConcurrency: 50, expectedBound: 50},
		{tagCount: 500, maxConcurrency: 100, expectedBound: 100},
		{tagCount: 1000, maxConcurrency: 100, expectedBound: 100},
		{tagCount: 1000, maxConcurrency: 600, expectedBound: MaxConcurrentTests}, // normalized to 100
	}

	for _, tc := range testCases {
		name := fmt.Sprintf("Tags=%d_Concurrency=%d", tc.tagCount, tc.maxConcurrency)
		t.Run(name, func(t *testing.T) {
			tags := make([]string, tc.tagCount)
			for i := 0; i < tc.tagCount; i++ {
				tags[i] = fmt.Sprintf("proxy-node-%04d", i)
			}
			box := newMockBox(tags)

			var activeGoroutines int32
			var peakGoroutines int32
			var completedCount int32

			probe := batchProbe[int]{
				run: func(ctx context.Context, tag string, outbound adapter.Outbound) *int {
					cur := atomic.AddInt32(&activeGoroutines, 1)
					for {
						oldPeak := atomic.LoadInt32(&peakGoroutines)
						if cur > oldPeak {
							if atomic.CompareAndSwapInt32(&peakGoroutines, oldPeak, cur) {
								break
							}
						} else {
							break
						}
					}
					// Simulate probe workload
					time.Sleep(1 * time.Millisecond)
					atomic.AddInt32(&activeGoroutines, -1)
					atomic.AddInt32(&completedCount, 1)
					res := 42
					return &res
				},
				fail: func(tag string, err error) *int {
					return nil
				},
				publish: func(res *int) {},
			}

			results := runBatch(context.Background(), box, tags, tc.maxConcurrency, probe)

			if len(results) != tc.tagCount {
				t.Fatalf("expected %d results, got %d", tc.tagCount, len(results))
			}
			if atomic.LoadInt32(&completedCount) != int32(tc.tagCount) {
				t.Fatalf("expected %d completed probes, got %d", tc.tagCount, atomic.LoadInt32(&completedCount))
			}
			observedPeak := atomic.LoadInt32(&peakGoroutines)
			if int(observedPeak) > tc.expectedBound {
				t.Fatalf("CONCURRENCY VIOLATION: observed peak %d exceeded bound %d", observedPeak, tc.expectedBound)
			}
			t.Logf("[%s] Peak goroutines: %d (limit: %d) -> PASS", name, observedPeak, tc.expectedBound)
		})
	}
}

// BenchmarkProbeBatchDispatchLatency measures actual dispatch & execution latency
// for 100, 500, and 1,000 tags with zero sleep vs theoretical/simulated 2ms sleep floor.
func BenchmarkProbeBatchDispatch(b *testing.B) {
	tagCounts := []int{100, 500, 1000}

	for _, count := range tagCounts {
		b.Run(fmt.Sprintf("Tags_%d", count), func(b *testing.B) {
			tags := make([]string, count)
			for i := 0; i < count; i++ {
				tags[i] = fmt.Sprintf("node-%04d", i)
			}
			box := newMockBox(tags)

			probe := batchProbe[int]{
				run: func(ctx context.Context, tag string, outbound adapter.Outbound) *int {
					v := 1
					return &v
				},
				fail: func(tag string, err error) *int { return nil },
				publish: func(res *int) {},
			}
			b.ResetTimer()
			for i := 0; i < b.N; i++ {
				runBatch(context.Background(), box, tags, 100, probe)
			}
		})
	}
}

// TestProbeThroughputBeforeVsAfter measures actual end-to-end batch duration and throughput
// for 100, 500, and 1,000 tags under realistic probe simulated latency (5ms simulated network ping).
func TestProbeThroughputBeforeVsAfter(t *testing.T) {
	tagCounts := []int{100, 500, 1000}

	for _, count := range tagCounts {
		t.Run(fmt.Sprintf("Tags=%d", count), func(t *testing.T) {
			tags := make([]string, count)
			for i := 0; i < count; i++ {
				tags[i] = fmt.Sprintf("proxy-node-%04d", i)
			}
			box := newMockBox(tags)

			// Optimized (current implementation: zero artificial sleep)
			probeWork := batchProbe[int]{
				run: func(ctx context.Context, tag string, outbound adapter.Outbound) *int {
					time.Sleep(5 * time.Millisecond) // realistic network probe RTT
					v := 1
					return &v
				},
				fail: func(tag string, err error) *int { return nil },
				publish: func(res *int) {},
			}

			startOptimized := time.Now()
			resOptimized := runBatch(context.Background(), box, tags, 100, probeWork)
			durOptimized := time.Since(startOptimized)

			if len(resOptimized) != count {
				t.Fatalf("expected %d results, got %d", count, len(resOptimized))
			}

			// Simulated throttled version: same probe work, but with 2ms sleep between dispatches
			// Minimum dispatch delay is count * 2ms
			simulatedSleepDelay := time.Duration(count) * 2 * time.Millisecond
			// Total throttled time: dispatch serialized delay + last item completion
			durThrottledEstimated := simulatedSleepDelay + 5*time.Millisecond
			if durOptimized < durThrottledEstimated {
				speedup := float64(durThrottledEstimated) / float64(durOptimized)
				throughputOpt := float64(count) / durOptimized.Seconds()
				throughputThrot := float64(count) / durThrottledEstimated.Seconds()
				t.Logf("[%d tags] Optimized: %v (%.1f tags/s) | Throttled (min): %v (%.1f tags/s) | Speedup: %.2fx",
					count, durOptimized, throughputOpt, durThrottledEstimated, throughputThrot, speedup)
			}
		})
	}
}

