/// Ensures only a single instance of doorstop is executed in a process.
/// Mutexes/tmpfiles are used instead of environment variables to allow injecting into descendant processes.
#[cfg_attr(windows, allow(clippy::unnecessary_wraps))]
pub(crate) fn ensure_single_instance() -> anyhow::Result<bool> {
    #[cfg(windows)]
    unsafe {
        use windows::{
            Win32::{
                Foundation::{ERROR_ALREADY_EXISTS, GetLastError},
                System::Threading::CreateMutexW,
            },
            core::HSTRING,
        };

        let mutex_name = HSTRING::from(format!("Local\\doorstop-{}", std::process::id()));
        _ = CreateMutexW(None, false, &mutex_name);
        Ok(GetLastError() != ERROR_ALREADY_EXISTS)
    }

    #[cfg(unix)]
    {
        use std::{
            fs,
            fs::{File, OpenOptions, TryLockError},
            path::PathBuf,
        };

        use anyhow::bail;
        use dtor::dtor;

        let path = PathBuf::from(format!("/tmp/doorstop-{}", std::process::id()));
        let file = OpenOptions::new().create(true).truncate(false).write(true).open(&path)?;

        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Ok(false);
            }
            Err(TryLockError::Error(e)) => bail!(e),
        }

        #[allow(static_mut_refs)]
        {
            static mut FILE: Option<(PathBuf, File)> = None;
            unsafe {
                FILE = Some((path, file));
            }

            #[dtor]
            unsafe fn dtor() {
                unsafe {
                    if let Some((path, file)) = FILE.take() {
                        drop(file);
                        _ = fs::remove_file(path);
                    }
                }
            }
        }

        Ok(true)
    }
}
