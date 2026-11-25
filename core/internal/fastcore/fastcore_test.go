package fastcore

import (
	"testing"
	"unsafe"
)

func TestFFIStructLayouts(t *testing.T) {
	if got := unsafe.Sizeof(ScanRangeDesc{}); got != 64 {
		t.Fatalf("ScanRangeDesc size = %d, want 64", got)
	}
	if got := unsafe.Sizeof(ScanTarget{}); got != 24 {
		t.Fatalf("ScanTarget size = %d, want 24", got)
	}
	if got := unsafe.Sizeof(XdnsResult{}); got != 4 {
		t.Fatalf("XdnsResult size = %d, want 4", got)
	}
}

func TestFastCoreLoadedAndHashVectors(t *testing.T) {
	if !Available() {
		t.Fatalf("expected throne_fast_core.dll to load (set THRONE_FAST_CORE_DLL or place DLL beside test)")
	}
	h0, ok := XXHash64(nil, 0)
	if !ok || h0 != 0xEF46DB3751D8E999 {
		t.Fatalf("XXHash64(nil, 0) = (%x, %v), want (ef46db3751d8e999, true)", h0, ok)
	}
	h1, ok := XXHash64([]byte("abc"), 0)
	if !ok || h1 != 0x44BC2CF5AD770999 {
		t.Fatalf("XXHash64(abc, 0) = (%x, %v), want (44bc2cf5ad770999, true)", h1, ok)
	}
}
