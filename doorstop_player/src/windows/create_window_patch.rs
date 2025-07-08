use std::ffi::c_void;

use doorstop_core::plt_hook;
use plthook::ObjectFile;
use windows::{
    Win32::{
        Foundation::{HINSTANCE, HWND, LPARAM, WPARAM},
        Globalization::lstrcmpW,
        UI::{
            Shell::ExtractIconExW,
            WindowsAndMessaging::{HICON, HMENU, ICON_BIG, ICON_SMALL, SendMessageW, WINDOW_EX_STYLE, WINDOW_STYLE, WM_SETICON},
        },
    },
    core::{PCWSTR, w},
};

use crate::windows::EXECUTABLE_PATH;

pub unsafe fn patch(object: &ObjectFile) -> anyhow::Result<()> {
    plt_hook!(
        object,
        "CreateWindowExW",
        extern "system" fn(
            orig,
            dwexstyle: WINDOW_EX_STYLE,
            lpclassname: PCWSTR,
            lpwindowname: PCWSTR,
            dwstyle: WINDOW_STYLE,
            x: i32,
            y: i32,
            nwidth: i32,
            nheight: i32,
            hwndparent: HWND,
            hmenu: HMENU,
            hinstance: HINSTANCE,
            lpparam: *const c_void,
        ) -> HWND,
        {
            unsafe {
                #[rustfmt::skip]
                let result = orig(dwexstyle, lpclassname, lpwindowname, dwstyle, x, y, nwidth, nheight, hwndparent, hmenu, hinstance, lpparam);

                if (lpclassname.as_ptr() as usize) >> 16 != 0 && lstrcmpW(lpclassname, w!("UnityWndClass")) == 0 {
                    let executable_path = EXECUTABLE_PATH.get().unwrap().as_ptr();

                    let mut large_icons: [HICON; 1] = [HICON::default()];
                    let mut small_icons: [HICON; 1] = [HICON::default()];

                    let icons_extracted = ExtractIconExW(PCWSTR(executable_path), 0, Some(large_icons.as_mut_ptr()), Some(small_icons.as_mut_ptr()), 1);

                    if icons_extracted > 0 && icons_extracted != u32::MAX {
                        let [large_icon] = large_icons;
                        let [small_icon] = small_icons;

                        if !large_icon.0.is_null() {
                            SendMessageW(result, WM_SETICON, Some(WPARAM(ICON_BIG as _)), Some(LPARAM(large_icon.0 as _)));
                        }

                        if !small_icon.0.is_null() {
                            SendMessageW(result, WM_SETICON, Some(WPARAM(ICON_SMALL as _)), Some(LPARAM(small_icon.0 as _)));
                        }
                    }
                }

                result
            }
        }
    )?;

    Ok(())
}
