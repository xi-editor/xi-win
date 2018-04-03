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

use xi_win_shell::menu::Menu;

#[repr(u32)]
pub enum MenuEntries {
    Exit = 0x100,
    Open,
    Save,
    SaveAs,
}

pub fn create_menus() -> Menu {
    let mut file_menu = Menu::new();
    file_menu.add_item(MenuEntries::Open as u32, "&Open…\tCtrl+O");
    file_menu.add_item(MenuEntries::Save as u32, "&Save\tCtrl+S");
    file_menu.add_item(MenuEntries::SaveAs as u32, "&Save as…");
    file_menu.add_item(MenuEntries::Exit as u32, "E&xit");
    let mut menubar = Menu::new();
    menubar.add_dropdown(file_menu, "&File");
    let edit_menu = Menu::new();
    menubar.add_dropdown(edit_menu, "&Edit");
    menubar
}
