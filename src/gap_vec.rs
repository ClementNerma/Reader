/// A fixed-size Vec<T> with gaps (meaning some indexes may not have a value)
/// Useful for representing a list of loading values that's filled progressively
pub struct GapVec<T> {
    items: Vec<Option<T>>,
}

impl<T> GapVec<T> {
    /// Create a gap vec with a fixed size
    pub fn new(size: usize) -> Self {
        Self {
            items:
                // TODO: find a more proper syntax
                (0..size).map(|_| None).collect(),
        }
    }

    /// Get the value at the provided index
    /// Panics if the index does not exist
    pub fn get(&self, index: usize) -> Option<&T> {
        self.items
            .get(index)
            .expect("invalid index provided")
            .as_ref()
    }

    /// Set the value at a provided index
    /// Panics if the index does not exist
    pub fn set(&mut self, index: usize, value: T) {
        self.items[index] = Some(value);
    }
}
