// Copied from https://github.com/rust-lang/rust/blob/6b00bc3880198600130e1cf62b8f8a93494488cc/library/std/src/sys/pal/windows/mod.rs#L190-L277
// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: The Rust Project Developers (see https://thanks.rust-lang.org)

use std::{ffi::OsString, io, mem::MaybeUninit, os::windows::ffi::OsStringExt, path::PathBuf};

use windows::Win32::Foundation::{ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, GetLastError, SetLastError};

// Many Windows APIs follow a pattern of where we hand a buffer and then they
// will report back to us how large the buffer should be or how many bytes
// currently reside in the buffer. This function is an abstraction over these
// functions by making them easier to call.
//
// The first callback, `f1`, is passed a (pointer, len) pair which can be
// passed to a syscall. The `ptr` is valid for `len` items (u16 in this case).
// The closure is expected to:
// - On success, return the actual length of the written data *without* the null terminator.
//   This can be 0. In this case the last_error must be left unchanged.
// - On insufficient buffer space,
//   - either return the required length *with* the null terminator,
//   - or set the last-error to ERROR_INSUFFICIENT_BUFFER and return `len`.
// - On other failure, return 0 and set last_error.
//
// This is how most but not all syscalls indicate the required buffer space.
// Other syscalls may need translation to match this protocol.
//
// Once the syscall has completed (errors bail out early) the second closure is
// passed the data which has been read from the syscall. The return value
// from this closure is then the return value of the function.
pub fn fill_utf16_buf<F1, F2, T>(mut f1: F1, f2: F2) -> io::Result<T>
where
    F1: FnMut(*mut u16, usize) -> u32,
    F2: FnOnce(&[u16]) -> T,
{
    // Start off with a stack buf but then spill over to the heap if we end up
    // needing more space.
    //
    // This initial size also works around `GetFullPathNameW` returning
    // incorrect size hints for some short paths:
    // https://github.com/dylni/normpath/issues/5
    let mut stack_buf: [MaybeUninit<u16>; 512] = [MaybeUninit::uninit(); 512];
    let mut heap_buf: Vec<MaybeUninit<u16>> = Vec::new();
    unsafe {
        let mut n = stack_buf.len();
        loop {
            let buf = if n <= stack_buf.len() {
                &mut stack_buf[..]
            } else {
                let extra = n - heap_buf.len();
                heap_buf.reserve(extra);
                // We used `reserve` and not `reserve_exact`, so in theory we
                // may have gotten more than requested. If so, we'd like to use
                // it... so long as we won't cause overflow.
                n = heap_buf.capacity().min(u32::MAX as usize);
                // Safety: MaybeUninit<u16> does not need initialization
                heap_buf.set_len(n);
                &mut heap_buf[..]
            };

            // This function is typically called on windows API functions which
            // will return the correct length of the string, but these functions
            // also return the `0` on error. In some cases, however, the
            // returned "correct length" may actually be 0!
            //
            // To handle this case we call `SetLastError` to reset it to 0 and
            // then check it again if we get the "0 error value". If the "last
            // error" is still 0 then we interpret it as a 0 length buffer and
            // not an actual error.
            SetLastError(ERROR_SUCCESS);
            let k = match f1(buf.as_mut_ptr().cast::<u16>(), n) {
                0 if GetLastError().0 == 0 => 0,
                0 => return Err(io::Error::last_os_error()),
                n => n,
            } as usize;
            if k == n && GetLastError() == ERROR_INSUFFICIENT_BUFFER {
                n = n.saturating_mul(2).min(u32::MAX as usize);
            } else if k > n {
                n = k;
            } else if k == n {
                // It is impossible to reach this point.
                // On success, k is the returned string length excluding the null.
                // On failure, k is the required buffer length including the null.
                // Therefore k never equals n.
                unreachable!();
            } else {
                // Safety: First `k` values are initialized.
                let slice: &[u16] = buf[..k].assume_init_ref();
                return Ok(f2(slice));
            }
        }
    }
}

pub fn os2path(s: &[u16]) -> PathBuf {
    PathBuf::from(OsString::from_wide(s))
}
