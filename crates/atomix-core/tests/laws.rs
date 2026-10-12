//! The `Atom` laws over every built-in: a value's repr lies in its range, which may wrap, and
//! decodes back to it; a repr that decodes re-encodes to itself, unchecked too; each validity's
//! promise holds; a ranged integer's repr decodes, as itself, exactly where it lies in its range;
//! `None` takes a spare repr: zero where zero is the niche, else one outside its value's range;
//! no bits make a reference or a function pointer, but null; an array's repr decodes where each
//! slot's does, and a `Packed` one's where each field's does and the bits above extend the last;
//! a tuple's of bits where each field's does in its slot; and core's small values': an ordering,
//! the IP addresses, a socket address, a duration, a nonzero `char`, a reversed value, its enums of
//! payloads, its ranges, `Poll`, and its alignments, layouts, ASCII characters and `FpCategory`s.
//!
//! Each law runs on every repr of a byte, on the edges of each width and range, and on random bits;
//! a 16-bit ranged integer's, on every repr of 16 bits too.

// Loom's cells exist only inside a model; `model.rs` holds the loom tests.
#![cfg(not(loom))]
#![feature(ascii_char, f16, integer_casts, ptr_alignment_type)]
#![cfg_attr(wide, feature(f128))]

#[cfg(test)]
mod testing;

#[cfg(test)]
mod tests {
    #[cfg(wide)]
    use core::alloc::Layout;
    use core::ascii::Char as AsciiChar;
    use core::cmp::{Ordering, Reverse};
    use core::convert::Infallible;
    use core::fmt::{Alignment as FormatAlignment, Debug, Error as FormatError};
    use core::marker::{PhantomData, PhantomPinned};
    use core::mem::{Alignment, ManuallyDrop};
    #[cfg(wide)]
    use core::net::Ipv6Addr;
    use core::net::{Ipv4Addr, SocketAddrV4};
    use core::num::{FpCategory, NonZero, Saturating, Wrapping};
    use core::ops::{
        Bound, ControlFlow, RangeFull, RangeTo, RangeToInclusive as LegacyRangeToInclusive,
    };
    use core::ptr::{self, NonNull};
    use core::range::{Range, RangeFrom, RangeInclusive, RangeToInclusive};
    use core::task::Poll;
    #[cfg(wide)]
    use core::time::Duration;

    #[cfg(aarch64_code)]
    use atomix_core::ordering::Relaxed;
    use atomix_core::validity::{Partial, Total, TotalZeroNiche, ZeroNiche, ZeroValid};
    use atomix_core::{
        Atom, ExactBits, Packed, Primitive, RangedI8, RangedI16, RangedI32, RangedI64, RangedIsize,
        RangedU8, RangedU16, RangedU32, RangedU64, RangedUsize,
    };
    #[cfg(aarch64_code)]
    use atomix_core::{AtomOrd, Atomic, Load, MinMax};
    #[cfg(wide)]
    use atomix_core::{RangedI128, RangedU128};
    use proptest::prelude::prop_oneof;
    #[cfg(wide)]
    use proptest::prelude::{Strategy, any};
    use proptest::test_runner::TestCaseError;
    use proptest::{prop_assert, prop_assert_eq, proptest};

    use crate::testing::law::{
        assert_holds, canonical, decodes_as_promised, edge_or_random_bits, edges, none_laws,
        none_takes_a_spare_repr, ranged_integer_laws, reprs_order_as_ord, round_trips,
    };

    /// The last scalar value, and the first and last surrogates.
    const CHAR_EDGES: [u128; 3] = [0x10_FFFF, 0xD800, 0xDFFF];

    /// The bits past a socket address's port, a duration's last nanosecond and first second, the
    /// longest duration's, where its range ends, and the tags above 16-, 32- and 64-bit payloads.
    const CORE_VALUE_EDGES: [u128; 9] = [
        1 << 48,
        999_999_999,
        1 << 64,
        (1 << 64) + 999_999_999,
        (u128::MAX << 64) + 999_999_999,
        1 << 16 | 0xFFFF,
        2 << 16,
        1 << 32,
        1 << 33,
    ];

