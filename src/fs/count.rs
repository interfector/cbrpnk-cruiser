use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

const COUNT_LIMIT: usize = 2048;

#[derive(Debug, Clone)]
pub struct CountRequest {
    pub paths: Vec<PathBuf>,
    pub generation: u64,
}

#[derive(Debug, Clone, Default)]
pub struct CountResult {
    pub generation: u64,
    pub counts: HashMap<PathBuf, usize>,
}

pub fn count_children(path: &Path) -> usize {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .filter(|entry| {
            let name = entry.file_name();
            name != "." && name != ".."
        })
        .take(COUNT_LIMIT)
        .count()
}

pub fn count_directories(request: &CountRequest) -> CountResult {
    let mut counts = HashMap::with_capacity(request.paths.len());
    for path in &request.paths {
        counts.insert(path.clone(), count_children(path));
    }
    CountResult {
        generation: request.generation,
        counts,
    }
}

pub struct CountWorker {
    sender: Sender<CountRequest>,
    receiver: Receiver<CountResult>,
    handle: Option<JoinHandle<()>>,
}

impl CountWorker {
    pub fn spawn() -> Self {
        let (request_tx, request_rx) = mpsc::channel::<CountRequest>();
        let (result_tx, result_rx) = mpsc::channel::<CountResult>();
        let handle = thread::Builder::new()
            .name("cbrpnk-count".to_string())
            .spawn(move || {
                while let Ok(request) = request_rx.recv() {
                    let result = count_directories(&request);
                    if result_tx.send(result).is_err() {
                        break;
                    }
                }
            })
            .ok();
        Self {
            sender: request_tx,
            receiver: result_rx,
            handle,
        }
    }

    pub fn request(&self, request: CountRequest) {
        let _ = self.sender.send(request);
    }

    pub fn try_recv(&self) -> Option<CountResult> {
        self.receiver.try_recv().ok()
    }
}

impl Drop for CountWorker {
    fn drop(&mut self) {
        let (tx, _rx) = mpsc::channel();
        drop(std::mem::replace(&mut self.sender, tx));
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    #[test]
    fn counts_entries_including_hidden_and_directories() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("one"), b"1").unwrap();
        fs::write(dir.path().join(".hidden"), b"h").unwrap();
        fs::create_dir(dir.path().join("nested")).unwrap();
        assert_eq!(count_children(dir.path()), 3);
    }

    #[test]
    fn missing_directories_count_as_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(count_children(&dir.path().join("missing")), 0);
    }

    #[test]
    fn worker_batches_counts() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("one"), b"1").unwrap();
        fs::write(dir.path().join("two"), b"2").unwrap();
        let worker = CountWorker::spawn();
        worker.request(CountRequest {
            paths: vec![
                dir.path().to_path_buf(),
                PathBuf::from("/definitely/missing"),
            ],
            generation: 7,
        });
        let result = loop {
            if let Some(result) = worker.try_recv() {
                break result;
            }
            std::thread::yield_now();
        };
        assert_eq!(result.generation, 7);
        assert_eq!(result.counts.get(dir.path()), Some(&2));
        assert_eq!(
            result.counts.get(Path::new("/definitely/missing")),
            Some(&0)
        );
    }
}
