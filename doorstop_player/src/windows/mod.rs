use std::sync::OnceLock;

use doorstop_core::fatal;
use doorstop_shared::OsStrExt;
use plthook::ObjectFile;
use windows::{Win32::System::LibraryLoader::LoadLibraryW, core::w};

mod create_window_patch;
mod executable_name_patch;
mod unity_main;

use unity_main::unity_main;

use crate::init;

pub static EXECUTABLE_PATH: OnceLock<Vec<u16>> = OnceLock::new();

fn try_main() -> anyhow::Result<()> {
    let executable = init()?;

    EXECUTABLE_PATH.set(executable.to_wide()).unwrap();

    unsafe {
        let unity_player_handle = LoadLibraryW(w!("UnityPlayer.dll"))?;
        let object = ObjectFile::open_by_handle(unity_player_handle.0)?;

        create_window_patch::patch(&object)?;
        executable_name_patch::patch(&object)?;

        doorstop_core::try_init(unity_player_handle.0)?;

        unity_main(unity_player_handle);
    }

    Ok(())
}

pub fn main() {
    fatal(try_main());
}
