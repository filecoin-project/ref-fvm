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
pub trait Select<A, B> {
    type Type;
}

impl<A, B> Select<A, B> for True {
    type Type = A;
}

impl<A, B> Select<A, B> for False {
    type Type = B;
}
