pub struct True;
pub struct False;

pub trait MaybeType<T> {
    type Type;
}

impl<T> MaybeType<T> for True {
    type Type = T;
}

impl<T> MaybeType<T> for False {
    type Type = ();
}
