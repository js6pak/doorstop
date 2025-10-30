#![feature(extern_types)]
#![feature(cstr_display)]
#![feature(drop_guard)]

mod config;
mod patches;
mod runtimes;
mod utils;

use std::{
    env,
    ffi::c_void,
    fs,
    io::Write,
    path::{Path, PathBuf},
    process,
    process::exit,
    str::FromStr,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::Context;
use cfg_if::cfg_if;
use fern::colors::{Color, ColoredLevelConfig};
use log::{LevelFilter, error, info, log_enabled, trace, warn};
use plthook::ObjectFile;

use crate::{
    config::Config,
    utils::{lazy_file_writer::LazyFileWriter, process_lock::ensure_single_instance},
};

static CONFIG: OnceLock<Config> = OnceLock::new();

pub(crate) fn get_config() -> &'static Config {
    CONFIG.get().unwrap()
}

fn fix_cwd() -> anyhow::Result<()> {
    if env::var("DOORSTOP_PLAYER").is_ok() {
        return Ok(());
    }

    let application_path = {
        cfg_if! {
            if #[cfg(target_os = "macos")] {
                use objc2_foundation::NSBundle;

                let bundle = NSBundle::mainBundle();

                PathBuf::from(bundle.bundlePath().to_string())
            } else {
                env::current_exe()?
            }
        }
    };

    let application_folder = application_path.parent().unwrap();

    env::set_current_dir(application_folder)?;

    Ok(())
}

pub unsafe fn init() {
    let unity_player_handle = {
        #[cfg(windows)]
        unsafe {
            use std::ptr;

            use windows::{Win32::System::LibraryLoader::GetModuleHandleW, core::w};

            GetModuleHandleW(w!("UnityPlayer.dll")).map_or(ptr::null(), |m| m.0)
        }

        #[cfg(unix)]
        unsafe {
            use std::ffi::CString;

            use libc::{RTLD_NOLOAD, RTLD_NOW, dlopen};

            let name: CString = {
                cfg_if! {
                    if #[cfg(target_os = "macos")] {
                        CString::from(c"@executable_path/../Frameworks/UnityPlayer.dylib")
                    } else {
                        CString::from(c"UnityPlayer.so")
                    }
                }
            };

            dlopen(name.as_ptr(), RTLD_NOW | RTLD_NOLOAD)
        }
    };

    fatal(unsafe { try_init(unity_player_handle) });
}

pub fn fatal<T, E: std::fmt::Debug + std::fmt::Display>(result: Result<T, E>) -> T {
    result.unwrap_or_else(|err| {
        if log_enabled!(log::Level::Error) {
            error!("{err:?}");
        } else {
            eprintln!("[doorstop] {err:?}");
        }

        #[cfg(windows)]
        unsafe {
            use ::windows::{
                Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MessageBoxW},
                core::{HSTRING, w},
            };

            let is_batchmode = env::args().any(|arg| arg == "-batchmode" || arg == "-nographics");
            if !is_batchmode {
                _ = MessageBoxW(None, &HSTRING::from(format!("{err:#}")), w!("Doorstop initialization failed"), MB_ICONERROR);
            }
        }

        exit(0xD004)
    })
}

pub unsafe fn try_init(unity_player_handle: *const c_void) -> anyhow::Result<()> {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::System::Console::{ATTACH_PARENT_PROCESS, AttachConsole};

        if env::var("DOORSTOP_ATTACH_CONSOLE").is_ok() {
            _ = AttachConsole(ATTACH_PARENT_PROCESS);
        }
    }

    let config = CONFIG.get_or_init(Config::load);

    setup_logging().context("Failed to setup logging")?;

    if unity_player_handle.is_null() {
        // In case there is no UnityPlayer, it could still be an old Unity version where it was compiled into the executable
        // Do a simple heuristic check by looking for a valid data folder
        if find_data_folder().is_none() {
            trace!(
                "Current process ({} - {}) is not an Unity game, skipping",
                process::id(),
                env::current_exe()?.display()
            );
            return Ok(());
        }

        FILE_LOGGING.store(true, Ordering::Relaxed);
        info!("UnityPlayer not found, hooking into main executable instead");
    } else {
        FILE_LOGGING.store(true, Ordering::Relaxed);
        info!("UnityPlayer found, initializing");
    }

    if !ensure_single_instance().context("Failed to setup process lock")? {
        warn!("Doorstop was injected more than once!");
        return Ok(());
    }

    trace!("config = {config:?}");

    fix_cwd().context("Failed to fix current working directory")?;

    unsafe {
        let object = if unity_player_handle.is_null() {
            ObjectFile::open_main_program()?
        } else {
            ObjectFile::open_by_handle(unity_player_handle)?
        };

        patches::patch(&object).context("Failed to apply patches")?;
    }

    Ok(())
}

fn find_data_folder() -> Option<PathBuf> {
    if let Ok(current_exe) = env::current_exe()
        && let Some(current_exe_dir) = current_exe.parent()
    {
        let paths = {
            cfg_if! {
                if #[cfg(target_os = "macos")] {
                    let contents_dir = current_exe_dir.parent()?;
                    [
                        contents_dir.join("Resources/Data"),
                        contents_dir.join("Data"),
                    ]
                } else {
                    [
                        {
                            let mut name = current_exe.file_stem().unwrap().to_owned();
                            name.push("_Data");
                            current_exe_dir.join(name)
                        },
                        current_exe_dir.join("Data"),
                    ]
                }
            }
        };

        for path in paths {
            let path = fs::read_link(&path).unwrap_or(path);
            if is_valid_data_folder(&path) {
                return Some(path);
            }
        }
    }

    None
}

fn is_valid_data_folder(path: &Path) -> bool {
    path.is_dir() && ["data.unity3d", "globalgamemanagers", "mainData"].iter().any(|file| path.join(file).exists())
}

static FILE_LOGGING: AtomicBool = AtomicBool::new(false);

fn setup_logging() -> anyhow::Result<()> {
    let log_level = if let Ok(level) = env::var("DOORSTOP_LOG_LEVEL") {
        LevelFilter::from_str(&level)?
    } else {
        LevelFilter::Info
    };

    let colors_line = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        .info(Color::White)
        .debug(Color::BrightBlack)
        .trace(Color::BrightBlack);

    fern::Dispatch::new()
        .level(log_level)
        .chain(
            fern::Dispatch::new()
                .format(move |out, message, record| {
                    out.finish(format_args!(
                        "{color_line}[{level} {target}{color_line}] {message}\x1B[0m",
                        color_line = format_args!("\x1B[{}m", colors_line.get_color(&record.level()).to_fg_str()),
                        target = record.target(),
                        level = record.level(),
                        message = message,
                    ));
                })
                .chain(std::io::stderr()),
        )
        .chain(
            fern::Dispatch::new()
                .filter(|_| FILE_LOGGING.load(Ordering::Relaxed))
                .format(|out, message, record| out.finish(format_args!("[{} {}] {}", record.level(), record.target(), message)))
                .chain(Box::new(LazyFileWriter::new("doorstop.log")) as Box<dyn Write + Send>),
        )
        .apply()?;

    Ok(())
}