    /// The range and round-trip laws for `value`, whose repr is a pointer.
    fn pointer_round_trips<T: Atom<Repr = *mut P> + PartialEq + Debug, P>(
        value: T,
    ) -> Result<(), TestCaseError> {
        let address = value.to_repr().addr();
        prop_assert!(
            u128::try_from(address).is_ok_and(|bits| T::REPRS.contains(bits)),
            "{value:?}'s address {address:#x} lies in {:?}",
            T::REPRS
        );
        prop_assert_eq!(T::from_repr(value.to_repr()), Some(value), "the value decodes back");
        canonical::<T>(value.to_repr())
    }

    /// The float laws for `bits`: they lie in range and are the repr of the float they make, which
    /// decodes back to them, checked and unchecked, NaN included.
    fn keeps_every_bit<T: Atom<Validity = Total> + Debug>(
        bits: u128, from_bits: fn(T::Repr) -> T,
    ) -> Result<(), TestCaseError>
    where
        T::Repr: ExactBits + PartialEq + Debug,
    {
        let repr = <T::Repr as Primitive>::from_bits(bits);
        let unsigned = repr.to_bits();
        prop_assert!(T::REPRS.contains(unsigned), "the bits {unsigned:#x} lie in {:?}", T::REPRS);
        prop_assert_eq!(from_bits(repr).to_repr(), repr, "the repr is the bits");
        decodes_as_promised::<Total, T>(repr)
    }

    /// The order law for `a` and `b` through the atomic maximum and minimum, which compare reprs:
    /// each keeps what `Ord`'s does, and returns the value before.
    #[cfg(aarch64_code)]
    fn atomics_order_as_ord<T: AtomOrd + Debug>(a: T, b: T) -> Result<(), TestCaseError>
    where
        T::Repr: Load + MinMax,
    {
        let larger = Atomic::from(a);
        prop_assert_eq!(larger.fetch_max(b, Relaxed), a, "fetch_max returns the value before");
        prop_assert_eq!(larger.load(Relaxed), a.max(b), "fetch_max keeps the larger");
        let smaller = Atomic::from(a);
        prop_assert_eq!(smaller.fetch_min(b, Relaxed), a, "fetch_min returns the value before");
        prop_assert_eq!(smaller.load(Relaxed), a.min(b), "fetch_min keeps the smaller");
        Ok(())
    }

    /// Checks every law on the low bits of `$bits` and `$other` as each integer type.
    ///
    /// The same laws run on its `NonZero`, the `Option` of that, `Wrapping` and `Saturating`.
    macro_rules! integer_laws {
        ($bits:expr, $other:expr; $($int:ty),+) => {$({
            let (a, b): ($int, $int) = ($bits.wrapping_cast(), $other.wrapping_cast());
            decodes_as_promised::<Total, $int>(a)?;
            decodes_as_promised::<TotalZeroNiche, NonZero<$int>>(a)?;
            decodes_as_promised::<Total, Option<NonZero<$int>>>(a)?;
            round_trips(a)?;
            round_trips(Wrapping(a))?;
            round_trips(Saturating(a))?;
            round_trips(NonZero::new(a))?;
            reprs_order_as_ord(a, b)?;
            if let (Some(a), Some(b)) = (NonZero::new(a), NonZero::new(b)) {
                round_trips(a)?;
                reprs_order_as_ord(a, b)?;
            }
        })+};
    }

    /// Checks the atomic order law on the low bits of `$bits` and `$other` as each integer type.
    ///
    /// The same law runs on its `NonZero`, `Wrapping` and `Saturating`.
    #[cfg(aarch64_code)]
    macro_rules! atomic_order_laws {
        ($bits:expr, $other:expr; $($int:ty),+) => {$({
            let (a, b): ($int, $int) = ($bits.wrapping_cast(), $other.wrapping_cast());
            atomics_order_as_ord(a, b)?;
            atomics_order_as_ord(Wrapping(a), Wrapping(b))?;
            atomics_order_as_ord(Saturating(a), Saturating(b))?;
            if let (Some(a), Some(b)) = (NonZero::new(a), NonZero::new(b)) {
                atomics_order_as_ord(a, b)?;
            }
        })+};
    }

