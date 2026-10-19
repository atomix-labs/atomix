//! `Atom` for core's `Layout`, its size in the lower word and its alignment in the upper, the two
//! words core keeps, so neither moves through a shift.

use core::alloc::Layout;

use super::Atom;
use crate::range::ReprRange;
use crate::validity::ZeroNiche;

/// Where a layout's alignment lies in its repr: the upper word.
const ALIGNMENT_OFFSET: u32 = u64::BITS;

/// The least repr: no size, aligned to one.
const MIN_REPR: u128 = 1 << ALIGNMENT_OFFSET;

/// The greatest repr: the greatest size beside the greatest alignment.
const MAX_REPR: u128 = 1 << (u128::BITS - 1) | isize::MAX.cast_unsigned().wrapping_cast::<u128>();

// SAFETY: the repr is the size, at most `isize::MAX`, in the lower word, and the alignment, a power
// of two, in the upper, so it lies in `REPRS`; `from_repr` decodes exactly the reprs whose size and
// alignment make a layout, by the repr alone, as `from_repr_unchecked` does unchecked, and zero,
// of no alignment, is none, which the niche spends; a layout holds no pointer, so it may cross
// threads.
#[expect(unsafe_code, reason = "an `Atom` impl promises what loads rely on")]
const unsafe impl Atom for Layout {
    type Repr = u128;
    type Validity = ZeroNiche;
    const REPRS: ReprRange<u128> = ReprRange::new(MIN_REPR, MAX_REPR);
    #[inline]
    fn to_repr(self) -> u128 {
        self.align().wrapping_cast::<u128>() << ALIGNMENT_OFFSET
            | self.size().wrapping_cast::<u128>()
    }
    #[inline]
    fn from_repr(repr: u128) -> Option<Self> {
        let alignment = repr >> ALIGNMENT_OFFSET;
        match Self::from_size_align(repr.wrapping_cast(), alignment.wrapping_cast()) {
            Ok(layout) if alignment >> usize::BITS == 0 => Some(layout),
            Ok(_) | Err(_) => None,
        }
    }
    // The default, `from_repr`'s unwrap, checks the size and the alignment again on each load,
    // which `layout_load`'s pin refuses.
    #[inline]
    unsafe fn from_repr_unchecked(repr: u128) -> Self {
        // SAFETY: the caller's repr decodes, so its size and alignment make a layout.
        unsafe {
            Self::from_size_align_unchecked(
                repr.wrapping_cast(),
                (repr >> ALIGNMENT_OFFSET).wrapping_cast(),
            )
        }
    }
}
