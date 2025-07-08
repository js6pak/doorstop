use doorstop_shared::OsStrExt;
use log::info;
use plthook::ObjectFile;

use crate::{fatal, get_config, plt_hook};

pub(super) fn patch(object: &ObjectFile) -> anyhow::Result<()> {
    if get_config().mono_override.is_none() {
        return Ok(());
    }

    #[cfg(windows)]
    {
        use std::{ffi::OsString, os::windows::ffi::OsStringExt, path::PathBuf};

        use anyhow::Context;
        use windows::{Win32::Foundation::HMODULE, core::PCWSTR};

        plt_hook!(&object, "LoadLibraryW", extern "system" fn(orig, path: PCWSTR) -> HMODULE, {
            {
                if let Some(mono_override_path) = get_config().mono_override.as_ref() {
                    let path = PathBuf::from(OsString::from_wide(unsafe { path.as_wide() }));
                    if let Some(file_name) = path.file_name()
                        && (file_name == "mono-2.0-bdwgc.dll" || file_name == "mono.dll")
                    {
                        info!("Overriding {} to {}", file_name.display(), mono_override_path.display());
                        let new_path = mono_override_path.to_wide();
                        let result = unsafe { orig(PCWSTR::from_raw(new_path.as_ptr())) };
                        if result.is_invalid() {
                            return fatal(Err(windows::core::Error::from_thread()).context("Overridden mono couldn't be loaded"));
                        }
                        return result;
                    }
                }
            }

            unsafe { orig(path) }
        })?;

        Ok(())
    }

    #[cfg(unix)]
    {
        use std::{
            env,
            ffi::{CStr, OsStr, c_char, c_int, c_void},
            path::Path,
        };

        use anyhow::anyhow;
        use doorstop_shared::CStrExt;
        use libc::dlerror;

        plt_hook!(
            &object,
            "dlopen",
            extern "system" fn(orig, path: *const c_char, flags: c_int) -> *const c_void,
            {
                {
                    if !path.is_null()
                        && let Some(mono_override_path) = get_config().mono_override.as_ref()
                    {
                        let path = Path::new(unsafe { CStr::from_ptr(path) }.as_osstr());
                        if let Some(file_name) = path.file_name().and_then(OsStr::to_str)
                            && let Some(library_name) = file_name
                                .strip_prefix(env::consts::DLL_PREFIX)
                                .and_then(|s| s.strip_suffix(env::consts::DLL_SUFFIX))
                            && (library_name == "monobdwgc-2.0" || library_name == "mono" || library_name == "mono.0")
                        {
                            info!("Overriding {} to {}", file_name, mono_override_path.display());
                            let new_path = mono_override_path.to_cstr().unwrap();

                            let result = unsafe { orig(new_path.as_ptr(), flags) };
                            if result.is_null() {
                                let error = unsafe { CStr::from_ptr(dlerror()) };
                                return fatal(Err(anyhow!("Overridden mono couldn't be loaded: {}", error.display())));
                            }
                            return result;
                        }
                    }
                }

                unsafe { orig(path, flags) }
            }
        )?;

        Ok(())
    }
}
