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

use winapi::shared::minwindef::WORD;
use winapi::um::winuser::*;

use xi_win_shell::menu::Menu;
use xi_win_shell::win_main::RunLoop;

#[repr(u32)]
pub enum MenuEntries {
    // File menu entries
    Exit = 0x100,
    Open,
    Save,
    SaveAs,

    // Edit menu entries
    Undo,
    Redo,
    Copy,
    Cut,
    Paste,
    UpperCase,
    LowerCase,
    Transpose,

    // Selection menu entries
    SingleSelection,
    AddCursorAbove,
    AddCursorBelow,
    SelectAll,
}

pub fn create_menus() -> Menu {
    let mut file_menu = Menu::new();
    file_menu.add_item(MenuEntries::Open as u32, "&Open…\tCtrl+O");
    file_menu.add_item(MenuEntries::Save as u32, "&Save\tCtrl+S");
    file_menu.add_item(MenuEntries::SaveAs as u32, "Save &as…\tCtrl+Shift+S");
    file_menu.add_item(MenuEntries::Exit as u32, "E&xit");
    let mut menubar = Menu::new();
    menubar.add_dropdown(file_menu, "&File");
    let mut edit_menu = Menu::new();
    edit_menu.add_item(MenuEntries::Undo as u32, "&Undo\tCtrl+Z");
    edit_menu.add_item(MenuEntries::Redo as u32, "&Redo\tCtrl+Y");
    edit_menu.add_separator();
    edit_menu.add_item(MenuEntries::Cut as u32, "Cu&t\tCtrl+X");
    edit_menu.add_item(MenuEntries::Copy as u32, "&Copy\tCtrl+C");
    edit_menu.add_item(MenuEntries::Paste as u32, "&Paste\tCtrl+V");
    edit_menu.add_item(MenuEntries::UpperCase as u32, "Upper Case");
    edit_menu.add_item(MenuEntries::LowerCase as u32, "Lower Case");
    edit_menu.add_item(MenuEntries::Transpose as u32, "Transpose");
    menubar.add_dropdown(edit_menu, "&Edit");
    let mut selection_menu = Menu::new();
    selection_menu.add_item(MenuEntries::AddCursorAbove as u32, "Add Cursor Above\tCtrl+Alt+Up");
    selection_menu.add_item(MenuEntries::AddCursorBelow as u32, "Add Cursor Below\tCtrl+Alt+Down");
    selection_menu.add_item(MenuEntries::SingleSelection as u32, "Single Selection\tEscape");
    selection_menu.add_item(MenuEntries::SelectAll as u32, "Select All\tCtrl+A");
    menubar.add_dropdown(selection_menu, "&Selection");
    menubar
}

pub fn set_accel(runloop: &mut RunLoop) {
    let accel = accel!{
        FCONTROL, 'O', MenuEntries::Open,
        FCONTROL, 'S', MenuEntries::Save,
        FCONTROL | FSHIFT, 'S', MenuEntries::SaveAs,

        FCONTROL, 'Z', MenuEntries::Undo,
        FCONTROL, 'Y', MenuEntries::Redo,
        FCONTROL | FSHIFT, 'Z', MenuEntries::Redo,
        FCONTROL, 'X', MenuEntries::Cut,
        FCONTROL, 'C', MenuEntries::Copy,
        FCONTROL, 'V', MenuEntries::Paste,
        FCONTROL, 'T', MenuEntries::Transpose,

        // Note: arrow keys and escape are actually handled in edit_view
        FCONTROL, 'A', MenuEntries::SelectAll,
    };
    runloop.set_accel(&accel);
}
