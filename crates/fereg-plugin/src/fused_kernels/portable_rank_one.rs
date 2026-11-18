use fereg_core::kernels::weighted_upper_rank_one;
use std::arch::{asm, x86_64::*};
use std::mem::MaybeUninit;

fn read_mxcsr() -> u32 {
    let mut control = MaybeUninit::<u32>::uninit();
    // SAFETY: STMXCSR writes four bytes to the valid stack slot.
    unsafe {
        asm!("stmxcsr [{pointer}]", pointer = in(reg) control.as_mut_ptr(),
            options(nostack, preserves_flags));
        control.assume_init()
    }
}

fn is_standard_mxcsr(control: u32) -> bool {
    // Nearest-even, gradual underflow, preserved subnormal inputs and masked
    // exceptions. Ignore the six sticky exception status bits.
    control & 0xffc0 == 0x1f80
}

fn standard_mxcsr() -> bool {
    is_standard_mxcsr(read_mxcsr())
}

/// Certify both lanes using nearest-even binary64 with gradual underflow.
///
/// For a finite nonzero rounded product, the two adjacent encodings bracket
/// its exact real value. Negative encodings reverse the endpoint order. IEEE
/// rounding is monotone, so equal rounded sums at both endpoints certify the
/// rounded exact sum, including overflow when both round to the same infinity.
/// A finite rounded product at the top of the range has an infinite endpoint,
/// and the real product still lies between that endpoint and its finite neighbor.
/// Zero results require separate sign handling. A zero or nonfinite product has
/// a NaN endpoint, which cannot pass the equality test.
#[inline(always)]
fn certify_pair(left: __m128d, right: __m128d, sum: __m128d) -> (__m128d, i32) {
    // SAFETY: every x86-64 processor supports SSE2, and no pointer is used.
    unsafe {
        let product = _mm_castpd_si128(_mm_mul_pd(left, right));
        let one = _mm_set1_epi64x(1);
        let endpoint1 = _mm_castsi128_pd(_mm_sub_epi64(product, one));
        let endpoint2 = _mm_castsi128_pd(_mm_add_epi64(product, one));
        let rounded1 = _mm_add_pd(endpoint1, sum);
        let rounded2 = _mm_add_pd(endpoint2, sum);
        let finite_sum = _mm_cmplt_pd(
            _mm_andnot_pd(_mm_set1_pd(-0.0), sum),
            _mm_set1_pd(f64::INFINITY),
        );
        let equal = _mm_and_pd(_mm_cmpeq_pd(rounded1, rounded2), finite_sum);
        let nonzero = _mm_cmpneq_pd(rounded1, _mm_setzero_pd());
        (rounded1, _mm_movemask_pd(_mm_and_pd(equal, nonzero)))
    }
}

#[inline(always)]
fn update_pairs(weighted_left: f64, entries: &mut [f64], right_values: &[f64]) {
    let mut entries = entries.chunks_exact_mut(2);
    let mut right_values = right_values.chunks_exact(2);
    // SAFETY: every x86-64 processor supports SSE2. Each complete chunk
    // contains two doubles and unaligned loads and stores accept its pointer.
    unsafe {
        let left = _mm_set1_pd(weighted_left);
        for (entries, right_values) in entries.by_ref().zip(right_values.by_ref()) {
            let right = _mm_loadu_pd(right_values.as_ptr());
            let sum = _mm_loadu_pd(entries.as_ptr());
            let (rounded, valid) = certify_pair(left, right, sum);
            if valid == 3 {
                _mm_storeu_pd(entries.as_mut_ptr(), rounded);
            } else {
                update_uncertified_pair(weighted_left, right_values, entries, rounded, valid);
            }
        }
    }
    for (entry, &right_value) in entries
        .into_remainder()
        .iter_mut()
        .zip(right_values.remainder())
    {
        *entry = weighted_left.mul_add(right_value, *entry);
    }
}

