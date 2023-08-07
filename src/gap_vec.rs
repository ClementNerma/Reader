pub struct GapVec<T> {
    items: Vec<Option<T>>,
}

impl<T> GapVec<T> {
    pub fn new(size: usize) -> Self {
        Self {
            items:
                // TODO: find a more proper syntax
                (0..size).map(|_| None).collect(),
        }
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        self.items
            .get(index)
            .expect("invalid index provided")
            .as_ref()
    }

    pub fn set(&mut self, index: usize, value: T) {
        self.items[index] = Some(value);
    }

    // pub fn get_or_insert_with(&mut self, index: usize, insert_with: impl FnOnce() -> T) -> &T {
    //     if self.items[index].is_none() {
    //         self.set(index, insert_with());
    //     }

    //     self.get(index).unwrap()
    // }

    // TODO: remove
}
