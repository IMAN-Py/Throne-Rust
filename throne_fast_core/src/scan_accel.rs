use core::slice;

const MIXER_ROUNDS: usize = 4;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ThroneFastScanRangeDesc {
    pub base: [u8; 16], // v4 stored in base[0..4], v6 stored in base[0..16] (big endian)
    pub start: u64,
    pub end: u64,
    pub high_key: u64,
    pub sampler_key: u64,
    pub port_offset: u32,
    pub port_count: u32,
    pub family: u8,    // 4 or 6
    pub host_bits: u8, // 0..=128
    pub sampled: u8,   // 0 or 1
    pub _pad: [u8; 5],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThroneFastScanTarget {
    pub addr: [u8; 16], // v4 in addr[0..4], v6 in addr[0..16]
    pub port: u16,
    pub family: u8, // 4 or 6
    pub _pad: [u8; 5],
}

#[inline(always)]
fn bit_mask(width: usize) -> u64 {
    if width == 0 {
        0
    } else if width >= 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    }
}

#[inline(always)]
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[derive(Clone, Copy, Debug)]
struct Mixer {
    add: [u64; MIXER_ROUNDS],
    mul: [u64; MIXER_ROUNDS],
    mask: u64,
    shift: u32,
}

impl Mixer {
    fn new(width: usize, key: u64) -> Self {
        let mask = bit_mask(width);
        let shift = (width / 2).max(1) as u32;
        let mut state = key;
        let mut add = [0u64; MIXER_ROUNDS];
        let mut mul = [0u64; MIXER_ROUNDS];
        for r in 0..MIXER_ROUNDS {
            add[r] = splitmix64(&mut state) & mask;
            mul[r] = (splitmix64(&mut state) | 1) & mask;
        }
        Self {
            add,
            mul,
            mask,
            shift,
        }
    }

    #[inline(always)]
    fn mix(&self, mut x: u64) -> u64 {
        let m = self.mask;
        let s = self.shift;
        // Unrolled 4 ARX+mul rounds matching core/internal/scan/targets.go bit-for-bit.
        x = x.wrapping_add(self.add[0]) & m;
        x ^= x >> s;
        x = x.wrapping_mul(self.mul[0]) & m;

        x = x.wrapping_add(self.add[1]) & m;
        x ^= x >> s;
        x = x.wrapping_mul(self.mul[1]) & m;

        x = x.wrapping_add(self.add[2]) & m;
        x ^= x >> s;
        x = x.wrapping_mul(self.mul[2]) & m;

        x = x.wrapping_add(self.add[3]) & m;
        x ^= x >> s;
        x = x.wrapping_mul(self.mul[3]) & m;
        x
    }
}

#[derive(Clone, Debug)]
struct CompiledRange {
    base_v4: u32,
    base_v6: u128,
    start: u64,
    end: u64,
    high_key: u64,
    port_offset: usize,
    port_count: u64,
    sampler: Mixer,
    family: u8,
    host_bits: u8,
    sampled: bool,
}

pub struct ThroneFastScanPlan {
    ranges: Vec<CompiledRange>,
    ports: Vec<u16>,
    total: u64,
    shuffle: bool,
    order: Mixer,
}

impl ThroneFastScanPlan {
    pub fn build(
        descs: &[ThroneFastScanRangeDesc],
        ports: &[u16],
        total: u64,
        shuffle: bool,
        seed: u64,
    ) -> Option<Self> {
        let mut compiled = Vec::with_capacity(descs.len());
        let mut expected_start = 0u64;
        for d in descs {
            if d.port_count == 0 {
                return None;
            }
            let port_end = (d.port_offset as usize).checked_add(d.port_count as usize)?;
            if port_end > ports.len() {
                return None;
            }
            if d.family != 4 && d.family != 6 {
                return None;
            }
            if d.start != expected_start || d.end < d.start || d.end > total {
                return None;
            }
            expected_start = d.end;

            let base_v4 = u32::from_be_bytes([d.base[0], d.base[1], d.base[2], d.base[3]]);
            let base_v6 = u128::from_be_bytes(d.base);
            let sampler = if d.sampled != 0 {
                Mixer::new((d.host_bits as usize).min(64), d.sampler_key)
            } else {
                Mixer {
                    add: [0; MIXER_ROUNDS],
                    mul: [0; MIXER_ROUNDS],
                    mask: 0,
                    shift: 1,
                }
            };
            compiled.push(CompiledRange {
                base_v4,
                base_v6,
                start: d.start,
                end: d.end,
                high_key: d.high_key,
                port_offset: d.port_offset as usize,
                port_count: d.port_count as u64,
                sampler,
                family: d.family,
                host_bits: d.host_bits,
                sampled: d.sampled != 0,
            });
        }
        if expected_start != total {
            return None;
        }
        let order = if shuffle && total > 1 {
            let bits = (64 - (total - 1).leading_zeros()) as usize;
            Mixer::new(bits, seed)
        } else {
            Mixer {
                add: [0; MIXER_ROUNDS],
                mul: [0; MIXER_ROUNDS],
                mask: 0,
                shift: 1,
            }
        };
        Some(Self {
            ranges: compiled,
            ports: ports.to_vec(),
            total,
            shuffle,
            order,
        })
    }

