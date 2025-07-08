use std::cmp::min;

use doorstop_core::plt_hook;
use plthook::ObjectFile;
use windows::{
    Win32::{
        Foundation::{ERROR_INSUFFICIENT_BUFFER, HMODULE, SetLastError},
        System::LibraryLoader::GetModuleHandleW,
    },
    core::{PCWSTR, PWSTR},
};

use crate::windows::EXECUTABLE_PATH;

unsafe extern "C" {
    pub fn wcslen(s: PCWSTR) -> usize;
}

unsafe fn copy_wide_string(src: PCWSTR, dst: PWSTR, size: u32) -> u32 {
    unsafe {
        let src_len = u32::try_from(wcslen(src)).unwrap();

        let copy_len = min(src_len, size - 1);

        std::ptr::copy(src.as_ptr(), dst.as_ptr(), copy_len as usize);
        *dst.as_ptr().add(copy_len as usize) = 0;

        if copy_len < src_len {
            SetLastError(ERROR_INSUFFICIENT_BUFFER);
            size
        } else {
            src_len
        }
    }
}

pub unsafe fn patch(object: &ObjectFile) -> anyhow::Result<()> {
    plt_hook!(
        object,
        "GetModuleFileNameW",
        extern "system" fn(orig, hmodule: HMODULE, lpfilename: PWSTR, nsize: u32) -> u32,
        {
            unsafe {
                if hmodule.is_invalid() || hmodule == GetModuleHandleW(None).unwrap() {
                    let file_name = EXECUTABLE_PATH.get().unwrap().as_ptr();
                    return copy_wide_string(PCWSTR(file_name), lpfilename, nsize);
                }

                orig(hmodule, lpfilename, nsize)
            }
        }
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use windows::{Win32::Foundation::GetLastError, core::w};

    use super::*;

    #[test]
    fn successful() -> anyhow::Result<()> {
        unsafe {
            let src = w!("test");

            let mut dst = [1u16; 10];

            let len = copy_wide_string(src, PWSTR(dst.as_mut_ptr()), u32::try_from(dst.len())?) as usize;
            println!("len: {len}");
            println!("dst: {dst:?}");

            let str = String::from_utf16(&dst[0..len]).unwrap();
            println!("{str:?}");

            anyhow::ensure!(len < dst.len());
            anyhow::ensure!(str == "test");
            anyhow::ensure!(dst[len] == 0);

            Ok(())
        }
    }

    #[test]
    fn too_small() -> anyhow::Result<()> {
        unsafe {
            let src = w!("test");

            let mut dst = [1u16; 2];

            let len = copy_wide_string(src, PWSTR(dst.as_mut_ptr()), u32::try_from(dst.len())?) as usize;
            anyhow::ensure!(GetLastError() == ERROR_INSUFFICIENT_BUFFER);
            println!("len: {len}");
            println!("dst: {dst:?}");

            let str = String::from_utf16(&dst[0..len]).unwrap();
            println!("{str:?}");

            anyhow::ensure!(len == dst.len());
            anyhow::ensure!(str == "t\0");

            Ok(())
        }
    }
}
