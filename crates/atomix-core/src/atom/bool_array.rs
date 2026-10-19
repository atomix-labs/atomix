//! The bits of an array of `bool`s, a bit each, element 0 lowest, and the array they hold: one
//! multiply per eight `bool`s, and, past eight, a vector per 64 at run time, NEON's on `aarch64`
//! and AVX2's on `x86_64`; a constant takes the multiplies.
//!
//! core's portable `Mask` reaches the same instructions on `x86_64`, but not NEON's `ushl; addp`
//! on `aarch64`.

#[cfg(neon_bools)]
use core::arch::aarch64::{
    uint8x16_t, vaddq_u8, vandq_u8, vcombine_u64, vcreate_u64, vdupq_n_u8, vdupq_n_u64,
    vgetq_lane_u64, vpaddq_u8, vqtbl1q_u8, vreinterpretq_s8_u64, vreinterpretq_u8_u64,
    vreinterpretq_u64_u8, vshlq_u8,
};
#[cfg(avx2_bools)]
use core::arch::x86_64::{
    __m256i, _mm256_and_si256, _mm256_min_epu8, _mm256_movemask_epi8, _mm256_set1_epi8,
    _mm256_set1_epi64x, _mm256_setr_epi64x, _mm256_shuffle_epi8, _mm256_slli_epi16,
};
#[cfg(vector_bools)]
use core::intrinsics::const_eval_select;
use core::mem::transmute;

/// Bit 0 of each byte.
const LOW_BIT_OF_EACH_BYTE: u64 = 0x0101_0101_0101_0101;

/// Eight `bool`s from a byte, bit `k` to element `k`: one multiply moves each bit `k` from 0 to 6
/// up `7k` places, to bit 0 of byte `k`, and a shift moves bit 7 to byte 7.
#[expect(unsafe_code, reason = "reads bytes of 0 or 1 as `bool`s")]
#[inline]
const fn spread(byte: u8) -> [bool; 8] {
    let byte = u64::from(byte);
    let word = ((byte & 0x7F).wrapping_mul(0x0002_0408_1020_4081) & LOW_BIT_OF_EACH_BYTE)
        | (byte >> 7) << 56;
    // SAFETY: the mask leaves each byte 0 or 1, and `byte >> 7`, 0 or 1, ors into bit 0 of byte 7
    // alone: each byte is a `bool`.
    unsafe { transmute::<[u8; 8], [bool; 8]>(word.to_le_bytes()) }
}

/// A byte from eight `bool`s, element `k` to bit `k`: one multiply sums byte `k` into bit `56 + k`.
#[expect(unsafe_code, reason = "reads `bool`s as bytes")]
#[inline]
const fn gather(bools: [bool; 8]) -> u8 {
    // SAFETY: a `bool` is a byte, 0 or 1.
    let word = u64::from_le_bytes(unsafe { transmute::<[bool; 8], [u8; 8]>(bools) });
    (word.wrapping_mul(0x0102_0408_1020_4080) >> 56).wrapping_cast()
}

/// 64 `bool`s from 64 bits, as [`spread`] gives eight from each byte.
#[inline]
const fn spread_word(bits: u64) -> [bool; 64] {
    #[cfg(vector_bools)]
    {
        const_eval_select((bits,), spread_bytes, spread_vector)
    }
    #[cfg(not(vector_bools))]
    {
        spread_bytes(bits)
    }
}

/// 64 `bool`s from 64 bits, a byte at a time: [`spread_word`] in a constant, and where no vector
/// runs it.
#[inline]
const fn spread_bytes(bits: u64) -> [bool; 64] {
    let mut bools = [false; 64];
    let (mut rest, _) = bools.as_chunks_mut::<8>();
    let mut byte = 0;
    while let [chunk, after @ ..] = rest {
        *chunk = spread(bits.unbounded_shr(byte).wrapping_cast());
        byte = byte.wrapping_add(8);
        rest = after;
    }
    bools
}

