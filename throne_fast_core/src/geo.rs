use core::slice;

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThroneFastGeoRange {
    pub start: u32,
    pub end: u32,
    pub country: [u8; 2],
    pub _pad: [u8; 2],
}

pub struct ThroneFastGeoIndex {
    ranges: Vec<ThroneFastGeoRange>,
}

impl ThroneFastGeoIndex {
    pub fn from_ranges(input: &[ThroneFastGeoRange]) -> Self {
        let mut sorted: Vec<ThroneFastGeoRange> =
            input.iter().copied().filter(|r| r.start <= r.end).collect();
        sorted.sort_unstable_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)));

        // Coalesce adjacent same-country intervals and trim partial overlaps cleanly
        // so all stored ranges are strictly non-overlapping and sorted by start.
        let mut merged: Vec<ThroneFastGeoRange> = Vec::with_capacity(sorted.len());
        for r in sorted {
            if let Some(last) = merged.last_mut() {
                if r.start <= last.end {
                    if r.end <= last.end {
                        // Completely contained inside `last`; keep deterministic first range.
                        continue;
                    }
                    if r.country == last.country {
                        last.end = r.end;
                        continue;
                    }
                    // Different country partially overlapping; trim start beyond last.end
                    let new_start = match last.end.checked_add(1) {
                        Some(s) if s <= r.end => s,
                        _ => continue,
                    };
                    merged.push(ThroneFastGeoRange {
                        start: new_start,
                        end: r.end,
                        country: r.country,
                        _pad: [0; 2],
                    });
                    continue;
                }
                if r.country == last.country && last.end.saturating_add(1) == r.start {
                    last.end = r.end;
                    continue;
                }
            }
            merged.push(r);
        }

        Self { ranges: merged }
    }

    #[inline(always)]
    pub fn lookup(&self, ip: u32) -> [u8; 2] {
        let idx = self.ranges.partition_point(|r| r.start <= ip);
        if idx == 0 {
            return [0, 0];
        }
        let r = &self.ranges[idx - 1];
        if ip <= r.end {
            r.country
        } else {
            [0, 0]
        }
    }
}

/// # Safety
/// When `count > 0` and `ranges` is non-null, `ranges` must point to `count` valid initialized `ThroneFastGeoRange` elements.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_geo_create(
    ranges: *const ThroneFastGeoRange,
    count: usize,
) -> *mut ThroneFastGeoIndex {
    let slice = if count == 0 || ranges.is_null() {
        &[]
    } else {
        unsafe { slice::from_raw_parts(ranges, count) }
    };
    Box::into_raw(Box::new(ThroneFastGeoIndex::from_ranges(slice)))
}

/// # Safety
/// `index` must have been allocated by `throne_fast_geo_create` or be null.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_geo_free(index: *mut ThroneFastGeoIndex) {
    if !index.is_null() {
        unsafe { drop(Box::from_raw(index)) };
    }
}

/// Lookup 2-byte ISO country code into `out_cc2` (writable 2 bytes).
/// Returns 1 if matched, 0 if not found, -1 on null pointer.
///
/// # Safety
/// `index` and `out_cc2` must be valid.
#[no_mangle]
pub unsafe extern "C" fn throne_fast_geo_lookup_v4(
    index: *const ThroneFastGeoIndex,
    ip_host: u32,
    out_cc2: *mut u8,
) -> i32 {
    if index.is_null() || out_cc2.is_null() {
        return -1;
    }
    let cc = unsafe { &*index }.lookup(ip_host);
    let dst = unsafe { slice::from_raw_parts_mut(out_cc2, 2) };
    dst.copy_from_slice(&cc);
    i32::from(cc != [0, 0])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geo_sorted_coalesced_lookup() {
        let ranges = [
            ThroneFastGeoRange {
                start: 100,
                end: 200,
                country: *b"US",
                _pad: [0; 2],
            },
            ThroneFastGeoRange {
                start: 201,
                end: 300,
                country: *b"US",
                _pad: [0; 2],
            },
            ThroneFastGeoRange {
                start: 250,
                end: 350,
                country: *b"DE",
                _pad: [0; 2],
            },
        ];
        let idx = ThroneFastGeoIndex::from_ranges(&ranges);
        assert_eq!(idx.lookup(99), [0, 0]);
        assert_eq!(idx.lookup(150), *b"US");
        assert_eq!(idx.lookup(300), *b"US");
        assert_eq!(idx.lookup(301), *b"DE");
        assert_eq!(idx.lookup(351), [0, 0]);
    }
}