    /// Every law on `bits` and `other` for each integer of 64 bits or fewer.
    fn narrow_integer_laws(bits: u128, other: u128) -> Result<(), TestCaseError> {
        integer_laws!(bits, other; u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);
        #[cfg(aarch64_code)]
        atomic_order_laws!(bits, other; u8, u16, u32, u64, usize, i8, i16, i32, i64, isize);
        Ok(())
    }

    /// The atomic order law for the low bits of `bits` and `other` as `T`'s reprs, where both
    /// decode.
    #[cfg(aarch64_code)]
    fn ranged_atomics_order_as_ord<T: AtomOrd + Debug>(
        bits: u128, other: u128,
    ) -> Result<(), TestCaseError>
    where
        T::Repr: Load + MinMax,
    {
        let decode = |bits| T::from_repr(Primitive::from_bits(bits));
        if let (Some(a), Some(b)) = (decode(bits), decode(other)) {
            atomics_order_as_ord(a, b)
        } else {
            Ok(())
        }
    }

    /// Checks the ranged integer laws on the low bits of `$bits` and `$other` as each type's
    /// reprs, and on `aarch64` the atomic order law.
    macro_rules! ranged_laws {
        ($bits:expr, $other:expr; $($ranged:ty),+ $(,)?) => {$(
            ranged_integer_laws::<Partial, $ranged>($bits, $other)?;
            #[cfg(aarch64_code)]
            ranged_atomics_order_as_ord::<$ranged>($bits, $other)?;
        )+};
    }

    /// Checks the laws on the low bits of `$bits` as the repr of an `Option` of each type, whose
    /// validity is `Partial`.
    macro_rules! partial_option_laws {
        ($bits:expr; $($value:ty),+ $(,)?) => {$(
            decodes_as_promised::<Partial, Option<$value>>(Primitive::from_bits($bits))?;
        )+};
    }

    /// Every law on `bits` and `other` for the ranged integers of 16 bits, and on `bits` for an
    /// `Option` of each.
    fn sixteen_bit_ranged_laws(bits: u128, other: u128) -> Result<(), TestCaseError> {
        ranged_laws!(bits, other; RangedU16<3>, RangedI16<-300, 300>);
        partial_option_laws!(bits; RangedU16<3>, RangedI16<-300, 300>);
        Ok(())
    }

    /// Every law on `bits` and `other` for ranged integers of 64 bits or fewer, unsigned and
    /// signed: ranges below, through and above zero, full ones, one of a single integer, and the
    /// default `MAX`; and on `bits` for an `Option` of each of 16 bits or fewer that leaves `None`
    /// a repr.
    fn narrow_ranged_laws(bits: u128, other: u128) -> Result<(), TestCaseError> {
        ranged_laws!(bits, other; RangedU8<1, 10>, RangedU8<0, 255>, RangedI8<-5, 5>);
        ranged_laws!(bits, other; RangedI8<-100, -10>, RangedI8<{ i8::MIN }>);
        partial_option_laws!(bits; RangedU8<1, 10>, RangedI8<-5, 5>, RangedI8<-100, -10>);
        sixteen_bit_ranged_laws(bits, other)?;
        ranged_laws!(bits, other; RangedU32<7, 7>, RangedI32<{ i32::MIN }, -1>);
        ranged_laws!(bits, other; RangedU64<3>, RangedI64<{ i64::MIN }, 5>);
        ranged_laws!(bits, other; RangedUsize<1>, RangedIsize<-1, 1>);
        Ok(())
    }

    /// Every law on `bits` as the repr of the 128-bit values.
    #[cfg(wide)]
    fn wide_laws(bits: u128, other: u128) -> Result<(), TestCaseError> {
        integer_laws!(bits, other; u128, i128);
        ranged_integer_laws::<Partial, RangedU128<1>>(bits, other)?;
        ranged_integer_laws::<Partial, RangedI128<{ i128::MIN }, -1>>(bits, other)?;
        ranged_integer_laws::<Partial, RangedI128<-5, 5>>(bits, other)?;
        keeps_every_bit(bits, f128::from_bits)
    }