/// 64 `bool`s from 64 bits at run time, by NEON, which the build enables: [`spread_neon`].
#[cfg(neon_bools)]
#[expect(unsafe_code, reason = "calls a function of a target feature the build enables")]
#[inline]
fn spread_vector(bits: u64) -> [bool; 64] {
    // SAFETY: the build enables NEON, as the `cfg` above says.
    unsafe { spread_neon(bits) }
}

/// 64 `bool`s from 64 bits by NEON: `tbl` gives lane `k` byte `k / 8`, and `ushl` right by `k % 8`
/// and `and` with 1 leave its bit `k % 8`.
#[cfg(neon_bools)]
#[expect(unsafe_code, reason = "reads vectors of bytes of 0 or 1 as `bool`s")]
#[inline]
#[target_feature(enable = "neon")]
fn spread_neon(bits: u64) -> [bool; 64] {
    let bytes = vreinterpretq_u8_u64(vdupq_n_u64(bits));
    // Lane `k` shifts right by `k % 8`, a negative left shift.
    let right = vreinterpretq_s8_u64(vdupq_n_u64(0xF9FA_FBFC_FDFE_FF00));
    // The first sixteen lanes take bytes 0 and 1, eight lanes each.
    let first =
        vreinterpretq_u8_u64(vcombine_u64(vcreate_u64(0), vcreate_u64(LOW_BIT_OF_EACH_BYTE)));
    let lanes = |index| vandq_u8(vshlq_u8(vqtbl1q_u8(bytes, index), right), vdupq_n_u8(1));
    let next = |index| vaddq_u8(index, vdupq_n_u8(2));
    let (second, third) = (next(first), next(next(first)));
    let vectors = [lanes(first), lanes(second), lanes(third), lanes(next(third))];
    // SAFETY: four vectors of sixteen bytes, each 0 or 1 by the `and`, are 64 `bool`s.
    unsafe { transmute::<[uint8x16_t; 4], [bool; 64]>(vectors) }
}

/// 64 `bool`s from 64 bits at run time, by AVX2, which the build enables: [`spread_avx2`].
#[cfg(avx2_bools)]
#[expect(unsafe_code, reason = "calls a function of a target feature the build enables")]
#[inline]
fn spread_vector(bits: u64) -> [bool; 64] {
    // SAFETY: the build enables AVX2, as the `cfg` above says.
    unsafe { spread_avx2(bits) }
}

/// 64 `bool`s from 64 bits by AVX2: `vpshufb` gives lane `k` byte `k / 8`, `vpand` keeps its bit
/// `k % 8`, and the lesser of that and 1 is 0 or 1, which LLVM emits as `vpcmpeqb; vpand`.
#[cfg(avx2_bools)]
#[expect(unsafe_code, reason = "reads vectors of bytes of 0 or 1 as `bool`s")]
#[inline]
#[target_feature(enable = "avx2")]
fn spread_avx2(bits: u64) -> [bool; 64] {
    /// Bit `k` of byte `k`.
    const BIT_OF_BYTE: u64 = 0x8040_2010_0804_0201;
    let bytes = _mm256_set1_epi64x(bits.cast_signed());
    let bit = _mm256_set1_epi64x(BIT_OF_BYTE.cast_signed());
    let lanes = |index| {
        let tested = _mm256_and_si256(_mm256_shuffle_epi8(bytes, index), bit);
        _mm256_min_epu8(tested, _mm256_set1_epi8(1))
    };
    // Each eight lanes take one byte, within the 128-bit half `vpshufb` shuffles in.
    let byte = |index: u64| LOW_BIT_OF_EACH_BYTE.wrapping_mul(index).cast_signed();
    let first = _mm256_setr_epi64x(byte(0), byte(1), byte(2), byte(3));
    let second = _mm256_setr_epi64x(byte(4), byte(5), byte(6), byte(7));
    // SAFETY: two vectors of 32 bytes, each 0 or 1 by the minimum, are 64 `bool`s.
    unsafe { transmute::<[__m256i; 2], [bool; 64]>([lanes(first), lanes(second)]) }
}

