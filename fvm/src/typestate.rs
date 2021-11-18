use std::ops::{Deref, DerefMut};

// Ideally we'd use const generics here, but rust isn't quite ready for that.

/// True is a type-level true used in conditionals.
pub struct True;

/// False is a type-level false used in conditionals.
pub struct False;

/// Select selects between A/B as follows:
///
/// ```ignore
/// let value: <True as Select<i32, bool>>::Type = 1;
/// ```
pub trait TypeOption<A>: Sized {
    type Container: Container<Self, A>;
}

#[doc(hidden)]
pub trait Container<T: TypeOption<A>, A>: Sized {
    fn as_option(&self) -> Option<&A>;
    fn as_option_mut(&mut self) -> Option<&mut A>;
    fn into_option(self) -> Option<A>;
}

impl<A> Container<True, A> for A {
    #[inline(always)]
    fn as_option(&self) -> Option<&A> {
        Some(self)
    }
    #[inline(always)]
    fn as_option_mut(&mut self) -> Option<&mut A> {
        Some(self)
    }
    #[inline(always)]
    fn into_option(self) -> Option<A> {
        Some(self)
    }
}

impl<A> Container<False, A> for () {
    #[inline(always)]
    fn as_option(&self) -> Option<&A> {
        None
    }
    #[inline(always)]
    fn as_option_mut(&mut self) -> Option<&mut A> {
        None
    }
    #[inline(always)]
    fn into_option(self) -> Option<A> {
        None
    }
}

#[repr(transparent)]
pub struct ConstOption<T: TypeOption<A>, A>(<T as TypeOption<A>>::Container);

impl<T, A> ConstOption<T, A>
where
    T: TypeOption<A>,
{
    #[inline(always)]
    pub fn as_option(&self) -> Option<&A> {
        self.0.as_option()
    }

    #[inline(always)]
    pub fn as_option_mut(&mut self) -> Option<&mut A> {
        self.0.as_option_mut()
    }

    #[inline(always)]
    pub fn into_option(self) -> Option<A> {
        self.0.into_option()
    }
}

impl<A> ConstOption<True, A> {
    pub fn some(inner: A) -> Self {
        ConstOption(inner)
    }
}

impl<A> ConstOption<False, A> {
    pub fn none() -> Self {
        ConstOption(())
    }
}

impl<A> Deref for ConstOption<True, A> {
    type Target = A;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<A> DerefMut for ConstOption<True, A> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl<A> TypeOption<A> for True {
    type Container = A;
}

impl<A> TypeOption<A> for False {
    type Container = ();
}
