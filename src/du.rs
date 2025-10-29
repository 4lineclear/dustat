//! disk usage

use std::{
    error::Error,
    ffi::OsStr,
    num::NonZero,
    ops::{Index, IndexMut},
    path::PathBuf,
    time::{Duration, Instant},
};

pub mod mt;
pub mod st;

/// Disk Usage
#[derive(Debug, Default)]
pub struct Du<P>(Stats, P);

impl<P> Du<P> {
    pub fn new(provider: P) -> Self {
        Self(Stats::new(), provider)
    }
}

impl<P: DuSource> Du<P> {
    pub fn stats(&self) -> &Stats {
        &self.0
    }

    pub fn begin(&mut self, path: impl Into<PathBuf>) {
        self.1.enqueue(NodeId::ROOT, path.into());
        self.1.begin();
    }

    pub fn read_for(&mut self, dur: Duration) -> (usize, Duration) {
        // TODO: consider reading the elapsed time every n seconds
        let now = Instant::now();
        let count = self.read(&mut |_, _| now.elapsed() < dur);

        (count, now.elapsed())
    }

    pub fn read(&mut self, with: &mut impl FnMut(&mut Stats, &mut P) -> bool) -> usize {
        let Self(stats, provider) = self;

        let mut count = 0;
        while let Some(entry) = provider.next_entry()
            && with(stats, provider)
        {
            let is_dir = entry.info.kind == FileKind::Dir;
            let next = stats.push(entry.parent, entry.info);
            if is_dir {
                provider.enqueue(next, entry.path);
            }
            count += 1;
        }

        count
    }
}

pub trait DuSource {
    type Error: Error;

    fn begin(&mut self);
    fn finish(&mut self);

    fn next_entry(&mut self) -> Option<Entry>;
    fn enqueue(&mut self, parent: NodeId, path: PathBuf);

    fn errors(&self) -> &[Self::Error];
}

pub struct Entry {
    parent: NodeId,
    info: Info,
    path: PathBuf,
}

impl Entry {
    pub fn new(parent: NodeId, info: Info, path: PathBuf) -> Self {
        Self { parent, info, path }
    }
}

#[derive(Debug)]
pub struct Stats {
    nodes: Vec<Node>,
}

impl Default for Stats {
    fn default() -> Self {
        let nodes = vec![Node::new(Info::default(), NodeId::ROOT)];
        Self { nodes }
    }
}

impl Stats {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn head(&self) -> &Node {
        &self[NodeId::ROOT]
    }

    pub fn parent(&self, id: NodeId) -> &Node {
        &self[self[id].parent]
    }

    fn push(&mut self, parent: NodeId, info: Info) -> NodeId {
        let id = NodeId::new(self.nodes.len());
        self[parent].children.push(id);

        let mut p = parent;
        while p != self[p].parent {
            self[p].info.apply(&info);
            p = self[p].parent;
        }
        self[p].info.apply(&info);

        self.nodes.push(Node::new(info, parent));
        id
    }
}

impl Index<NodeId> for Stats {
    type Output = Node;

    fn index(&self, index: NodeId) -> &Self::Output {
        &self.nodes[index.get()]
    }
}

impl IndexMut<NodeId> for Stats {
    fn index_mut(&mut self, index: NodeId) -> &mut Self::Output {
        &mut self.nodes[index.get()]
    }
}

#[derive(Debug)]
pub struct Node {
    info: Info,
    parent: NodeId,
    children: Vec<NodeId>,
}

impl Node {
    pub fn new(info: Info, parent: NodeId) -> Self {
        Self {
            info,
            parent,
            children: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NodeId(NonZero<usize>);

impl NodeId {
    pub const ROOT: Self = Self::new(0);

    const fn new(id: usize) -> Self {
        Self(NonZero::new(id + 1).unwrap())
    }

    fn get(self) -> usize {
        self.0.get() - 1
    }
}

#[derive(Debug)]
pub struct Info {
    pub name: Box<OsStr>,
    pub kind: FileKind,
    pub size: u64,
    /// sub-files, includes self
    pub files: u32,
    /// sub-dirs, includes self
    pub dirs: u32,
    /// unknown sub-items, includes self
    pub other: u32,
}

impl Default for Info {
    fn default() -> Self {
        Self::new(OsStr::new(""), FileKind::Other, 0)
    }
}

impl Info {
    pub fn new(name: impl Into<Box<OsStr>>, kind: FileKind, size: u64) -> Self {
        Self {
            name: name.into(),
            kind,
            size,
            files: (kind == FileKind::File) as u32,
            dirs: (kind == FileKind::Dir) as u32,
            other: (kind == FileKind::Other) as u32,
        }
    }

    fn apply(&mut self, info: &Info) {
        self.size += info.size;
        self.files += info.files;
        self.dirs += info.dirs;
        self.other += info.other;
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum FileKind {
    Dir,
    File,
    #[default]
    Other,
}

impl From<std::fs::FileType> for FileKind {
    fn from(value: std::fs::FileType) -> Self {
        if value.is_dir() {
            Self::Dir
        } else if value.is_file() {
            Self::File
        } else {
            Self::Other
        }
    }
}