    #[inline(always)]
    fn permute(&self, position: u64) -> u64 {
        if self.total <= 1 {
            return position;
        }
        let mut y = self.order.mix(position);
        while y >= self.total {
            y = self.order.mix(y);
        }
        y
    }

    #[inline(always)]
    fn range_for_index(&self, index: u64) -> usize {
        if self.ranges.len() <= 8 {
            for (i, r) in self.ranges.iter().enumerate() {
                if index < r.end {
                    return i;
                }
            }
            return self.ranges.len() - 1;
        }
        self.ranges.partition_point(|r| r.end <= index)
    }

    #[inline(always)]
    fn target_at_with_hint(&self, position: u64, range_hint: &mut usize) -> ThroneFastScanTarget {
        let index = if self.shuffle {
            self.permute(position)
        } else {
            position
        };

        let mut ri = *range_hint;
        if !self.shuffle && ri < self.ranges.len() {
            while index >= self.ranges[ri].end {
                ri += 1;
            }
            if index < self.ranges[ri].start {
                ri = self.range_for_index(index);
            }
        } else {
            ri = self.range_for_index(index);
        }
        *range_hint = ri;

        let r = &self.ranges[ri];
        let local = index - r.start;
        let (host_index, port_index) = if r.port_count == 1 {
            (local, 0usize)
        } else {
            (local / r.port_count, (local % r.port_count) as usize)
        };

        let (low, high) = if r.sampled {
            let l = r.sampler.mix(host_index);
            let h = if r.host_bits > 64 {
                let mut state = host_index ^ r.high_key;
                splitmix64(&mut state) & bit_mask((r.host_bits - 64) as usize)
            } else {
                0
            };
            (l, h)
        } else {
            (host_index, 0)
        };

        let mut addr = [0u8; 16];
        if r.family == 4 {
            let v = r.base_v4.wrapping_add(low as u32);
            addr[..4].copy_from_slice(&v.to_be_bytes());
        } else {
            let offset = ((high as u128) << 64) | (low as u128);
            let v = r.base_v6.wrapping_add(offset);
            addr.copy_from_slice(&v.to_be_bytes());
        }

        ThroneFastScanTarget {
            addr,
            port: self.ports[r.port_offset + port_index],
            family: r.family,
            _pad: [0; 5],
        }
    }

    pub fn fill_batch(&self, start_pos: u64, out: &mut [ThroneFastScanTarget]) -> usize {
        if start_pos >= self.total || self.ranges.is_empty() || out.is_empty() {
            return 0;
        }
        let avail = (self.total - start_pos) as usize;
        let count = out.len().min(avail);
        let mut range_hint = if !self.shuffle {
            self.range_for_index(start_pos)
        } else {
            0
        };
        for i in 0..count {
            out[i] = self.target_at_with_hint(start_pos + i as u64, &mut range_hint);
        }
        count
    }
}

