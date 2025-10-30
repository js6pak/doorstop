use std::{
    ffi::{CString, c_char},
    slice,
    sync::OnceLock,
};

use cfg_if::cfg_if;
use doorstop_core::fatal;
use doorstop_shared::OsStrExt;
use libloading::os::unix::{RTLD_GLOBAL, RTLD_LAZY};

use crate::init;

pub static EXECUTABLE_PATH: OnceLock<CString> = OnceLock::new();

#[cfg_attr(not(test), unsafe(no_mangle))]
#[cfg_attr(test, allow(unused))]
extern "C" fn main(argc: i32, argv: *mut *mut c_char) -> i32 {
    fn try_main(argc: i32, argv: *mut *mut c_char) -> anyhow::Result<i32> {
        let executable = init()?;

        EXECUTABLE_PATH.set(executable.to_cstr().unwrap().into_owned()).unwrap();

        unsafe {
            let args = slice::from_raw_parts_mut(argv, argc.try_into()?);
            args[0] = EXECUTABLE_PATH.get().unwrap().as_ptr().cast_mut();
        }

        unsafe {
            const PLAYER_MAIN_SYMBOL: &[u8] = {
                cfg_if! {
                    if #[cfg(target_os = "macos")] {
                        b"_Z10PlayerMainiPPKc" // PlayerMain(int, char const**)
                    } else {
                        b"_Z10PlayerMainiPPc" // PlayerMain(int, char**)
                    }
                }
            };

            let lib = libloading::os::unix::Library::open(
                Some({
                    cfg_if! {
                        if #[cfg(target_os = "macos")] {
                            executable.join("Contents/Frameworks/UnityPlayer.dylib")
                        } else {
                            "./UnityPlayer.so"
                        }
                    }
                }),
                RTLD_LAZY | RTLD_GLOBAL,
            )?;

            let player_main: unsafe extern "system" fn(argc: i32, argv: *mut *mut c_char) -> i32 = *lib.get(PLAYER_MAIN_SYMBOL)?;

            let unity_player_handle = lib.into_raw();

            #[cfg(target_os = "linux")]
            {
                use std::{cmp::min, ffi::CStr};

                use doorstop_core::plt_hook;
                use libc::{size_t, ssize_t, strcpy};
                use plthook::ObjectFile;

                let object = ObjectFile::open_by_handle(unity_player_handle)?;

                plt_hook!(
                    &object,
                    "realpath",
                    extern "system" fn(orig, path: *const c_char, resolved_path: *mut c_char) -> *mut c_char,
                    {
                        if unsafe { CStr::from_ptr(path) } == c"/proc/self/exe" {
                            unsafe {
                                strcpy(resolved_path, EXECUTABLE_PATH.get().unwrap().as_ptr());
                            }
                            return resolved_path;
                        }

                        unsafe { orig(path, resolved_path) }
                    }
                )?;

                plt_hook!(
                    &object,
                    "readlink",
                    extern "system" fn(orig, path: *const c_char, buf: *mut c_char, bufsz: size_t) -> ssize_t,
                    {
                        if unsafe { CStr::from_ptr(path) } == c"/proc/self/exe" {
                            let path = EXECUTABLE_PATH.get().unwrap();
                            let len = min(path.as_bytes().len(), bufsz);
                            unsafe {
                                std::ptr::copy_nonoverlapping(path.as_ptr(), buf, len);
                            }
                            return len.cast_signed();
                        }

                        unsafe { orig(path, buf, bufsz) }
                    }
                )?;
            }

            #[cfg(target_os = "macos")]
            {
                use std::ptr::NonNull;

                use objc2::{
                    ClassType,
                    ffi::{class_addMethod, method_getTypeEncoding},
                    rc::Retained,
                    runtime::{AnyClass, AnyObject, Bool, Imp, Sel},
                    sel,
                };
                use objc2_foundation::{NSBundle, NSString};

                extern "C-unwind" fn my_main_bundle(_cls: &AnyClass, _cmd: Sel) -> *mut AnyObject {
                    unsafe {
                        let path = NSString::stringWithUTF8String(NonNull::new(EXECUTABLE_PATH.get().unwrap().as_ptr().cast_mut()).unwrap()).unwrap();
                        let bundle = NSBundle::bundleWithPath(&path).unwrap();
                        Retained::into_raw(bundle).cast()
                    }
                }

                let ns_bundle_class = NSBundle::class().metaclass();

                let original_method = ns_bundle_class.class_method(sel!(mainBundle)).unwrap();

                assert_eq!(
                    class_addMethod(
                        std::ptr::from_ref(ns_bundle_class).cast_mut(),
                        sel!(my_mainBundle),
                        std::mem::transmute::<*const (), Imp>(my_main_bundle as _),
                        method_getTypeEncoding(original_method),
                    ),
                    Bool::YES
                );

                let hook_method = ns_bundle_class.class_method(sel!(my_mainBundle)).unwrap();

                original_method.exchange_implementation(hook_method);
            }

            doorstop_core::try_init(unity_player_handle)?;

            Ok(player_main(argc, argv))
        }
    }

    fatal(try_main(argc, argv))
}
