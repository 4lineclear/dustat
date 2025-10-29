use std::{
    collections::VecDeque,
    io::{Error, ErrorKind, Result},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed},
        mpsc,
    },
    thread,
};

use crate::{
    du::{DuSource, Entry, NodeId},
    util,
};

type Handle<T> = Arc<Mutex<T>>;
type TaskHandle = Handle<VecDeque<(NodeId, PathBuf)>>;

pub struct Source {
    running: AtomicBool,
    tasks: TaskHandle,

    tx_entries: mpsc::Sender<Result<Entry>>,
    rx_entries: mpsc::Receiver<Result<Entry>>,

    errors: Vec<Error>,
}

impl Default for Source {
    fn default() -> Self {
        let (tx_entries, rx_entries) = mpsc::channel();

        Self {
            running: AtomicBool::new(false),
            tasks: TaskHandle::default(),
            tx_entries,
            rx_entries,
            errors: Vec::new(),
        }
    }
}

impl Source {
    fn handle_err<T>(&mut self, res: Result<T>) -> Option<T> {
        match res {
            Ok(value) => Some(value),
            Err(e) => {
                self.errors.push(e);
                None
            }
        }
    }
}

impl DuSource for Source {
    type Error = Error;

    fn begin(&mut self) {
        let parallelism = thread::available_parallelism().map_or(1, |n| n.get());
        let threads = AtomicUsize::new(0);
        let running = &self.running;
        let tasks = &self.tasks;
        let entries = &self.tx_entries;

        self.running.store(true, Relaxed);
        thread::scope(|s| {
            for _ in 0..parallelism {
                s.spawn(|| {
                    run_thread(&threads, running, tasks, entries);
                });
            }
        });
        self.running.store(false, Relaxed);
    }

    fn finish(&mut self) {
        self.running.store(false, Relaxed);
    }

    fn next_entry(&mut self) -> Option<Entry> {
        self.handle_err(self.rx_entries.try_recv().unwrap())
    }

    fn enqueue(&mut self, parent: NodeId, path: PathBuf) {
        self.tasks.lock().unwrap().push_back((parent, path));
    }

    fn errors(&self) -> &[Self::Error] {
        &self.errors
    }
}

fn run_thread(
    threads: &AtomicUsize,
    running: &AtomicBool,
    tasks: &TaskHandle,
    entries: &mpsc::Sender<Result<Entry>>,
) {
    while running.load(Relaxed) {
        let Some((parent, path)) = tasks.lock().unwrap().pop_front() else {
            if threads.load(Relaxed) == 0 {
                break;
            }
            thread::yield_now();
            continue;
        };

        threads.fetch_add(1, Relaxed);
        util::read_dir(
            parent,
            &path,
            |e| entries.send(Ok(e)).unwrap(),
            |e| entries.send(Err(e)).unwrap(),
        );
        threads.fetch_sub(1, Relaxed);
    }
}
