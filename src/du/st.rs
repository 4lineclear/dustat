use std::{io::Error, path::PathBuf};

use crate::{
    du::{DuSource, Entry, NodeId},
    util,
};

#[derive(Default)]
pub struct Source {
    entries: Vec<Entry>,
    errors: Vec<Error>,
}

impl DuSource for Source {
    type Error = Error;

    fn begin(&mut self) {}
    fn finish(&mut self) {}

    fn next_entry(&mut self) -> Option<Entry> {
        self.entries.pop()
    }

    fn enqueue(&mut self, parent: NodeId, path: PathBuf) {
        util::read_dir(
            parent,
            &path,
            |e| self.entries.push(e),
            |e| self.errors.push(e),
        );
    }

    fn errors(&self) -> &[Self::Error] {
        &self.errors
    }
}
