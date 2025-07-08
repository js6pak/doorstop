use std::{
    borrow::Cow,
    ffi::{CStr, CString, FromBytesWithNulError, OsStr, c_char},
};

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct InteriorNulError {
    /// The position of the interior nul byte.
    position: usize,
}

// Based on https://github.com/nagisa/rust_libloading/blob/f4ec9e702de2d0778bccff8525dc44e4cacac2d1/src/util.rs#L10-L25
pub fn cstr_cow_from_bytes(slice: &[u8]) -> Result<Cow<'_, CStr>, InteriorNulError> {
    static ZERO: c_char = 0;
    Ok(match slice.last() {
        // Slice out of 0 elements
        None => unsafe { Cow::Borrowed(CStr::from_ptr(&raw const ZERO)) },
        // Slice with trailing 0
        Some(&0) => Cow::Borrowed(CStr::from_bytes_with_nul(slice).map_err(|source| match source {
            FromBytesWithNulError::InteriorNul { position } => InteriorNulError { position },
            FromBytesWithNulError::NotNulTerminated => unreachable!(),
        })?),
        // Slice with no trailing 0
        Some(_) => Cow::Owned(CString::new(slice).map_err(|source| InteriorNulError {
            position: source.nul_position(),
        })?),
    })
}

pub trait OsStrExt {
    fn to_cstr(&self) -> Result<Cow<'_, CStr>, InteriorNulError>;

    #[cfg(windows)]
    fn to_wide(&self) -> Vec<u16>;
}

impl<S: AsRef<OsStr> + ?Sized> OsStrExt for S {
    fn to_cstr(&self) -> Result<Cow<'_, CStr>, InteriorNulError> {
        let s = self.as_ref();

        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStrExt;
            cstr_cow_from_bytes(s.as_bytes())
        }

        #[cfg(windows)]
        {
            cstr_cow_from_bytes(s.as_encoded_bytes())
        }
    }

    #[cfg(windows)]
    fn to_wide(&self) -> Vec<u16> {
        use std::os::windows::ffi::OsStrExt;

        self.as_ref().encode_wide().chain(std::iter::once(0)).collect()
    }
}

pub trait CStrExt {
    #[cfg(unix)]
    fn as_osstr(&self) -> &OsStr;
}

impl<S: AsRef<CStr> + ?Sized> CStrExt for S {
    #[cfg(unix)]
    fn as_osstr(&self) -> &OsStr {
        use std::os::unix::ffi::OsStrExt;

        let s = self.as_ref();

        OsStr::from_bytes(s.to_bytes())
    }
}
