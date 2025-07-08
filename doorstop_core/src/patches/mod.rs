mod boot_config_override_patch;
mod disable_console_redirect_patch;
mod mono_override_patch;

use std::{
    env,
    ffi::{CStr, c_char, c_void},
};

use log::trace;
use plthook::ObjectFile;

use crate::{
    get_config, plt_hook,
    runtimes::{il2cpp, mono},
};

pub unsafe fn patch(object: &ObjectFile) -> anyhow::Result<()> {
    if get_config().redirect_output_log {
        unsafe { env::set_var("UNITY_LOG_FILE", "output_log.txt") }
    }

    boot_config_override_patch::patch(object)?;
    disable_console_redirect_patch::patch(object)?;
    mono_override_patch::patch(object)?;

    unsafe {
        env::set_var("DOORSTOP_INITIALIZED", "TRUE");
        env::set_var("DOORSTOP_PROCESS_PATH", env::current_exe()?);
        if let Some(target_assembly) = get_config().target_assembly.as_ref() {
            env::set_var("DOORSTOP_INVOKE_DLL_PATH", target_assembly);
        }
    }

    // Older MacOS builds linked mono directly
    #[cfg(target_os = "macos")]
    unsafe {
        use std::ffi::CString;

        use anyhow::bail;
        use libc::{RTLD_NOLOAD, RTLD_NOW, dlopen};
        use log::{info, warn};

        let mut libmono_handle = dlopen(
            c"@executable_path/../Frameworks/MonoEmbedRuntime/osx/libmono.0.dylib".as_ptr(),
            RTLD_NOW | RTLD_NOLOAD,
        );

        if !libmono_handle.is_null() {
            warn!("libmono is linked directly");

            if let Some(mono_override_path) = get_config().mono_override.as_ref() {
                info!("Overriding mono to {}", mono_override_path.display());
                libc::dlclose(libmono_handle);
                libmono_handle = libloading::os::unix::Library::new(mono_override_path)?.into_raw();
            }

            for symbol in object.symbols() {
                let Some(name) = symbol.name.to_str().ok().and_then(|s| s.strip_prefix("_")) else {
                    continue;
                };

                if !name.starts_with("mono_") && !name.starts_with("unity_mono_") && !name.starts_with("GC_") && name != "g_free" {
                    continue;
                }

                let address = libc::dlsym(libmono_handle, CString::new(name)?.as_ptr());
                if address.is_null() {
                    bail!("Couldn't find {name} in mono");
                }

                if let Some(address) = mono::try_hook(libmono_handle, name, address) {
                    trace!("Hooking {name}");
                    object.replace(name, address)?.discard();
                } else if get_config().mono_override.is_some() {
                    object.replace(name, address)?.discard();
                }
            }

            return Ok(());
        }
    }

    let get_symbol_name = {
        #[cfg(windows)]
        {
            "GetProcAddress"
        }

        #[cfg(unix)]
        {
            "dlsym"
        }
    };

    plt_hook!(
        &object,
        get_symbol_name,
        extern "system" fn(orig, module: *mut c_void, name: *const c_char) -> *const c_void,
        {
            let address = unsafe { orig(module, name) };

            #[cfg(windows)]
            if (name as usize) >> 16 == 0 {
                // High-order word is 0, the name parameter is the function's ordinal value
                return address;
            }

            let name = unsafe { CStr::from_ptr(name) };

            if let Ok(name) = name.to_str()
                && let Some(address) = Option::or(mono::try_hook(module, name, address), il2cpp::try_hook(module, name, address))
            {
                trace!("Hooking {name}");
                return address;
            }

            address
        }
    )?;

    Ok(())
}