    /// Every law on the low bits of `bits` as the repr of a `char` and of its `Option`s.
    ///
    /// The order laws compare the `char` they make, if any, with `other`.
    fn char_laws(bits: u128, other: char) -> Result<(), TestCaseError> {
        let repr = u32::from_bits(bits);
        decodes_as_promised::<ZeroValid, char>(repr)?;
        decodes_as_promised::<ZeroValid, Option<char>>(repr)?;
        decodes_as_promised::<ZeroValid, Option<Option<char>>>(repr)?;
        round_trips(None::<char>)?;
        round_trips(None::<Option<char>>)?;
        if let Some(letter) = char::from_u32(repr) {
            round_trips(letter)?;
            round_trips(Some(letter))?;
            reprs_order_as_ord(letter, other)?;
            #[cfg(aarch64_code)]
            atomics_order_as_ord(letter, other)?;
        }
        Ok(())
    }

    /// Every law on the low bits of `bits` as each float's.
    fn float_laws(bits: u128) -> Result<(), TestCaseError> {
        keeps_every_bit(bits, f16::from_bits)?;
        keeps_every_bit(bits, f32::from_bits)?;
        keeps_every_bit(bits, f64::from_bits)
    }

    /// Every law on the low bits of `bits` as an address, for each pointer and `Option<NonNull>`.
    fn pointer_laws(bits: u128) -> Result<(), TestCaseError> {
        let pointer = ptr::without_provenance_mut::<u8>(bits.wrapping_cast());
        pointer_round_trips(pointer)?;
        pointer_round_trips(pointer.cast_const())?;
        pointer_round_trips(NonNull::new(pointer))?;
        if let Some(value) = NonNull::new(pointer) {
            pointer_round_trips(value)?;
        }
        decodes_as_promised::<Total, *mut u8>(pointer)?;
        decodes_as_promised::<Total, *const u8>(pointer)?;
        decodes_as_promised::<TotalZeroNiche, NonNull<u8>>(pointer)?;
        decodes_as_promised::<Total, Option<NonNull<u8>>>(pointer)?;
        decodes_as_promised::<TotalZeroNiche, [NonNull<u8>; 1]>(pointer)?;
        decodes_as_promised::<Total, [*mut u8; 1]>(pointer)
    }

    /// The type of a hook, whose signature is higher-ranked.
    type Hook = fn(&str) -> usize;

    /// A value a reference points to for as long as the program runs.
    static DEPTH: u64 = 10;

    /// Every law on the low bits of `bits` as the repr of a reference and of a function pointer,
    /// which bits make only null, and on a reference and a function pointer whole.
    fn sealed_laws(bits: u128) -> Result<(), TestCaseError> {
        let reference: <&u64 as Atom>::Repr = Primitive::from_bits(bits);
        let function: <Hook as Atom>::Repr = Primitive::from_bits(bits);
        prop_assert!(reference.is_bits(0) && function.is_bits(0), "bits make null alone");
        decodes_as_promised::<ZeroNiche, &u64>(reference)?;
        decodes_as_promised::<ZeroValid, Option<&u64>>(reference)?;
        decodes_as_promised::<ZeroNiche, [&u64; 1]>(reference)?;
        round_trips::<[&u64; 1]>([&DEPTH])?;
        decodes_as_promised::<TotalZeroNiche, Hook>(function)?;
        decodes_as_promised::<Total, Option<Hook>>(function)?;
        canonical::<Option<&u64>>(reference)?;
        canonical::<Option<Hook>>(function)?;
        round_trips::<&u64>(&DEPTH)?;
        round_trips(Some(&DEPTH))?;
        canonical::<&u64>((&DEPTH).to_repr())?;
        // A word's tags, in the bits a `u64`'s alignment leaves clear, make no reference.
        let tagged = (&DEPTH).to_repr().with_packed_bits((&DEPTH).to_repr().packed_bits() | 0b11);
        decodes_as_promised::<ZeroNiche, &u64>(tagged)?;
        prop_assert!(
            <&u64 as Atom>::from_repr(tagged).is_none(),
            "a tagged repr decodes as no reference"
        );
        // A wide reference's repr is sealed as a thin one's: no bits make one but null.
        #[cfg(wide)]
        {
            let name: <&str as Atom>::Repr = Primitive::from_bits(bits);
            decodes_as_promised::<ZeroNiche, &str>(name)?;
            decodes_as_promised::<ZeroValid, Option<&str>>(name)?;
            decodes_as_promised::<ZeroNiche, &str>("XNAS".to_repr())?;
            round_trips("XNAS")?;
        }
        let hook: Hook = str::len;
        canonical::<Hook>(hook.to_repr())
    }

