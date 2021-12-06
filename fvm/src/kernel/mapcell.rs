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

    /// Constructs an empty MapCell to be filled in later.
    ///
    /// WARNING: Calling any other method on the MapCell and/or dereferencing it will _panic_.
    pub fn empty() -> Self {
        MapCell(None)
    }

    /// Returns whether this MapCell is empty.
    pub fn is_empty(&self) -> bool {
        self.0.is_none()
    }

    /// Set the MapCell value.
    pub fn set(&mut self, item: T) {
        self.0 = Some(item)
    }

    /// Map over the MapCell value, temporarily removing it and replacing it.
    ///
    /// If the inner function panics, the MapCell will be poisoned.
    pub fn map<F>(&mut self, f: F)
    where
        F: FnOnce(T) -> T,
    {
        self.0 = Some(f(self.0.take().expect("MapCell empty")));
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

fn main() {
    let mut cell = MapCell::new(String::new());
    cell.map(|mut s| {
        s.push_str("foo");
        s
    });
}
