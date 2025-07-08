#![feature(macro_metavar_expr_concat)]
#![feature(maybe_uninit_slice)]

use ctor::ctor;

#[cfg(windows)]
mod windows;

#[ctor]
fn doorstop_ctor() {
    unsafe {
        doorstop_core::init();
    }
}
