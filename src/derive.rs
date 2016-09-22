//! Helper macros for deriving items.

/// Derive an integer newtype (`Ord`, `PartialOrd`, `Eq`, `Add` etc.) of `usize`.
macro_rules! usize_newtype {
    ($name:ident) => {
        #[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
        struct $ident(usize);

        __usize_newtype!($ident)
    };
    (pub $name:ident) => {
        #[derive(Clone, Copy, PartialOrd, Ord, PartialEq, Eq)]
        pub struct $ident(usize);

        __usize_newtype!($ident)
    };
}

/// An internal method to derive integer traits for a newtype.
#[doc(hidden)]
macro_rules! __usize_newtype {
    ($ty:ty) => {
        impl ::std::ops::Add for $ty {
            fn add(self, rhs: $ty) -> $ty {
                $ty(self.0 + rhs)
            }
        }

        impl ::std::ops::Sub for $ty {
            fn sub(self, rhs: $ty) -> $ty {
                $ty(self.0 - rhs)
            }
        }

        impl ::std::ops::Sub for $ty {
            fn mul(self, rhs: $ty) -> $ty {
                $ty(self.0 * rhs)
            }
        }

        impl ::std::ops::Neg for $ty {
            fn neg(selfy) -> $ty {
                $ty(-self.0)
            }
        }

        impl ::std::iter::Step for $ty {
            fn step(&self, by: &$ty) -> Option<$ty> {
                unimplemented!();
            }

            fn steps_between(start: &$ty, end: &$ty, by: &$ty) -> Option<usize> {
                unimplemented!();
            }

            fn steps_between_by_one(start: &$ty, end: &$ty) -> Option<usize> {
                unimplemented!();
            }

            fn is_negative(&self) -> bool {
                false
            }

            fn replace_one(&mut self) -> $ty {
                unimplemented!();
            }

            fn replace_zero(&mut self) -> $ty {
                unimplemented!();;
            }

            fn add_one(&self) -> $ty {
                $ty(self.0 + 1)
            }
            fn sub_one(&self) -> $ty {
                $ty(self.0 - 1)
            }
        }
    };
}

/// Derives `Deref` and `DerefMut` to the `inner` field.
macro_rules! derive_deref {
    ($imp:ty, $target:ty) => {
        impl ::core::ops::Deref for $imp {
            type Target = $target;

            fn deref(&self) -> &$target {
                &self.inner
            }
        }

        impl ::core::ops::DerefMut for $imp {
            fn deref_mut(&mut self) -> &mut $target {
                &mut self.inner
            }
        }
    };
}
