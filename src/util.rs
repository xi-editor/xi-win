// Copyright 2017 Google Inc. All rights reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Utilities for converting between Windows and Rust types, including strings.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::slice;

use winapi::{HRESULT, LPWSTR};

#[derive(Debug)]
pub enum Error {
    Null,
    Hr(HRESULT),
}

/*
pub fn as_result(hr: HRESULT) -> Result<(), Error> {
    match hr {
        S_OK => Ok(()),
        _ => Err(Error::Hr(hr)),
    }
}
*/

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

pub trait FromWide {
    fn from_wide(&self) -> Option<String>;
}

impl FromWide for LPWSTR {
    fn from_wide(&self) -> Option<String> {
        unsafe {
            let mut len = 0;
            while *self.offset(len) != 0 {
                len += 1;
            }
            slice::from_raw_parts(*self, len as usize).from_wide()
        }
    }
}

impl FromWide for [u16] {
    fn from_wide(&self) -> Option<String> {
        String::from_utf16(self).ok()
    }
}