    /// Checks what each type's validity promises of the low bits of `bits` as its repr, the repr
    /// laws, and the round trip of the value they decode as.
    macro_rules! repr_laws {
        ($bits:ident; $($validity:ty => $value_type:ty),+ $(,)?) => {$({
            let repr = <<$value_type as Atom>::Repr as Primitive>::from_bits($bits);
            decodes_as_promised::<$validity, $value_type>(repr)?;
            if let Some(value) = <$value_type as Atom>::from_repr(repr) {
                round_trips(value)?;
            }
        })+};
    }

    /// Every law on the low bits of `bits` as each of core's small values of 64 bits or fewer, and
    /// of their `Option`s.
    ///
    /// The order laws compare each value `bits` makes with the one `other` makes, if any.
    fn core_value_laws(bits: u128, other: u128) -> Result<(), TestCaseError> {
        repr_laws!(bits;
            ZeroValid => Result<u32, u8>, ZeroValid => Option<Result<u32, u8>>,
            Partial => Result<NonZero<u8>, bool>, ZeroValid => Result<i8, i16>,
            ZeroValid => ControlFlow<u8, u16>, ZeroValid => Bound<i16>, ZeroValid => Bound<bool>,
            Total => Range<u32>, Total => RangeInclusive<u8>, Total => RangeFrom<u64>,
            Total => RangeTo<u8>, Total => RangeToInclusive<u8>,
            Total => LegacyRangeToInclusive<u8>, ZeroValid => Poll<char>,
            Total => Poll<NonZero<u8>>, ZeroValid => Alignment, ZeroValid => AsciiChar,
            ZeroValid => FpCategory, ZeroValid => FormatAlignment, ZeroValid => PhantomPinned,
            ZeroValid => RangeFull, ZeroValid => FormatError, Total => ManuallyDrop<u16>,
        );
        let (byte, other_byte) = (u8::from_bits(bits), u8::from_bits(other));
        // No value is an `Infallible`, so no repr decodes, nor round-trips.
        decodes_as_promised::<Partial, Infallible>(byte)?;
        if let (Some(a), Some(b)) = (Alignment::from_repr(byte), Alignment::from_repr(other_byte)) {
            reprs_order_as_ord(a, b)?;
        }
        if let (Some(a), Some(b)) = (AsciiChar::from_repr(byte), AsciiChar::from_repr(other_byte)) {
            reprs_order_as_ord(a, b)?;
        }
        let (sign, repr) = (i8::from_bits(bits), u32::from_bits(bits));
        decodes_as_promised::<ZeroValid, Ordering>(sign)?;
        decodes_as_promised::<ZeroValid, Option<Ordering>>(sign)?;
        decodes_as_promised::<Total, Ipv4Addr>(repr)?;
        decodes_as_promised::<ZeroValid, SocketAddrV4>(u64::from_bits(bits))?;
        decodes_as_promised::<ZeroValid, Option<SocketAddrV4>>(u64::from_bits(bits))?;
        decodes_as_promised::<ZeroNiche, NonZero<char>>(repr)?;
        decodes_as_promised::<ZeroValid, Option<NonZero<char>>>(repr)?;
        decodes_as_promised::<Total, Reverse<u8>>(u8::from_bits(bits))?;
        decodes_as_promised::<ZeroValid, Reverse<char>>(repr)?;
        round_trips(Ipv4Addr::from_bits(repr))?;
        round_trips(Reverse(u8::from_bits(bits)))?;
        if let Some(socket) = SocketAddrV4::from_repr(u64::from_bits(bits)) {
            round_trips(socket)?;
            round_trips(Some(socket))?;
        }
        if let (Some(a), Some(b)) =
            (Ordering::from_repr(sign), Ordering::from_repr(i8::from_bits(other)))
        {
            round_trips(a)?;
            reprs_order_as_ord(a, b)?;
            #[cfg(aarch64_code)]
            atomics_order_as_ord(a, b)?;
        }
        if let (Some(a), Some(b)) =
            (NonZero::<char>::from_repr(repr), NonZero::<char>::from_repr(u32::from_bits(other)))
        {
            round_trips(a)?;
            round_trips(Some(a))?;
            reprs_order_as_ord(a, b)?;
            #[cfg(aarch64_code)]
            atomics_order_as_ord(a, b)?;
        }
        Ok(())
    }