/// 64 bits from 64 `bool`s, as [`gather`] gives a byte from each eight.
#[inline]
const fn gather_word(bools: [bool; 64]) -> u64 {
    #[cfg(vector_bools)]
    {
        const_eval_select((bools,), gather_bytes, gather_vector)
    }
    #[cfg(not(vector_bools))]
    {
        gather_bytes(bools)
    }
}

/// 64 bits from 64 `bool`s, a byte at a time: [`gather_word`] in a constant, and where no vector
/// runs it.
#[inline]
const fn gather_bytes(bools: [bool; 64]) -> u64 {
    let mut bits = 0;
    let mut byte = 0;
    let (mut rest, _) = bools.as_chunks::<8>();
    while let [chunk, after @ ..] = rest {
        bits |= u64::from(gather(*chunk)).unbounded_shl(byte);
        byte = byte.wrapping_add(8);
        rest = after;
    }
    bits
}

/// 64 bits from 64 `bool`s at run time, by NEON, which the build enables: [`gather_neon`].
#[cfg(neon_bools)]
#[expect(unsafe_code, reason = "calls a function of a target feature the build enables")]
#[inline]
fn gather_vector(bools: [bool; 64]) -> u64 {
    // SAFETY: the build enables NEON, as the `cfg` above says.
    unsafe { gather_neon(bools) }
}

/// 64 bits from 64 `bool`s by NEON: `ushl` moves lane `k`'s 0 or 1 to bit `k % 8`, and pairwise
/// adds, `addp`, sum each eight lanes into a byte.
#[cfg(neon_bools)]
#[expect(unsafe_code, reason = "reads `bool`s as vectors of bytes")]
#[inline]
#[target_feature(enable = "neon")]
fn gather_neon(bools: [bool; 64]) -> u64 {
    // SAFETY: 64 `bool`s, each a byte, are four vectors of sixteen bytes.
    let [first, second, third, fourth] = unsafe { transmute::<[bool; 64], [uint8x16_t; 4]>(bools) };
    // Lane `k` shifts left by `k % 8`.
    let left = vreinterpretq_s8_u64(vdupq_n_u64(0x0706_0504_0302_0100));
    let weigh = |vector| vshlq_u8(vector, left);
    let halves = vpaddq_u8(weigh(first), weigh(second));
    let sums = vpaddq_u8(halves, vpaddq_u8(weigh(third), weigh(fourth)));
    vgetq_lane_u64::<0>(vreinterpretq_u64_u8(vpaddq_u8(sums, sums)))
}

/// 64 bits from 64 `bool`s at run time, by AVX2, which the build enables: [`gather_avx2`].
#[cfg(avx2_bools)]
#[expect(unsafe_code, reason = "calls a function of a target feature the build enables")]
#[inline]
fn gather_vector(bools: [bool; 64]) -> u64 {
    // SAFETY: the build enables AVX2, as the `cfg` above says.
    unsafe { gather_avx2(bools) }
}

/// 64 bits from 64 `bool`s by AVX2: a shift left by 7 moves each byte's 0 or 1 to its top bit,
/// which `vpmovmskb` gathers.
#[cfg(avx2_bools)]
#[expect(unsafe_code, reason = "reads `bool`s as vectors of bytes")]
#[inline]
#[target_feature(enable = "avx2")]
fn gather_avx2(bools: [bool; 64]) -> u64 {
    // SAFETY: 64 `bool`s, each a byte, are two vectors of 32 bytes.
    let [low, high] = unsafe { transmute::<[bool; 64], [__m256i; 2]>(bools) };
    let mask =
        |vector| u64::from(_mm256_movemask_epi8(_mm256_slli_epi16::<7>(vector)).cast_unsigned());
    mask(low) | mask(high) << 32
}

