#![cfg_attr(windows, feature(windows_process_exit_code_from))]
#![cfg_attr(all(not(test), unix), no_main)]

use std::{
    env,
    env::consts::{DLL_PREFIX, DLL_SUFFIX},
    fmt::{Display, Formatter},
    path::PathBuf,
};

use anyhow::{anyhow, bail};

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use windows::main;

#[cfg(unix)]
mod unix;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[allow(unused)]
enum ProcessorArchitecture {
    X64,
    X86,
    Arm64,
    Arm,
}

impl Display for ProcessorArchitecture {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ProcessorArchitecture::X64 => "X64",
                ProcessorArchitecture::X86 => "X86",
                ProcessorArchitecture::Arm64 => "ARM64",
                ProcessorArchitecture::Arm => "ARM",
            }
        )
    }
}

fn get_doorstop_path(architecture: Option<ProcessorArchitecture>) -> anyhow::Result<PathBuf> {
    let mut vars = vec!["DOORSTOP_PATH".to_string()];
    if let Some(architecture) = architecture {
        vars.insert(0, format!("DOORSTOP_{}_PATH", architecture.to_string().to_uppercase()));
    }

    for var_name in vars {
        if let Ok(var) = env::var(&var_name) {
            let path = PathBuf::from(&var);

            if !path.is_file() {
                bail!("{var_name} has to be a valid file");
            }

            return Ok(path);
        }
    }

    let mut paths_to_check = vec![env::current_dir()?];
    if let Ok(current_exe) = env::current_exe()
        && let Some(current_exe_dir) = current_exe.parent()
    {
        paths_to_check.push(current_exe_dir.to_path_buf());
    }

    let mut names = vec!["doorstop".to_string()];
    if let Some(architecture) = architecture {
        names.insert(0, format!("doorstop-{}", architecture.to_string().to_lowercase()));
    }

    for path in paths_to_check {
        for name in &names {
            let path = path.join(format!("{DLL_PREFIX}{name}{DLL_SUFFIX}"));

            if path.is_file() {
                return Ok(path);
            }
        }
    }

    Err(anyhow!(
        "Couldn't find the doorstop library, specify one with DOORSTOP_PATH environment variable"
    ))
}