    /// A duration's repr: any seconds, and nanoseconds below a billion, which random bits alone
    /// make once in 1.8e10 draws.
    #[cfg(wide)]
    fn duration_reprs() -> impl Strategy<Value = u128> {
        (any::<u64>(), 0..1_000_000_000_u128)
            .prop_map(|(seconds, nanoseconds)| u128::from(seconds) << 64 | nanoseconds)
    }

    /// Every law on `bits` and `other` as each of core's 128-bit values, and of their `Option`s.
    #[cfg(wide)]
    fn wide_core_value_laws(bits: u128, other: u128) -> Result<(), TestCaseError> {
        repr_laws!(bits;
            ZeroValid => Result<u64, u8>, ZeroValid => Bound<u64>, Total => Range<u64>,
            ZeroNiche => Layout, ZeroValid => Option<Layout>,
        );
        decodes_as_promised::<Total, Ipv6Addr>(bits)?;
        decodes_as_promised::<ZeroValid, Duration>(bits)?;
        decodes_as_promised::<ZeroValid, Option<Duration>>(bits)?;
        round_trips(Ipv6Addr::from_bits(bits))?;
        if let (Some(a), Some(b)) = (Duration::from_repr(bits), Duration::from_repr(other)) {
            round_trips(a)?;
            round_trips(Some(a))?;
            reprs_order_as_ord(a, b)?;
        }
        Ok(())
    }

    /// Every law on the low bits of `bits` as the repr of arrays of each width and validity.
    fn array_laws(bits: u128) -> Result<(), TestCaseError> {
        repr_laws!(bits;
            Total => [u8; 1], Total => [u8; 2], ZeroValid => [u8; 3], Total => [u8; 4],
            Total => [i8; 8], ZeroValid => [u16; 3], Total => [i32; 2],
            Total => [bool; 8], ZeroValid => [bool; 13], Total => [bool; 32],
            Total => [bool; 64], ZeroValid => [char; 2], ZeroNiche => [NonZero<u16>; 4],
            Total => [Option<NonZero<u8>>; 4], Partial => [RangedU8<1, 10>; 2],
            Partial => [RangedI8<-1, 1>; 4], Total => [[u8; 2]; 2],
            ZeroValid => Option<[u8; 3]>, ZeroValid => Option<[NonZero<u16>; 4]>,
            Partial => Packed<[RangedU8<0, 2>; 4], u8>, Partial => Packed<[RangedI8<-1, 1>; 5], u16>,
            ZeroValid => Packed<[bool; 13], u16>, ZeroNiche => Packed<[NonZero<u8>; 2], u32>,
            ZeroValid => Packed<[u8; 3], u32>,
        );
        #[cfg(wide)]
        repr_laws!(bits;
            Total => [u8; 16], ZeroValid => [u32; 3], ZeroValid => [bool; 100],
            Total => [u64; 2], ZeroValid => [char; 4],
        );
        Ok(())
    }