/// # Safety
/// `descs` and `ports` pointers must be valid for their declared counts (or null when zero).
#[no_mangle]
pub unsafe extern "C" fn throne_fast_scan_create(
    descs: *const ThroneFastScanRangeDesc,
    desc_count: usize,
    ports: *const u16,
    port_count: usize,
    total: u64,
    shuffle: u8,
    seed: u64,
) -> *mut ThroneFastScanPlan {
    if (desc_count > 0 && descs.is_null()) || (port_count > 0 && ports.is_null()) {
        return core::ptr::null_mut();
    }
    let desc_slice = if desc_count == 0 {
        &[]
    } else {
        unsafe { slice::from_raw_parts(descs, desc_count) }
    };
    let port_slice = if port_count == 0 {
        &[]
    } else {
        unsafe { slice::from_raw_parts(ports, port_count) }
    };
    match ThroneFastScanPlan::build(desc_slice, port_slice, total, shuffle != 0, seed) {
        Some(plan) => Box::into_raw(Box::new(plan)),
        None => core::ptr::null_mut(),
    }
}

/// # Safety
/// `plan` must have been allocated by `throne_fast_scan_create` or be null.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_scan_free(plan: *mut ThroneFastScanPlan) {
    if !plan.is_null() {
        unsafe { drop(Box::from_raw(plan)) };
    }
}

/// # Safety
/// `plan` must be valid; `out_targets` writable for `count` items.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_scan_fill_batch(
    plan: *const ThroneFastScanPlan,
    start_pos: u64,
    count: usize,
    out_targets: *mut ThroneFastScanTarget,
) -> usize {
    if plan.is_null() || out_targets.is_null() || count == 0 {
        return 0;
    }
    let p = unsafe { &*plan };
    let dst = unsafe { slice::from_raw_parts_mut(out_targets, count) };
    p.fill_batch(start_pos, dst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sequential_and_shuffled_coverage() {
        let descs = [
            ThroneFastScanRangeDesc {
                base: [104, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                start: 0,
                end: 16,
                high_key: 0,
                sampler_key: 0x1111,
                port_offset: 0,
                port_count: 2,
                family: 4,
                host_bits: 3,
                sampled: 0,
                _pad: [0; 5],
            },
            ThroneFastScanRangeDesc {
                base: [0x26, 0x06, 0x47, 0x00, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
                start: 16,
                end: 20,
                high_key: 0x2222,
                sampler_key: 0x3333,
                port_offset: 2,
                port_count: 1,
                family: 6,
                host_bits: 96,
                sampled: 1,
                _pad: [0; 5],
            },
        ];
        let ports = [443u16, 8443u16, 2053u16];

        let plan = ThroneFastScanPlan::build(&descs, &ports, 20, true, 0xdeadbeef).unwrap();
        let mut out = vec![
            ThroneFastScanTarget {
                addr: [0; 16],
                port: 0,
                family: 0,
                _pad: [0; 5],
            };
            20
        ];
        let n = plan.fill_batch(0, &mut out);
        assert_eq!(n, 20);

        // Shuffled bijection over small set: verify all 16 IPv4 targets (8 hosts * 2 ports) appear uniquely!
        let mut seen_v4 = std::collections::HashSet::new();
        let mut v6_count = 0usize;
        for t in &out {
            if t.family == 4 {
                assert!(t.addr[0] == 104 && t.addr[1] == 16 && t.addr[2] == 0 && t.addr[3] < 8);
                assert!(t.port == 443 || t.port == 8443);
                assert!(seen_v4.insert((t.addr[3], t.port)));
            } else {
                assert_eq!(t.family, 6);
                assert_eq!(t.port, 2053);
                v6_count += 1;
            }
        }
        assert_eq!(seen_v4.len(), 16);
        assert_eq!(v6_count, 4);
    }

    #[test]
    fn batch_out_of_bounds_returns_exact_remaining() {
        let descs = [ThroneFastScanRangeDesc {
            base: [1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            start: 0,
            end: 4,
            high_key: 0,
            sampler_key: 0,
            port_offset: 0,
            port_count: 1,
            family: 4,
            host_bits: 2,
            sampled: 0,
            _pad: [0; 5],
        }];
        let plan = ThroneFastScanPlan::build(&descs, &[443], 4, false, 0).unwrap();
        let mut out = [ThroneFastScanTarget {
            addr: [0; 16],
            port: 0,
            family: 0,
            _pad: [0; 5],
        }; 10];
        assert_eq!(plan.fill_batch(2, &mut out), 2);
        assert_eq!(out[0].addr[3], 2);
        assert_eq!(out[1].addr[3], 3);
        assert_eq!(plan.fill_batch(4, &mut out), 0);
    }
}
