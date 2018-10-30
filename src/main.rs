// Copyright 2017 The xi-editor Authors.
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

//! The main module for the xi editor front end.


// NOTE: This disables stdout, so no println :(
// TODO: If we checked what GetStdHandle returns for stdout and see
// that it is an invalid handle (either -1 or 0), then we can open up
// up a file to log stdout and SetStdHandle.
#![windows_subsystem = "windows"]

extern crate winapi;
extern crate direct2d;
extern crate directwrite;

extern crate serde;
#[macro_use]
extern crate serde_json;

extern crate xi_core_lib;
extern crate xi_rpc;
#[macro_use]
extern crate xi_win_shell;
extern crate xi_win_ui;

mod edit_view;
mod linecache;
mod menus;
mod rpc;
mod textline;
mod xi_thread;

use std::sync::{Arc, Mutex};
use std::collections::HashMap;

use serde_json::Value;

use edit_view::EditView;
use menus::MenuEntries;
use rpc::{Core, Handler};
use xi_thread::start_xi_thread;

use xi_win_shell::win_main::{self};
use xi_win_shell::window::{Cursor, IdleHandle, WindowBuilder};

use xi_win_ui::{UiMain, UiState};
use xi_win_ui::Id;
use xi_win_ui::{FileDialogOptions, FileDialogType};

use edit_view::EditViewCommands;

type ViewId = String;

#[derive(Clone)]
struct ViewState {
    id: Id,
    filename: Option<String>,
    handle: Arc<Mutex<IdleHandle>>,
}

#[derive(Clone)]
struct AppState {
    focused: ViewId,
    views: HashMap<ViewId, ViewState>,
}

impl AppState {
    fn new() -> AppState {
        AppState {
            focused: Default::default(),
            views: HashMap::new(),
        }
    }

    fn get_focused_viewstate(&mut self) -> &mut ViewState {
        if let Some(state) = self.views.get_mut(&self.focused) {
            return state
        } else {
            panic!("Getting viewstate failed.\nFocused: {}\n", &self.focused)
        }
    }
}

#[derive(Clone)]
struct App {
    core: Arc<Mutex<Core>>,
    state: Arc<Mutex<AppState>>,
}

impl App {
    fn new(core: Core) -> App {
        App {
            core: Arc::new(Mutex::new(core)),
            state: Arc::new(Mutex::new(AppState::new())),
        }
    }

    fn send_notification(&self, method: &str, params: &Value) {
        self.get_core().send_notification(method, params);
    }

    fn send_view_cmd(&self, cmd: EditViewCommands) {
        let mut state = self.get_state();
        let focused = state.get_focused_viewstate();

        UiMain::send_ext(&focused.handle.lock().unwrap(), focused.id, cmd);
    }
}

impl App {
    fn get_core(&self) -> std::sync::MutexGuard<'_, rpc::Core, > {
        self.core.lock().unwrap()
    }

    fn get_state(&self) -> std::sync::MutexGuard<'_, AppState, > {
        self.state.lock().unwrap()
    }
}

impl App {
    fn req_new_view(&self, filename: Option<&str>, handle: Arc<Mutex<IdleHandle>>) {
        let mut params = json!({});

        let filename = if filename.is_some() {
            params["file_path"] = json!(filename.unwrap());
            Some(filename.unwrap().to_string())
        } else {
            None
        };

        let edit_view = 0;
        let core = Arc::downgrade(&self.core);
        let state = self.state.clone();
        self.core.lock().unwrap().send_request("new_view", &params,
            move |value| {
                let view_id = value.clone().as_str().unwrap().to_string();
                let mut state = state.lock().unwrap();
                let handle = handle.clone();
                state.focused = view_id.clone();
                state.views.insert(view_id.clone(),
                    ViewState {
                        id: 0,
                        filename: filename.clone(),
                        handle: handle.clone(),
                    }
                );
                UiMain::send_ext(&handle.lock().unwrap(), edit_view, EditViewCommands::Core(core));
                UiMain::send_ext(&handle.lock().unwrap(), edit_view, EditViewCommands::ViewId(view_id));
            }
        );
    }

    fn handle_cmd(&self, method: &str, params: &Value) {
        match method {
            "update" => self.send_view_cmd(EditViewCommands::ApplyUpdate(params["update"].clone())),
            "scroll_to" => self.send_view_cmd(EditViewCommands::ScrollTo(params["line"].as_u64().unwrap() as usize)),
            "available_themes" => (), // TODO
            "available_plugins" => (), // TODO
            "available_languages" => (), // TODO
            "config_changed" => (), // TODO
            "language_changed" => (), // TODO
            _ => println!("unhandled core->fe method {}", method),
        }
    }
}

#[derive(Clone)]
struct AppDispatcher {
    app: Arc<Mutex<Option<App>>>,
}

impl AppDispatcher {
    fn new() -> AppDispatcher {
        AppDispatcher {
            app: Default::default(),
        }
    }

    fn set_app(&mut self, app: &App) {
        *self.app.lock().unwrap() = Some(app.clone());
    }