    /// Every law on the low bits of `bits` as the repr of tuples of each shape of bits and
    /// validity, and on the tuples they decode as.
    fn tuple_laws(bits: u128) -> Result<(), TestCaseError> {
        repr_laws!(bits;
            Total => (u8,), Total => (u8, u8), ZeroValid => (u8, u8, u8), Total => (i16, u16),
            ZeroValid => (bool, u8), ZeroValid => (u32, bool), Total => (u32, i32),
            ZeroNiche => (NonZero<u8>, u16), Partial => (RangedU8<1, 10>, u8),
            Partial => (u8, RangedI8<-1, 1>), ZeroValid => (char, bool),
            ZeroValid => Option<(u8, u8, u8)>,
        );
        #[cfg(wide)]
        repr_laws!(bits; Total => (u64, u64), ZeroValid => (u64, u32, u8));
        Ok(())
    }

    #[test]
    fn every_byte_obeys_the_laws() {
        for bits in 0..=u128::from(u8::MAX) {
            assert_holds(narrow_integer_laws(bits, bits.wrapping_add(1)));
            assert_holds(narrow_ranged_laws(bits, bits.wrapping_add(1)));
            assert_holds(array_laws(bits));
            assert_holds(tuple_laws(bits));
            assert_holds(core_value_laws(bits, bits.wrapping_add(1)));
            let byte = u8::from_bits(bits);
            assert_holds(decodes_as_promised::<ZeroValid, ()>(byte));
            assert_holds(decodes_as_promised::<ZeroValid, PhantomData<str>>(byte));
            assert_holds(decodes_as_promised::<ZeroValid, Option<()>>(byte));
            assert_holds(decodes_as_promised::<ZeroValid, Option<Option<()>>>(byte));
            assert_holds(decodes_as_promised::<ZeroValid, Option<PhantomData<str>>>(byte));
        }
        for value in [false, true] {
            assert_holds(decodes_as_promised::<Total, bool>(value));
            assert_holds(round_trips(value));
        }
        assert_holds(round_trips(()));
        assert_holds(round_trips(PhantomData::<str>));
        assert_holds(round_trips(Some(())));
        assert_holds(round_trips(None::<()>));
        assert_holds(round_trips(None::<Option<()>>));
    }

    #[test]
    fn every_repr_of_16_bits_obeys_the_ranged_laws() {
        for bits in 0..=u128::from(u16::MAX) {
            assert_holds(sixteen_bit_ranged_laws(bits, bits.wrapping_add(1)));
        }
    }

    #[test]
    fn every_edge_obeys_the_laws() {
        for bits in edges(&[CHAR_EDGES.as_slice(), &CORE_VALUE_EDGES].concat()) {
            assert_holds(narrow_integer_laws(bits, bits.wrapping_sub(1)));
            assert_holds(narrow_ranged_laws(bits, bits.wrapping_sub(1)));
            assert_holds(char_laws(bits, char::MAX));
            assert_holds(float_laws(bits));
            assert_holds(pointer_laws(bits));
            assert_holds(array_laws(bits));
            assert_holds(tuple_laws(bits));
            assert_holds(core_value_laws(bits, bits.wrapping_sub(1)));
            #[cfg(wide)]
            assert_holds(wide_laws(bits, bits.wrapping_sub(1)));
            #[cfg(wide)]
            assert_holds(wide_core_value_laws(bits, bits.wrapping_sub(1)));
        }
    }