/// The `bool`s the low `N` bits of `bits` hold, bit `i` to element `i`: at most 128 of them, the
/// first 64 from one word and the rest from the next.
#[inline]
pub(super) const fn bits_to_bools<const N: usize>(bits: u128) -> [bool; N] {
    let mut bools = [false; N];
    if N <= 8 {
        let eight = spread(bits.wrapping_cast());
        let (low, _) = eight.split_at(N);
        bools.copy_from_slice(low);
        return bools;
    }
    let (low, high) = bools.split_at_mut(if N < 64 { N } else { 64 });
    let word = spread_word(bits.wrapping_cast());
    let (spread, _) = word.split_at(low.len());
    low.copy_from_slice(spread);
    if !high.is_empty() {
        let word = spread_word(bits.unbounded_shr(64).wrapping_cast());
        let (spread, _) = word.split_at(high.len());
        high.copy_from_slice(spread);
    }
    bools
}

/// The bits of `bools`, element `i` to bit `i`: at most 128 of them, the first 64 in one word and
/// the rest in the next.
#[inline]
pub(super) const fn bools_to_bits<const N: usize>(bools: [bool; N]) -> u128 {
    if N <= 8 {
        let mut eight = [false; 8];
        let (low, _) = eight.split_at_mut(N);
        low.copy_from_slice(&bools);
        return u128::from(gather(eight));
    }
    let (low, high) = bools.split_at(if N < 64 { N } else { 64 });
    let mut word = [false; 64];
    let (filled, _) = word.split_at_mut(low.len());
    filled.copy_from_slice(low);
    let mut bits = u128::from(gather_word(word));
    if !high.is_empty() {
        let mut word = [false; 64];
        let (filled, _) = word.split_at_mut(high.len());
        filled.copy_from_slice(high);
        bits |= u128::from(gather_word(word)) << 64;
    }
    bits
}

#[cfg(test)]
mod tests {
    use core::array;
    use core::hint::black_box;

    use super::{
        bits_to_bools, bools_to_bits, gather, gather_bytes, gather_word, spread, spread_bytes,
        spread_word,
    };

    /// The `bool`s `bits` hold, one at a time: what each decode must give.
    fn each_bit<const N: usize>(bits: u128) -> [bool; N] {
        array::from_fn(|index| bits >> index & 1 == 1)
    }

    #[test]
    fn eight_bools_spread_from_and_gather_into_every_byte() {
        for byte in 0..=u8::MAX {
            assert_eq!(spread(byte), each_bit(byte.into()), "{byte:#010b} spread");
            assert_eq!(gather(spread(byte)), byte, "and gathered back");
        }
    }

    #[test]
    fn a_word_spreads_and_gathers_alike_at_run_time_and_in_a_constant() {
        const WORD: u64 = 0x8123_4567_89AB_CDEF;
        const SPREAD: [bool; 64] = spread_word(WORD);
        const GATHERED: u64 = gather_word(SPREAD);
        assert_eq!(SPREAD, spread_word(black_box(WORD)), "spread in a constant");
        assert_eq!(GATHERED, WORD, "and gathered back in one");
        for shift in 0..64 {
            let word = WORD.rotate_left(shift) ^ (1 << shift);
            assert_eq!(spread_word(word), each_bit(word.into()), "{word:#x} spread");
            assert_eq!(spread_word(word), spread_bytes(word), "as a byte at a time");
            assert_eq!(gather_word(spread_word(word)), word, "and gathered back");
            assert_eq!(gather_bytes(spread_word(word)), word, "as a byte at a time");
        }
    }

    #[test]
    fn every_length_round_trips() {
        const BITS: u128 = 0x8_0123_4567_89AB_CDEF_FEDC_BA98;
        /// Checks `N` `bool`s from `BITS`' low `N` bits, and back.
        fn round_trips<const N: usize>() {
            let low = BITS & u128::MAX >> (128 - N);
            assert_eq!(bits_to_bools::<N>(low), each_bit::<N>(low), "{N} spread");
            assert_eq!(bools_to_bits(bits_to_bools::<N>(low)), low, "{N} gathered back");
        }
        round_trips::<1>();
        round_trips::<3>();
        round_trips::<8>();
        round_trips::<9>();
        round_trips::<63>();
        round_trips::<64>();
        round_trips::<65>();
        round_trips::<100>();
        round_trips::<128>();
    }
}
