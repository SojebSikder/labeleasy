//! Bounded snapshot-based undo/redo, generic over the state being edited.

use std::collections::VecDeque;

#[derive(Debug, Clone)]
pub struct History<T> {
    undo: VecDeque<T>,
    redo: Vec<T>,
    capacity: usize,
}

impl<T> History<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            capacity: capacity.max(1),
        }
    }

    /// records `before` (the state prior to an edit) and forgets the redo branch.
    pub fn record(&mut self, before: T) {
        if self.undo.len() == self.capacity {
            self.undo.pop_front();
        }
        self.undo.push_back(before);
        self.redo.clear();
    }

    /// swaps `current` for the previous state; `false` if there is none.
    pub fn undo(&mut self, current: &mut T) -> bool {
        let Some(previous) = self.undo.pop_back() else {
            return false;
        };
        self.redo.push(std::mem::replace(current, previous));
        true
    }

    pub fn redo(&mut self, current: &mut T) -> bool {
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push_back(std::mem::replace(current, next));
        true
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}
