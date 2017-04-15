use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;

use winapi::{HRESULT, S_OK};

#[derive(Debug)]
pub enum Error {
    Null,
    Hr(HRESULT),
}

pub fn as_result(hr: HRESULT) -> Result<(), Error> {
    match hr {
        S_OK => Ok(()),
        _ => Err(Error::Hr(hr)),
    }
}

impl From<HRESULT> for Error {
    fn from(hr: HRESULT) -> Error {
        Error::Hr(hr)
    }
}

pub trait ToWide {
    fn to_wide_sized(&self) -> Vec<u16>;
    fn to_wide(&self) -> Vec<u16>;
}

impl<T> ToWide for T where T: AsRef<OsStr> {
    fn to_wide_sized(&self) -> Vec<u16> {
        self.as_ref().encode_wide().collect()
    }
    fn to_wide(&self) -> Vec<u16> {
        self.as_ref().encode_wide().chain(Some(0)).collect()
    }
}