#[inline(never)]
fn update_uncertified_pair(
    weighted_left: f64,
    right_values: &[f64],
    entries: &mut [f64],
    rounded: __m128d,
    valid: i32,
) {
    let mut values = [0.0; 2];
    // SAFETY: SSE2 is baseline and the destination has exactly two doubles.
    unsafe { _mm_storeu_pd(values.as_mut_ptr(), rounded) };
    for lane in 0..2 {
        entries[lane] = if valid & (1 << lane) != 0 {
            values[lane]
        } else {
            weighted_left.mul_add(right_values[lane], entries[lane])
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hint::black_box;

    const STANDARD_CONTROL: u32 = 0x1f80;
    const INVALID_UNMASKED: u32 = STANDARD_CONTROL & !0x0080;
    const NONSTANDARD_CONTROLS: [u32; 6] = [
        STANDARD_CONTROL | 0x2000, // Downward rounding
        STANDARD_CONTROL | 0x4000, // Upward rounding
        STANDARD_CONTROL | 0x6000, // Rounding toward zero
        STANDARD_CONTROL | 0x0040, // Denormals are zero
        STANDARD_CONTROL | 0x8000, // Flush to zero
        INVALID_UNMASKED,
    ];

    #[test]
    fn mxcsr_predicate_accepts_status_flags_and_refuses_nonstandard_modes() {
        assert!(is_standard_mxcsr(STANDARD_CONTROL));
        assert!(is_standard_mxcsr(STANDARD_CONTROL | 0x003f));
        for control in NONSTANDARD_CONTROLS {
            assert!(!is_standard_mxcsr(control), "{control:#06x}");
        }
    }

    fn runs_under_rosetta() -> bool {
        #[cfg(target_os = "macos")]
        {
            let mut translated: libc::c_int = 0;
            let mut size = std::mem::size_of_val(&translated);
            // SAFETY: the query is read-only and the output pointer and length
            // describe a writable c_int. ENOENT is expected on Intel Macs.
            let status = unsafe {
                libc::sysctlbyname(
                    c"sysctl.proc_translated".as_ptr(),
                    (&mut translated as *mut libc::c_int).cast(),
                    &mut size,
                    std::ptr::null_mut(),
                    0,
                )
            };
            status == 0 && size == std::mem::size_of_val(&translated) && translated == 1
        }
        #[cfg(not(target_os = "macos"))]
        false
    }

    #[test]
    fn mxcsr_guard_reads_the_workers_control_word() {
        #[repr(align(16))]
        struct FxState([u8; 512]);

        // FXSAVE's MXCSR_MASK describes which bits can be loaded. Some SSE2
        // processors lack DAZ, so the register test checks it only if writable.
        let mut state = FxState([0; 512]);
        // SAFETY: FXSAVE writes within the 512-byte, 16-byte-aligned area.
        unsafe {
            asm!("fxsave [{pointer}]", pointer = in(reg) state.0.as_mut_ptr(),
                options(nostack, preserves_flags));
        }
        let mask = u32::from_le_bytes(state.0[28..32].try_into().unwrap());
        let mask = if mask == 0 { 0xffbf } else { mask };

        let translated = runs_under_rosetta();
        let original = read_mxcsr();
        for control in [STANDARD_CONTROL].into_iter().chain(NONSTANDARD_CONTROLS) {
            if control & !mask != 0 {
                continue;
            }
            std::thread::spawn(move || {
                let mut previous = MaybeUninit::<u32>::uninit();
                let mut readback = MaybeUninit::<u32>::uninit();
                // SAFETY: the requested control has no unsupported bits or
                // pending exceptions. This block restores the original MXCSR
                // before any compiler-generated instruction can run.
                let (previous, observed) = unsafe {
                    asm!(
                        "stmxcsr [{previous}]",
                        "ldmxcsr [{requested}]",
                        "stmxcsr [{readback}]",
                        "ldmxcsr [{previous}]",
                        requested = in(reg) &control,
                        readback = in(reg) readback.as_mut_ptr(),
                        previous = in(reg) previous.as_mut_ptr(),
                        options(nostack, preserves_flags),
                    );
                    (previous.assume_init(), readback.assume_init())
                };
                assert_eq!(read_mxcsr(), previous);
                assert_eq!(
                    standard_mxcsr(),
                    is_standard_mxcsr(previous),
                    "worker's restored MXCSR {previous:#06x}"
                );
                if observed != control {
                    // Rosetta can keep the invalid-operation exception masked.
                    // The guard must follow the live state returned by STMXCSR.
                    assert!(
                        translated && control == INVALID_UNMASKED && observed == STANDARD_CONTROL,
                        "requested {control:#06x}, observed {observed:#06x}, Rosetta {translated}"
                    );
                    eprintln!("Rosetta requested MXCSR {control:#06x}, read back {observed:#06x}");
                }
                assert_eq!(is_standard_mxcsr(observed), observed == STANDARD_CONTROL);
            })
            .join()
            .unwrap();
        }
        assert_eq!(read_mxcsr() & !0x003f, original & !0x003f);
    }

    #[test]
    fn adjacent_bounds_retain_exact_rounding() {
        // The exact rational witnesses in fereg-core's EXACT_FMA_VECTORS,
        // checked independently by scripts/check_fma_rounding_vectors.py.
        let witnesses = [
            [
                0x3ff0_0000_0000_0001,
                0x3ff8_0000_0000_0000,
                0xb960_0000_0000_0000,
                0x3ff8_0000_0000_0001,
            ],
            [
                0x3ff0_0000_0000_0003,
                0x3ff8_0000_0000_0000,
                0x3960_0000_0000_0000,
                0x3ff8_0000_0000_0005,
            ],
            [
                0xbff0_0000_0000_0001,
                0x3ff8_0000_0000_0000,
                0x3960_0000_0000_0000,
                0xbff8_0000_0000_0001,
            ],
            [
                0xbff0_0000_0000_0003,
                0x3ff8_0000_0000_0000,
                0xb960_0000_0000_0000,
                0xbff8_0000_0000_0005,
            ],
            [
                0x3ff0_0000_0200_0000,
                0x3fef_ffff_fc00_0000,
                0xbff0_0000_0000_0000,
                0xbc90_0000_0000_0000,
            ],
            [
                0x3ff0_0000_0000_0001,
                0x3fef_ffff_ffff_fffe,
                0xbff0_0000_0000_0000,
                0xb970_0000_0000_0000,
            ],
        ];
        for [left, right, sum, expected] in black_box(witnesses) {
            assert_pair(
                f64::from_bits(left),
                f64::from_bits(right),
                f64::from_bits(sum),
                expected,
            );
        }
    }

    #[test]
    fn ties_zero_subnormal_boundaries_and_overflow_round_exactly() {
        const NEXT_ONE: f64 = f64::from_bits(0x3ff0_0000_0000_0001);
        const HALF_ULP_ONE: f64 = f64::from_bits(0x3ca0_0000_0000_0000);
        const LEAST_SUBNORMAL: f64 = f64::from_bits(1);
        const GREATEST_SUBNORMAL: f64 = f64::from_bits(0x000f_ffff_ffff_ffff);
        let cases = [
            (1.0, HALF_ULP_ONE, 1.0, 1.0f64.to_bits()),
            (1.0, HALF_ULP_ONE, NEXT_ONE, NEXT_ONE.to_bits() + 1),
            (1.0, -1.0, 1.0, 0),
            (1.0, -0.0, -0.0, (-0.0f64).to_bits()),
            (1.0, 0.0, -0.0, 0),
            (LEAST_SUBNORMAL, -0.5, 0.0, (-0.0f64).to_bits()),
            (LEAST_SUBNORMAL, 0.5, LEAST_SUBNORMAL, 2),
            (LEAST_SUBNORMAL, 1.0, 0.0, 1),
            (LEAST_SUBNORMAL, -1.0, 0.0, (-LEAST_SUBNORMAL).to_bits()),
            (
                LEAST_SUBNORMAL,
                1.0,
                GREATEST_SUBNORMAL,
                f64::MIN_POSITIVE.to_bits(),
            ),
            (
                LEAST_SUBNORMAL,
                -1.0,
                f64::MIN_POSITIVE,
                GREATEST_SUBNORMAL.to_bits(),
            ),
            (f64::MIN_POSITIVE, 0.5, 0.0, 0x0008_0000_0000_0000),
            (f64::MAX, 2.0, -f64::MAX, f64::MAX.to_bits()),
            (f64::MAX, 1.0, f64::MAX, f64::INFINITY.to_bits()),
            (-f64::MAX, 1.0, -f64::MAX, f64::NEG_INFINITY.to_bits()),
        ];

        for (left, right, sum, expected) in black_box(cases) {
            assert_pair(left, right, sum, expected);
        }
    }

    fn assert_pair(left: f64, right: f64, sum: f64, expected: u64) {
        let mut entries = [sum, sum];
        update_pairs(left, &mut entries, &[right, right]);
        assert_eq!(entries.map(f64::to_bits), [expected; 2]);
    }

    #[test]
    fn certificate_handles_mixed_lanes() {
        // SAFETY: SSE2 is baseline and no pointer is used.
        let (_, valid) =
            unsafe { certify_pair(_mm_set1_pd(1.0), _mm_set1_pd(1.0), _mm_set_pd(16.0, -1.0)) };
        assert_eq!(valid, 0b10);
        let mut entries = black_box([-1.0, 16.0]);
        update_pairs(1.0, &mut entries, &[1.0; 2]);
        assert_eq!(entries.map(f64::to_bits), [0, 17.0f64.to_bits()]);

        let mut entries = black_box([16.0, -1.0]);
        update_pairs(1.0, &mut entries, &[1.0; 2]);
        assert_eq!(entries.map(f64::to_bits), [17.0f64.to_bits(), 0]);
    }

    fn assert_same(actual: f64, expected: f64) {
        if expected.is_nan() {
            assert!(actual.is_nan());
        } else {
            assert_eq!(actual.to_bits(), expected.to_bits());
        }
    }

    fn next(bits: &mut u64) -> u64 {
        *bits ^= *bits << 13;
        *bits ^= *bits >> 7;
        *bits ^= *bits << 17;
        *bits
    }

    #[test]
    fn adjacent_bounds_match_fma_across_exponents() {
        let mut bits = 0xc58f_1a7b_51cd_093e;
        for _ in 0..40_000 {
            let left = f64::from_bits(next(&mut bits));
            let right = [
                f64::from_bits(next(&mut bits)),
                f64::from_bits(next(&mut bits)),
            ];
            let sum = [
                f64::from_bits(next(&mut bits)),
                f64::from_bits(next(&mut bits)),
            ];
            let mut actual = sum;
            update_pairs(left, &mut actual, &right);
            for lane in 0..2 {
                assert_same(actual[lane], left.mul_add(right[lane], sum[lane]));
            }
        }

        let values = [
            0.0,
            -0.0,
            f64::MIN_POSITIVE,
            f64::MAX,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ];
        for left in black_box(values) {
            for right in values {
                for sum in values {
                    let mut actual = [sum, -sum];
                    update_pairs(left, &mut actual, &[right, -right]);
                    assert_same(actual[0], left.mul_add(right, sum));
                    assert_same(actual[1], left.mul_add(-right, -sum));
                }
            }
        }
    }

    #[test]
    fn partitions_keep_portable_observation_order_and_storage() {
        let mut bits = 0x2497_ed67_81ad_ef12;
        let mut value = || ((next(&mut bits) as i64) >> 11) as f64 * (1.0 / (1u64 << 52) as f64);
        upper_rank_one(1.0, &[], 0.0, 0, &mut [], &mut []);
        for dimension in [1_usize, 2, 3, 7, 8, 17, 64, 65] {
            for left_start in [0, dimension / 2, dimension - 1] {
                let count = (dimension - left_start).div_ceil(2);
                let mut expected_rhs: Vec<_> = (0..count).map(|_| value()).collect();
                let mut expected_gram: Vec<_> =
                    (0..dimension * (count + 1)).map(|_| value()).collect();
                let mut actual_rhs = expected_rhs.clone();
                let mut actual_gram = expected_gram.clone();
                for _ in 0..50 {
                    let row: Vec<_> = (0..dimension).map(|_| value()).collect();
                    let weight = value();
                    let response = value();
                    weighted_upper_rank_one(
                        weight,
                        &row,
                        response,
                        left_start,
                        &mut expected_rhs,
                        &mut expected_gram,
                    );
                    upper_rank_one(
                        weight,
                        &row,
                        response,
                        left_start,
                        &mut actual_rhs,
                        &mut actual_gram,
                    );
                }
                for (actual, expected) in actual_rhs.iter().zip(expected_rhs) {
                    assert_same(*actual, expected);
                }
                for (actual, expected) in actual_gram.iter().zip(expected_gram) {
                    assert_same(*actual, expected);
                }
            }
        }
    }
}

pub(super) fn upper_rank_one(
    weight: f64,
    row: &[f64],
    response: f64,
    left_start: usize,
    rhs: &mut [f64],
    gram: &mut [f64],
) {
    if rhs.is_empty() {
        return;
    }
    let dimension = row.len();
    if dimension.saturating_sub(left_start) < 2 || !standard_mxcsr() {
        weighted_upper_rank_one(weight, row, response, left_start, rhs, gram);
        return;
    }
    let gram = &mut gram[..rhs.len() * dimension];
    for (local_left, (destination, gram_row)) in rhs
        .iter_mut()
        .zip(gram.chunks_exact_mut(dimension))
        .enumerate()
    {
        let left = left_start + local_left;
        let weighted_left = weight * row[left];
        *destination = weighted_left.mul_add(response, *destination);
        update_pairs(weighted_left, &mut gram_row[left..], &row[left..]);
    }
}
