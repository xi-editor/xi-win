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

//! Various utilities for working with windows. Includes utilities for converting between Windows 
//! and Rust types, including strings. 
//! Also includes some code to dynamically load functions at runtime. This is needed for functions
//! which are only supported on certain versions of windows.

use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::slice;
use std::mem;

use winapi::{HRESULT, LPWSTR, UINT, HMONITOR, MONITOR_DPI_TYPE, PROCESS_DPI_AWARENESS, CHAR};
use kernel32::{LoadLibraryA, GetProcAddress};

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

// Types for functions we want to load, which are only supported on newer windows versions
// from shcore.dll
type GetDpiForSystem = unsafe extern "system" fn() -> UINT;
type GetDpiForMonitor = unsafe extern "system" fn(HMONITOR, MONITOR_DPI_TYPE, *mut UINT, *mut UINT);
// from user32.dll
type SetProcessDpiAwareness = unsafe extern "system" fn(PROCESS_DPI_AWARENESS) -> HRESULT;

pub struct OptionalFunctions {
    pub get_dpi_for_system: Option<GetDpiForSystem>,
    pub get_dpi_for_monitor: Option<GetDpiForMonitor>,
    pub set_process_dpi_awareness: Option<SetProcessDpiAwareness>,
}

pub fn load_optional_functions() -> OptionalFunctions {
    let mut get_dpi_for_system = None;
    let mut get_dpi_for_monitor = None;
    let mut set_process_dpi_awareness = None;

    let shcore_lib_name = b"shcore.dll\0";
    let shcore_lib = unsafe { LoadLibraryA(shcore_lib_name.as_ptr() as *const CHAR) };

    if shcore_lib.is_null() {
        println!("No shcore.dll");
    } else {
        // Load GetDpiForSystem
        // TODO (seventh-chord, 15.10.17) somebody with win10 needs to test this
        let name = b"GetDpiForSystem\0";
        let name_ptr = name.as_ptr() as *const CHAR;
        let function_ptr = unsafe { GetProcAddress(shcore_lib, name_ptr) };

        if function_ptr.is_null() {
            println!("Could not load GetDpiForSystem (Only on windows 10)");
        } else {
            let function = unsafe { mem::transmute::<_, GetDpiForSystem>(function_ptr) };
            get_dpi_for_system = Some(function);
        }

        // Load GetDpiForMonitor
        let name = b"GetDpiForMonitor\0";
        let name_ptr = name.as_ptr() as *const CHAR;
        let function_ptr = unsafe { GetProcAddress(shcore_lib, name_ptr) };

        if function_ptr.is_null() {
            println!("Could not load GetDpiForMonitor (Only on windows 8.1 or later)");
        } else {
            let function = unsafe { mem::transmute::<_, GetDpiForMonitor>(function_ptr) };
            get_dpi_for_monitor = Some(function);
        }
    }

    let user32_lib_name = b"user32.dll\0";
    let user32_lib = unsafe { LoadLibraryA(user32_lib_name.as_ptr() as *const CHAR) };

    if user32_lib.is_null() {
        println!("No user32.dll");
    } else {
        // Load SetProcessDpiAwareness
        // TODO (seventh-chord, 15.10.17) somebody with win10 needs to test this
        let name = b"SetProcessDpiAwareness\0";
        let name_ptr = name.as_ptr() as *const CHAR;
        let function_ptr = unsafe { GetProcAddress(user32_lib, name_ptr) };

        if function_ptr.is_null() {
            get_dpi_for_system = None;
            println!("Could not load SetProcessDpiAwareness (Only on windows 10)");
        } else {
            let function = unsafe { mem::transmute::<_, SetProcessDpiAwareness>(function_ptr) };
            set_process_dpi_awareness = Some(function);
        }
    }

    OptionalFunctions {
        get_dpi_for_system,
        get_dpi_for_monitor,
        set_process_dpi_awareness,
    }
}
