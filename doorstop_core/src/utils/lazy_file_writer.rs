use std::{
    fs,
    fs::{File, OpenOptions, TryLockError},
    io::{self, Write},
    path::PathBuf,
    sync::Mutex,
};

pub(crate) struct LazyFileWriter {
    path: PathBuf,
    file: Mutex<Option<File>>,
}

impl LazyFileWriter {
    pub(crate) fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            file: Mutex::new(None),
        }
    }
}

impl Write for LazyFileWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut file_guard = self.file.lock().unwrap();

        if file_guard.is_none() {
            if let Some(parent) = self.path.parent() {
                fs::create_dir_all(parent)?;
            }

            let mut attempt = 0;

            loop {
                let path = if attempt == 0 {
                    &self.path
                } else {
                    &self.path.with_added_extension(attempt.to_string())
                };

                let file = OpenOptions::new().create(true).write(true).truncate(false).open(path)?;
                match file.try_lock() {
                    Ok(()) => {}
                    Err(TryLockError::WouldBlock) => {
                        attempt += 1;
                        continue;
                    }
                    Err(TryLockError::Error(e)) => return Err(e),
                }
                file.set_len(0)?;

                *file_guard = Some(file);
                break;
            }
        }

        file_guard.as_mut().unwrap().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut file_guard = self.file.lock().unwrap();
        match file_guard.as_mut() {
            Some(file) => file.flush(),
            None => Ok(()),
        }
    }
}