    fn set_menu_listeners(&self, state: &mut UiState) {
        let app = self.app.clone();
        state.set_command_listener(move |cmd, mut ctx| {            
            match cmd {
                cmd if cmd == MenuEntries::Exit as u32 => {
                    ctx.close();
                }
                cmd if cmd == MenuEntries::Open as u32 => {
                    if let Some(app) = app.lock().unwrap().as_ref() {
                        let filename = ctx.file_dialog(FileDialogType::Open, FileDialogOptions::default());
                        if filename.is_err() {
                            return;
                        }
                        let filename = filename.unwrap().into_string();
                        if filename.is_err() { // invalid unicode data
                            return;
                        }
                        let filename = filename.unwrap();
                        let mut state = app.get_state();
                        let mut view_state = state.get_focused_viewstate();
                        app.req_new_view(Some(&filename), view_state.handle.clone());
                        view_state.filename = Some(filename);
                    }
                }
                cmd if cmd == MenuEntries::Save as u32 => {
                    if let Some(app) = app.lock().unwrap().as_ref() {
                        {
                            let mut state = app.get_state();
                            let mut view_state = state.get_focused_viewstate();
                            if view_state.filename.is_none() {
                                let filename = ctx.file_dialog(FileDialogType::Save, FileDialogOptions::default());
                                if filename.is_err() {
                                    return;
                                }
                                let filename = filename.unwrap().into_string();
                                if filename.is_err() { // invalid unicode data
                                    return;
                                }
                                view_state.filename = Some(filename.unwrap());
                            }
                        }
                        let state = app.get_state();
                        let view_state = &state.views[&state.focused];
                        app.send_notification("save", &json!({
                            "view_id": &state.focused,
                            "file_path": view_state.filename,
                        }));
                    }
                }
                cmd if cmd == MenuEntries::SaveAs as u32 => {
                    if let Some(app) = app.lock().unwrap().as_ref() {
                        let filename = ctx.file_dialog(FileDialogType::Save, FileDialogOptions::default());
                        let filename = filename.unwrap().into_string().unwrap();
                        app.send_notification("save", &json!({
                            "view_id": app.get_state().focused,
                            "file_path": filename,
                        }));

                        app.get_state().get_focused_viewstate().filename = Some(filename);
                    }
                }
                cmd if cmd == MenuEntries::Undo as u32 => {
                    if let Some(app) = app.lock().unwrap().as_ref() {
                        app.send_view_cmd(EditViewCommands::Undo);
                    }
                }
                cmd if cmd == MenuEntries::Redo as u32 => {
                    if let Some(app) = app.lock().unwrap().as_ref() {
                        app.send_view_cmd(EditViewCommands::Redo);
                    }
                }
                // TODO: cut, copy, paste (requires pasteboard)
                cmd if cmd == MenuEntries::UpperCase as u32 => {
                    if let Some(app) = app.lock().unwrap().as_ref() {
                        app.send_view_cmd(EditViewCommands::UpperCase);
                    }
                }
                cmd if cmd == MenuEntries::LowerCase as u32 => {
                    if let Some(app) = app.lock().unwrap().as_ref() {
                        app.send_view_cmd(EditViewCommands::LowerCase);
                    }
                }
                cmd if cmd == MenuEntries::Transpose as u32 => {
                    if let Some(app) = app.lock().unwrap().as_ref() {
                        app.send_view_cmd(EditViewCommands::Transpose);
                    }
                }
                cmd if cmd == MenuEntries::AddCursorAbove as u32 => {
                    if let Some(app) = app.lock().unwrap().as_ref() {
                        app.send_view_cmd(EditViewCommands::AddCursorAbove);
                    }
                }
                cmd if cmd == MenuEntries::AddCursorBelow as u32 => {
                    if let Some(app) = app.lock().unwrap().as_ref() {
                        app.send_view_cmd(EditViewCommands::AddCursorBelow);
                    }
                }
                cmd if cmd == MenuEntries::SingleSelection as u32 => {
                    if let Some(app) = app.lock().unwrap().as_ref() {
                        app.send_view_cmd(EditViewCommands::SingleSelection);
                    }
                }
                cmd if cmd == MenuEntries::SelectAll as u32 => {
                    if let Some(app) = app.lock().unwrap().as_ref() {
                        app.send_view_cmd(EditViewCommands::SelectAll);
                    }
                }
                _ => println!("unexpected cmd {}", cmd),
            }
        });
    }
}

impl Handler for AppDispatcher {
    fn notification(&self, method: &str, params: &Value) {
        println!("core->fe: {} {}", method, params);
        if let Some(ref app) = *self.app.lock().unwrap() {
            app.handle_cmd(method, params);
        }
    }
}

fn build_app(state: &mut UiState) {
    // todo: widgets which support tabs and split panes
    let edit_view = EditView::new().ui(state);
    state.set_root(edit_view);
    state.set_focus(Some(edit_view));
}

fn main() {
    xi_win_shell::init();

    let (xi_peer, rx) = start_xi_thread();

    let mut handler = AppDispatcher::new();
    let mut runloop = win_main::RunLoop::new();
    let mut builder = WindowBuilder::new();
    let mut state = UiState::new();

    handler.set_menu_listeners(&mut state);
    build_app(&mut state);
    menus::set_accel(&mut runloop);

    builder.set_handler(Box::new(UiMain::new(state)));
    builder.set_title("xi-editor");
    builder.set_cursor(Cursor::IBeam);
    builder.set_menu(menus::create_menus());
    let window = builder.build().unwrap();

    let core = Core::new(xi_peer, rx, handler.clone());
    let app = App::new(core);
    handler.set_app(&app);

    app.send_notification("client_started", &json!({}));

    let handle = Arc::new(Mutex::new(window.get_idle_handle().unwrap()));
    app.req_new_view(None, handle);

    window.show();
    runloop.run();
}
