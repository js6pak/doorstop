use std::path::{Path, PathBuf};

use doorstop_shared::OsStrExt;
use log::info;
use plthook::ObjectFile;

use crate::{get_config, plt_hook};

fn try_override(path: &Path) -> Option<&PathBuf> {
    if let Some(file_name) = path.file_name()
        && file_name == "boot.config"
        && let Some(new_path) = get_config().boot_config_override.as_ref()
    {
        info!("Overriding boot.config to {}", new_path.display());
        return Some(new_path);
    }

    None
}

pub(super) fn patch(object: &ObjectFile) -> anyhow::Result<()> {
    if get_config().boot_config_override.is_none() {
        return Ok(());
    }

    #[cfg(windows)]
    {
        use std::{ffi::OsString, os::windows::ffi::OsStringExt};

        use windows::{
            Win32::{
                Foundation::HANDLE,
                Security::SECURITY_ATTRIBUTES,
                Storage::FileSystem::{FILE_CREATION_DISPOSITION, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_MODE},
            },
            core::PCWSTR,
        };

        plt_hook!(
            &object,
            "CreateFileW",
            extern "system" fn(
                orig,
                lpfilename: PCWSTR,
                dwdesiredaccess: u32,
                dwsharemode: FILE_SHARE_MODE,
                lpsecurityattributes: *const SECURITY_ATTRIBUTES,
                dwcreationdisposition: FILE_CREATION_DISPOSITION,
                dwflagsandattributes: FILE_FLAGS_AND_ATTRIBUTES,
                htemplatefile: HANDLE,
            ) -> HANDLE,
            {
                let mut lpfilename = lpfilename;

                let path = PathBuf::from(OsString::from_wide(unsafe { lpfilename.as_wide() }));
                if let Some(new_path) = try_override(&path) {
                    let new_path = new_path.to_wide();
                    lpfilename = PCWSTR::from_raw(new_path.as_ptr());
                }

                unsafe {
                    orig(
                        lpfilename,
                        dwdesiredaccess,
                        dwsharemode,
                        lpsecurityattributes,
                        dwcreationdisposition,
                        dwflagsandattributes,
                        htemplatefile,
                    )
                }
            }
        )?;

        Ok(())
    }

    #[cfg(unix)]
    {
        use std::ffi::{CStr, c_char};

        use doorstop_shared::CStrExt;
        use libc::FILE;

        for symbol_name in ["fopen", "fopen64"] {
            plt_hook!(
                &object,
                symbol_name,
                extern "system" fn(orig, filename: *const c_char, mode: *const c_char) -> *mut FILE,
                {
                    let path = Path::new(unsafe { CStr::from_ptr(filename) }.as_osstr());
                    if let Some(new_path) = try_override(path) {
                        let new_path = new_path.to_cstr().unwrap();
                        return unsafe { orig(new_path.as_ptr(), mode) };
                    }

                    unsafe { orig(filename, mode) }
                }
            )
            .or_else(|e| match e.kind() {
                plthook::ErrorKind::FunctionNotFound => Ok(()),
                _ => Err(e),
            })?;
        }

        Ok(())
    }
}
