use core::iter::{repeat, Map, Repeat, Zip};
use core::ops::Range;

#[derive(Clone, Copy, PartialEq)]
#[repr(transparent)]
pub struct SmallIndex<const MAX: usize>(u16);
pub type SmallRange<const MAX: usize> = Map<Range<u16>, fn(u16) -> SmallIndex<MAX>>;
pub type PairsFor<const MAX: usize> = Zip<Repeat<SmallIndex<MAX>>, SmallRange<MAX>>;
pub type FnPairsFor<const MAX: usize> = fn(SmallIndex<MAX>) -> PairsFor<MAX>;
pub type Pairs<const MAX: usize> = Map<SmallRange<MAX>, FnPairsFor<MAX>>;
impl<const MAX: usize> SmallIndex<MAX> {
    #[inline]
    pub fn new(index: u16) -> Self {
        Self(index.min(MAX as u16 - 1))
    }
    #[inline]
    pub const fn new_unchecked(index: u16) -> Self {
        Self(index)
    }
    #[inline]
    pub fn raw(self) -> u16 {
        self.0
    }
    #[inline]
    pub const fn array<const N: usize>() -> [Self; N] {
        [Self(0); N]
    }
    #[inline]
    pub fn get<T: Sized + Copy>(self, a: &[T; MAX]) -> T {
        unsafe { *a.get_unchecked(self.0 as usize) }
    }
    #[inline]
    pub fn get_mut<T: Sized>(self, a: &mut [T; MAX]) -> &mut T {
        unsafe { a.get_unchecked_mut(self.0 as usize) }
    }
    #[inline]
    pub fn all() -> SmallRange<MAX> {
        (0..MAX as u16).map(Self::new_unchecked)
    }
    #[inline]
    fn priors(self) -> SmallRange<MAX> {
        (0..self.0).map(Self::new_unchecked)
    }
    #[inline]
    fn pairs_for(index: Self) -> PairsFor<MAX> {
        repeat(index).zip(index.priors())
    }
    #[inline]
    pub fn pairs() -> Pairs<MAX> {
        Self::all().map(Self::pairs_for as FnPairsFor<MAX>)
    }
}
macro_rules! mkshift {
    ($name:ident: $max:ident >> $shift:literal => $newmax:ident) => {
        impl SmallIndex<$max> {
            #[inline]
            pub fn $name(self) -> (SmallIndex<$newmax>, u16) {
                const _: [(); ($max >> $shift) + ($max != ($max >> $shift << $shift)) as usize] =
                    [(); $newmax];
                let mask = (1u16 << $shift) - 1;
                (SmallIndex(self.0 >> $shift), self.0 & mask)
            }
        }
    };
    ($name:ident: $max:ident << $shift:literal => $newmax:ident) => {
        impl SmallIndex<$max> {
            #[inline]
            pub fn $name(self, value: u16) -> SmallIndex<$newmax> {
                const _: [(); $max << $shift] = [(); $newmax];
                let mask = (1u16 << $shift) - 1;
                SmallIndex((self.0 << $shift) | (value & mask))
            }
        }
    };
}
use super::{UNITS, VALUES, VARS};
mkshift!(to_var: VALUES >> 1 => VARS);
mkshift!(raw_bit: VALUES >> 5 => UNITS);
mkshift!(raw_crumb: VARS >> 4 => UNITS);
mkshift!(from_var: VARS << 1 => VALUES);
