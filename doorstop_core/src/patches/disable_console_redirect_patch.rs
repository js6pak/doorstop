use log::trace;
use plthook::ObjectFile;

use crate::plt_hook;

pub(super) fn patch(object: &ObjectFile) -> anyhow::Result<()> {
    #[cfg(windows)]
    #[allow(static_mut_refs)]
    {
        use std::mem::MaybeUninit;

        use log::warn;
        use windows::Win32::{
            Foundation::{CompareObjectHandles, HANDLE, TRUE},
            System::Console::{GetStdHandle, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE},
        };

        static mut STD_HANDLES: MaybeUninit<(HANDLE, HANDLE)> = MaybeUninit::uninit();

        let stdout_handle = match unsafe { GetStdHandle(STD_OUTPUT_HANDLE) } {
            Ok(stdout_handle) => stdout_handle,
            Err(e) => {
                warn!("Failed to get stdout handle: {e}");
                return Ok(());
            }
        };

        let stderr_handle = match unsafe { GetStdHandle(STD_ERROR_HANDLE) } {
            Ok(stderr_handle) => stderr_handle,
            Err(e) => {
                warn!("Failed to get stderr handle: {e}");
                return Ok(());
            }
        };

        unsafe {
            STD_HANDLES.write((stdout_handle, stderr_handle));
        }

        plt_hook!(&object, "CloseHandle", extern "system" fn(orig, hobject: HANDLE) -> i32, {
            if unsafe { CompareObjectHandles(hobject, STD_HANDLES.assume_init_ref().0) } == TRUE {
                trace!("Preventing stdout close");
                return 1;
            }

            if unsafe { CompareObjectHandles(hobject, STD_HANDLES.assume_init_ref().1) } == TRUE {
                trace!("Preventing stderr close");
                return 1;
            }

            unsafe { orig(hobject) }
        })?;

        Ok(())
    }

    #[cfg(unix)]
    {
        use libc::{F_OK, FILE, STDERR_FILENO, STDOUT_FILENO};

        plt_hook!(&object, "dup2", extern "system" fn(orig, oldfd: i32, newfd: i32) -> i32, {
            if newfd == STDOUT_FILENO {
                trace!("Preventing stdout redirect");
                return F_OK;
            }

            if newfd == STDERR_FILENO {
                trace!("Preventing stderr redirect");
                return F_OK;
            }

            unsafe { orig(oldfd, newfd) }
        })?;

        plt_hook!(&object, "fclose", extern "system" fn(orig, file: *mut FILE) -> i32, {
            unsafe extern "C" {
                #[cfg_attr(target_os = "macos", link_name = "__stdoutp")]
                static stdout: *mut FILE;
                #[cfg_attr(target_os = "macos", link_name = "__stderrp")]
                static stderr: *mut FILE;
            }

            if file == unsafe { stdout } {
                trace!("Preventing stdout close");
                return F_OK;
            }

            if file == unsafe { stderr } {
                trace!("Preventing stderr close");
                return F_OK;
            }

            unsafe { orig(file) }
        })?;

        Ok(())
    }
}
