use std::ops::{Deref, DerefMut};

/// A MapCell<T> is a convenient container holding a type T which may or may
/// not be set.
#[repr(transparent)]
pub struct MapCell<T>(Option<T>);

impl<T> MapCell<T> {
    /// Constructs a new MapCell.
    pub fn new(item: T) -> Self {
        MapCell(Some(item))
    }

    /// Apply over the MapCell value, temporarily removing it and replacing it.
    ///
    /// If the inner function panics, the MapCell will be poisoned.
    pub fn map_mut<F, R>(&mut self, f: F) -> R
    where
        F: FnOnce(T) -> (T, R),
    {
        let (next, r) = f(self.0.take().expect("MapCell empty"));
        self.0 = Some(next);
        r
    }

    /// Destructively take the MapCell value
    pub fn take(self) -> T {
        self.0.unwrap()
    }
}

impl<T> Deref for MapCell<T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.0.as_ref().expect("MapCell empty")
    }
}

impl<T> DerefMut for MapCell<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.as_mut().expect("MapCell empty")
    }
}
