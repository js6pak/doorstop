#![cfg_attr(all(not(test), unix), no_main)]

use std::{env, env::current_dir, fs::read_dir, path::PathBuf};

use anyhow::bail;
use cfg_if::cfg_if;

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::main;

#[cfg(unix)]
mod unix;

fn get_executable_path() -> anyhow::Result<PathBuf> {
    if let Ok(var) = env::var("DOORSTOP_PLAYER_EXECUTABLE") {
        let path = PathBuf::from(var);

        cfg_if! {
            if #[cfg(target_os = "macos")] {
                if !path.is_dir() {
                    bail!("DOORSTOP_PLAYER_EXECUTABLE has to point to an app bundle");
                }
            } else {
                if path.is_dir() || path.parent().is_none_or(|p| !p.is_dir()) {
                    bail!("DOORSTOP_PLAYER_EXECUTABLE has to be a valid file");
                }
            }
        }

        return Ok(path);
    }

    let current_dir = current_dir()?;
    let mut executables = read_dir(&current_dir)?.filter_map(Result::ok).map(|e| e.path()).filter(|path| {
        cfg_if! {
            if #[cfg(target_os = "macos")] {
                return path.is_dir() && path.extension() == Some(std::ffi::OsStr::new("app")) && path.join("Contents/Resources/Data").is_dir();
            } else {
                use std::ffi::OsString;

                #[cfg(windows)]
                if path.extension() != Some(std::ffi::OsStr::new("exe")) {
                    return false;
                }

                if let Some(stem) = path.file_stem() {
                    let mut data_name = OsString::from(stem);
                    data_name.push("_Data");
                    let data_path = current_dir.join(data_name);
                    return data_path.is_dir();
                }

                false
            }
        }
    });

    if let Some(executable) = executables.next() {
        if executables.next().is_some() {
            bail!("Found more than 1 unity executable in current directory, specify one with DOORSTOP_PLAYER_EXECUTABLE environment variable");
        }

        return Ok(executable);
    }

    bail!("Couldn't find the unity executable in current directory, specify one with DOORSTOP_PLAYER_EXECUTABLE environment variable");
}

pub fn init() -> anyhow::Result<PathBuf> {
    unsafe {
        env::set_var("DOORSTOP_PLAYER", "true");
    }

    let executable = get_executable_path()?;
    env::set_current_dir(executable.parent().unwrap())?;

    Ok(executable)
}