    #[test]
    fn none_takes_a_spare_repr_of_each_built_in() {
        none_laws!(NonZero<u8>, NonZero<u16>, NonZero<u32>, NonZero<u64>, NonZero<usize>);
        none_laws!(NonZero<i8>, NonZero<i16>, NonZero<i32>, NonZero<i64>, NonZero<isize>);
        none_laws!(char, Option<char>, (), Option<()>, PhantomData<str>);
        none_laws!([u8; 3], [bool; 7], [NonZero<u8>; 4], [char; 2], [RangedU8<1, 10>; 2]);
        none_laws!((u8, u8, u8), (bool, bool), (NonZero<u8>, u16), (RangedU8<1, 10>, u8), (char,));
        none_laws!(Packed<[bool; 5], u8>, Packed<[NonZero<u8>; 2], u16>, Packed<[RangedU8<1, 3>; 4], u8>);
        none_laws!(RangedU8<1, 10>, RangedI8<-5, 5>, RangedI8<-100, -10>, RangedU16<3>);
        none_laws!(RangedI16<-300, 300>, RangedU32<7, 7>, RangedI32<{ i32::MIN }, -1>);
        none_laws!(RangedU64<3>, RangedI64<{ i64::MIN }, 5>, RangedUsize<1>, RangedIsize<-1, 1>);
        none_laws!(Ordering, SocketAddrV4, NonZero<char>, Reverse<NonZero<u8>>);
        none_laws!(Result<u32, u8>, ControlFlow<(), u8>, Bound<u16>, Alignment, AsciiChar, FpCategory);
        none_laws!(FormatAlignment);
        none_laws!(Poll<char>, Infallible);
        #[cfg(wide)]
        none_laws!(Duration, Layout, Result<u64, u64>);
        #[cfg(wide)]
        none_laws!(NonZero<u128>, NonZero<i128>);
        #[cfg(wide)]
        none_laws!(RangedU128<1>, RangedI128<{ i128::MIN }, -1>, RangedI128<-5, 5>);
        let null = None::<NonNull<u8>>.to_repr().addr();
        assert_holds(none_takes_a_spare_repr::<NonNull<u8>>(
            u128::try_from(null).expect("an address fits a `u128`"),
        ));
    }

    #[test]
    fn an_option_of_a_nonzero_or_non_null_is_total() {
        /// Compiles only where `T` declares every repr decodes.
        const fn is_total<T: Atom<Validity = Total>>() {}
        is_total::<Option<NonZero<u8>>>();
        is_total::<Option<NonZero<i64>>>();
        is_total::<Option<NonNull<u8>>>();
    }

    proptest! {
        #[test]
        fn integers_obey_the_laws(bits in edge_or_random_bits(&[]), other: u128) {
            narrow_integer_laws(bits, other)?;
        }

        #[test]
        fn ranged_integers_obey_the_laws(bits in edge_or_random_bits(&[]), other: u128) {
            narrow_ranged_laws(bits, other)?;
        }

        #[test]
        fn chars_obey_the_laws(
            bits in prop_oneof![edge_or_random_bits(&CHAR_EDGES), 0..=0x11_0001_u128], other: char,
        ) {
            char_laws(bits, other)?;
        }

        #[test]
        fn floats_keep_every_bit(bits in edge_or_random_bits(&[])) {
            float_laws(bits)?;
        }

        #[test]
        fn pointers_obey_the_laws(bits in edge_or_random_bits(&[])) {
            pointer_laws(bits)?;
        }

        #[test]
        fn arrays_obey_the_laws(bits in edge_or_random_bits(&[])) {
            array_laws(bits)?;
        }

        #[test]
        fn tuples_obey_the_laws(bits in edge_or_random_bits(&[])) {
            tuple_laws(bits)?;
        }

        #[test]
        fn references_and_function_pointers_obey_the_laws(bits in edge_or_random_bits(&[])) {
            sealed_laws(bits)?;
        }

        #[test]
        fn core_values_obey_the_laws(
            bits in edge_or_random_bits(&CORE_VALUE_EDGES), other in edge_or_random_bits(&[]),
        ) {
            core_value_laws(bits, other)?;
        }
    }

    #[cfg(wide)]
    proptest! {
        #[test]
        fn wide_values_obey_the_laws(bits in edge_or_random_bits(&[]), other: u128) {
            wide_laws(bits, other)?;
        }

        #[test]
        fn durations_obey_the_laws(a in duration_reprs(), b in duration_reprs()) {
            wide_core_value_laws(a, b)?;
        }

        #[test]
        fn wide_core_values_obey_the_laws(
            bits in edge_or_random_bits(&CORE_VALUE_EDGES), other in edge_or_random_bits(&CORE_VALUE_EDGES),
        ) {
            wide_core_value_laws(bits, other)?;
        }
    }
}
