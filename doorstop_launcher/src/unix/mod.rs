use std::{
    env,
    ffi::{CStr, OsStr, c_char},
    fs::File,
    io,
    io::Read,
    path::Path,
    slice,
};

use anyhow::{Context, bail};
use doorstop_shared::CStrExt;
use libc::execvp;

use crate::{
    get_doorstop_path,
    unix::utils::{get_executable_architectures, pick_architecture},
};

mod utils;

const PRELOAD_VARIABLE_NAME: &str = {
    #[cfg(target_os = "linux")]
    {
        "LD_PRELOAD"
    }

    #[cfg(target_os = "macos")]
    {
        "DYLD_INSERT_LIBRARIES"
    }
};

#[cfg_attr(not(test), unsafe(no_mangle))]
#[cfg_attr(test, allow(unused))]
pub unsafe extern "C" fn main(argc: i32, argv: *mut *mut c_char) -> i32 {
    let args = unsafe { slice::from_raw_parts_mut(argv, argc.try_into().unwrap()) };

    try_main(args).unwrap_or_else(|err| {
        eprintln!("[doorstop_launcher] {err:#}");
        1
    })
}

fn try_main(args: &mut [*mut c_char]) -> anyhow::Result<i32> {
    if args.len() <= 1 {
        eprintln!("usage: doorstop_launcher COMMAND [ARGS]");
        return Ok(1);
    }

    let mut executable_path = unsafe { CStr::from_ptr(args[1]) };

    if let Some(new_executable_path) = transform_executable_path(executable_path) {
        executable_path = new_executable_path;
        args[1] = new_executable_path.as_ptr().cast_mut();
    }

    let executable_path = Path::new(executable_path.as_osstr());

    if is_windows_exe(executable_path).unwrap_or(false) {
        bail!("The specified command is a Windows executable, did you mean to use doorstop_launcher.exe with wine/proton instead?");
    }

    // We explicitly don't error on failed architecture detection because we want to allow launching shell scripts
    let architecture = {
        match get_executable_architectures(executable_path) {
            Ok(architectures) => pick_architecture(&architectures),
            Err(_) => None,
        }
    };

    let doorstop_path = get_doorstop_path(architecture)?;

    let mut preload = doorstop_path.into_os_string();
    if let Some(existing_preload) = env::var_os(PRELOAD_VARIABLE_NAME) {
        preload.push(":");
        preload.push(existing_preload);
    }

    unsafe {
        env::set_var(PRELOAD_VARIABLE_NAME, preload);
    }

    unsafe {
        assert_eq!(execvp(args[1], args[1..].as_ptr().cast()), -1);

        Err(io::Error::last_os_error()).context("exec failed")?
    }
}

#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
fn transform_executable_path(path: &CStr) -> Option<&CStr> {
    #[cfg(target_os = "macos")]
    {
        use std::ptr::NonNull;

        use objc2_foundation::{NSBundle, NSString};

        let path = unsafe { NSString::stringWithUTF8String(NonNull::new(path.as_ptr().cast_mut()).unwrap()) }.unwrap();

        if let Some(executable_path) = unsafe { NSBundle::bundleWithPath(&path).and_then(|bundle| bundle.executablePath()) } {
            return Some(unsafe { CStr::from_ptr(libc::strdup(executable_path.UTF8String())) });
        }
    }

    None
}

fn is_windows_exe(path: &Path) -> io::Result<bool> {
    if path.extension() != Some(OsStr::new("exe")) {
        return Ok(false);
    }

    let mut file = File::open(path)?;
    let mut header = [0; 2];
    file.read_exact(&mut header)?;
    Ok(header == [b'M', b'Z'])
}
