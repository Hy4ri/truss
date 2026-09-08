use std::ffi::{CStr, CString, OsString};
use std::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use tracing::warn;

/// File watcher using Linux inotify to monitor a config file for changes.
pub struct ConfigWatcher {
    inotify_fd: OwnedFd,
    _watch_dir: Option<i32>,
    watch_file: Option<i32>,
    config_path: PathBuf,
    filename: OsString,
}

impl ConfigWatcher {
    /// Initialize inotify watch on the configuration file and its parent directory.
    pub fn new(config_path: &Path) -> Result<Self, std::io::Error> {
        let abs_path = config_path
            .canonicalize()
            .unwrap_or_else(|_| config_path.to_path_buf());
        let filename = abs_path
            .file_name()
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "Config path has no valid filename",
                )
            })?
            .to_os_string();

        let parent = abs_path.parent().unwrap_or_else(|| Path::new("."));

        let raw_fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        if raw_fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        let inotify_fd = unsafe { OwnedFd::from_raw_fd(raw_fd) };

        let mut watch_dir = None;
        if let Ok(c_parent) = CString::new(parent.as_os_str().as_bytes()) {
            let wd = unsafe {
                libc::inotify_add_watch(
                    inotify_fd.as_raw_fd(),
                    c_parent.as_ptr(),
                    libc::IN_CLOSE_WRITE | libc::IN_MOVED_TO | libc::IN_CREATE,
                )
            };
            if wd >= 0 {
                watch_dir = Some(wd);
            } else {
                warn!(
                    "truss: failed to watch config directory {}: {}",
                    parent.display(),
                    std::io::Error::last_os_error()
                );
            }
        }

        let mut watch_file = None;
        if abs_path.exists() {
            if let Ok(c_file) = CString::new(abs_path.as_os_str().as_bytes()) {
                let wd = unsafe {
                    libc::inotify_add_watch(
                        inotify_fd.as_raw_fd(),
                        c_file.as_ptr(),
                        libc::IN_CLOSE_WRITE | libc::IN_MODIFY,
                    )
                };
                if wd >= 0 {
                    watch_file = Some(wd);
                }
            }
        }

        Ok(Self {
            inotify_fd,
            _watch_dir: watch_dir,
            watch_file,
            config_path: abs_path,
            filename,
        })
    }

    /// Re-adds watch on the file itself if it was re-created/moved atomically.
    pub fn refresh_file_watch(&mut self) {
        if self.config_path.exists() {
            if let Ok(c_file) = CString::new(self.config_path.as_os_str().as_bytes()) {
                let wd = unsafe {
                    libc::inotify_add_watch(
                        self.inotify_fd.as_raw_fd(),
                        c_file.as_ptr(),
                        libc::IN_CLOSE_WRITE | libc::IN_MODIFY,
                    )
                };
                if wd >= 0 {
                    self.watch_file = Some(wd);
                }
            }
        }
    }

    /// Reads all pending events from inotify queue.
    /// Returns `true` if any event affected the target config file.
    pub fn check_events(&mut self) -> bool {
        let mut matched = false;
        let mut buffer = [0u8; 4096];

        loop {
            let res = unsafe {
                libc::read(
                    self.inotify_fd.as_raw_fd(),
                    buffer.as_mut_ptr() as *mut libc::c_void,
                    buffer.len(),
                )
            };

            if res <= 0 {
                break;
            }

            let bytes_read = res as usize;
            let mut offset = 0;
            let event_size = std::mem::size_of::<libc::inotify_event>();

            while offset + event_size <= bytes_read {
                let event =
                    unsafe { &*(buffer.as_ptr().add(offset) as *const libc::inotify_event) };

                if event.len > 0 {
                    let name_ptr = unsafe { buffer.as_ptr().add(offset + event_size) };
                    let name_cstr = unsafe { CStr::from_ptr(name_ptr as *const libc::c_char) };
                    if name_cstr.to_bytes() == self.filename.as_os_str().as_bytes() {
                        matched = true;
                    }
                } else if Some(event.wd) == self.watch_file {
                    matched = true;
                }

                offset += event_size + event.len as usize;
            }
        }

        if matched {
            self.refresh_file_watch();
        }

        matched
    }

    /// Returns the raw inotify file descriptor.
    pub fn raw_fd(&self) -> RawFd {
        self.inotify_fd.as_raw_fd()
    }
}

impl AsFd for ConfigWatcher {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.inotify_fd.as_fd()
    }
}

impl AsRawFd for ConfigWatcher {
    fn as_raw_fd(&self) -> RawFd {
        self.inotify_fd.as_raw_fd()
    }
}
