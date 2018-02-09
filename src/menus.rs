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

//! Configuration and runtime for the main window's menus.

use user32::*;
use winapi::*;

use util::ToWide;

#[repr(u32)]
pub enum MenuEntries {
    Exit = 0x100,
    Open,
    Save,
}

pub struct Menus {
    hmenubar: HMENU,
}

impl Menus {
    // TODO: wire up accelerators corresponding to the advertised keyboard shortcuts.
    pub fn create() -> Menus {
        unsafe {
            let hmenubar = CreateMenu();
            let hmenu = CreateMenu();
            AppendMenuW(hmenubar, MF_POPUP, hmenu as UINT_PTR, "&File".to_wide().as_ptr());
            AppendMenuW(hmenu, MF_STRING, MenuEntries::Open as UINT_PTR, "&Open\tCtrl+O".to_wide().as_ptr());
            AppendMenuW(hmenu, MF_STRING, MenuEntries::Save as UINT_PTR, "&Save\tCtrl+S".to_wide().as_ptr());
            AppendMenuW(hmenu, MF_STRING, MenuEntries::Exit as UINT_PTR, "E&xit".to_wide().as_ptr());

            let hmenu = CreateMenu();
            AppendMenuW(hmenubar, MF_POPUP, hmenu as UINT_PTR, "&Edit".to_wide().as_ptr());

            Menus {
                hmenubar: hmenubar,
            }
        }
    }

    pub fn get_hmenubar(&self) -> HMENU {
        self.hmenubar
    }
}
