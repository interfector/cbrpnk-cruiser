use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::fs::entry::{sort_entries, Entry};
use crate::util::is_hidden_name;

#[derive(Debug, Clone)]
pub struct ScanRequest {
    pub path: PathBuf,
    pub generation: u64,
    pub show_hidden: bool,
    pub max_entries: usize,
}

#[derive(Debug, Clone)]
pub struct ScanResult {
    pub path: PathBuf,
    pub canonical_path: Option<PathBuf>,
    pub generation: u64,
    pub entries: Vec<Entry>,
    pub total_entries: usize,
    pub visible_entries: usize,
    pub hidden_filtered: usize,
    pub entry_errors: usize,
    pub truncated: bool,
    pub error: Option<String>,
    pub duration: Duration,
}

pub fn scan_directory(request: &ScanRequest) -> ScanResult {
    let started = Instant::now();
    let canonical_path = std::fs::canonicalize(&request.path).ok();
    let mut entries = Vec::new();
    let mut total_entries = 0usize;
    let mut hidden_filtered = 0usize;
    let mut entry_errors = 0usize;
    let mut error = None;

    match std::fs::read_dir(&request.path) {
        Ok(read_dir) => {
            for item in read_dir {
                let item = match item {
                    Ok(item) => item,
                    Err(err) => {
                        entry_errors += 1;
                        error.get_or_insert_with(|| err.to_string());
                        continue;
                    }
                };
                let name = item.file_name();
                if name == "." || name == ".." {
                    continue;
                }
                total_entries += 1;
                if !request.show_hidden && is_hidden_name(&name) {
                    hidden_filtered += 1;
                    continue;
                }
                let entry = Entry::from_dir_entry(&item);
                if entry.metadata_error.is_some() {
                    entry_errors += 1;
                }
                entries.push(entry);
            }
        }
        Err(err) => {
            error = Some(err.to_string());
        }
    }

    sort_entries(&mut entries);
    let visible_entries = entries.len();
    let truncated = visible_entries > request.max_entries;
    if truncated {
        entries.truncate(request.max_entries);
    }

    ScanResult {
        path: request.path.clone(),
        canonical_path,
        generation: request.generation,
        entries,
        total_entries,
        visible_entries,
        hidden_filtered,
        entry_errors,
        truncated,
        error,
        duration: started.elapsed(),
    }
}

pub struct ScanWorker {
    sender: Sender<ScanRequest>,
    receiver: Receiver<ScanResult>,
    handle: Option<JoinHandle<()>>,
}

impl ScanWorker {
    pub fn spawn() -> Self {
        let (request_tx, request_rx) = mpsc::channel::<ScanRequest>();
        let (result_tx, result_rx) = mpsc::channel::<ScanResult>();
        let handle = thread::Builder::new()
            .name("cbrpnk-scan".to_string())
            .spawn(move || {
                while let Ok(request) = request_rx.recv() {
                    let result = scan_directory(&request);
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

    pub fn request(&self, request: ScanRequest) {
        let _ = self.sender.send(request);
    }

    pub fn try_recv(&self) -> Option<ScanResult> {
        self.receiver.try_recv().ok()
    }
}

impl Drop for ScanWorker {
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
    use std::ffi::OsString;
    use std::fs;
    use std::path::Path;
    use std::time::SystemTime;

    fn request(path: &Path) -> ScanRequest {
        ScanRequest {
            path: path.to_path_buf(),
            generation: 1,
            show_hidden: false,
            max_entries: 100,
        }
    }

    #[test]
    fn scans_and_sorts() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("beta"), b"b").unwrap();
        fs::write(dir.path().join("alpha"), b"a").unwrap();
        fs::create_dir(dir.path().join("zdir")).unwrap();
        fs::write(dir.path().join(".hidden"), b"h").unwrap();

        let result = scan_directory(&request(dir.path()));
        assert!(result.error.is_none());
        assert_eq!(result.total_entries, 4);
        assert_eq!(result.hidden_filtered, 1);
        assert_eq!(result.entries.len(), 3);
        assert_eq!(result.entries[0].display_name(), "zdir");
        assert_eq!(result.entries[1].display_name(), "alpha");
        assert_eq!(result.entries[2].display_name(), "beta");
    }

    #[test]
    fn shows_hidden_when_requested() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".hidden"), b"h").unwrap();
        let mut req = request(dir.path());
        req.show_hidden = true;
        let result = scan_directory(&req);
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.hidden_filtered, 0);
    }

    #[test]
    fn caps_entries() {
        let dir = tempfile::tempdir().unwrap();
        for index in 0..8 {
            fs::write(dir.path().join(format!("f{index}")), b"x").unwrap();
        }
        let mut req = request(dir.path());
        req.max_entries = 3;
        let result = scan_directory(&req);
        assert_eq!(result.entries.len(), 3);
        assert!(result.truncated);
        assert_eq!(result.visible_entries, 8);
    }

    #[test]
    fn missing_directory_reports_error() {
        let dir = tempfile::tempdir().unwrap();
        let result = scan_directory(&request(&dir.path().join("missing")));
        assert!(result.error.is_some());
        assert!(result.entries.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn keeps_non_utf8_names() {
        use std::os::unix::ffi::OsStringExt;
        let dir = tempfile::tempdir().unwrap();
        let name = OsString::from_vec(vec![b'f', 0xff, b'x']);
        fs::write(dir.path().join(&name), b"x").unwrap();
        let result = scan_directory(&request(dir.path()));
        assert_eq!(result.entries.len(), 1);
        assert_eq!(result.entries[0].name, name);
    }

    #[cfg(unix)]
    #[test]
    fn records_symlinks_without_following_for_metadata() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("target")).unwrap();
        symlink("target", dir.path().join("portal")).unwrap();
        let result = scan_directory(&request(dir.path()));
        let portal = result
            .entries
            .iter()
            .find(|entry| entry.display_name() == "portal")
            .unwrap();
        assert_eq!(portal.kind, crate::fs::EntryKind::Symlink);
        assert_eq!(portal.link_target_is_dir, Some(true));
        assert!(!portal.link_broken);
    }

    #[test]
    fn worker_returns_result() {
        let dir = tempfile::tempdir().unwrap();
        let worker = ScanWorker::spawn();
        worker.request(request(dir.path()));
        let deadline = SystemTime::now() + Duration::from_secs(2);
        loop {
            if let Some(result) = worker.try_recv() {
                assert_eq!(result.path, dir.path());
                break;
            }
            assert!(SystemTime::now() < deadline);
            thread::sleep(Duration::from_millis(10));
        }
    }
}
