use std::ffi::OsStr;
use std::io::Error;
use std::io::ErrorKind;
use std::io::Result;
use std::path::Path;

use crate::du::{Entry, FileKind, Info, NodeId};

pub fn get_name(path: &Path) -> Result<&OsStr> {
    match path.components().next_back() {
        Some(name) => Ok(name.as_os_str()),
        None => Err(ErrorKind::InvalidFilename.into()),
    }
}

pub fn read_dir(
    parent: NodeId,
    path: &Path,
    mut entry: impl FnMut(Entry),
    mut error: impl FnMut(Error),
) {
    macro_rules! handle {
        ($e:expr) => {
            match $e {
                Ok(value) => value,
                Err(e) => {
                    error(e);
                    return;
                }
            }
        };
    }

    for value in handle!(std::fs::read_dir(path)) {
        let de = handle!(value);
        let md = handle!(de.metadata());
        let path = de.path();

        // TODO: consider just ignoring invalid file names.
        let name = handle!(get_name(&path));
        let info = Info::new(name, FileKind::from(md.file_type()), md.len());

        entry(Entry::new(parent, info, path));
    }
}
