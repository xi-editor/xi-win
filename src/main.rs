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

//! The main module for the xi editor front end.

#![windows_subsystem = "windows"] // NOTE: This disables stdout, so no println :(

#[macro_use]
extern crate winapi;
extern crate direct2d;
extern crate directwrite;

extern crate serde;
#[macro_use]
extern crate serde_json;

extern crate xi_core_lib;
extern crate xi_rpc;

mod hwnd_rt;
mod linecache;
mod menus;
mod util;
mod window;
mod dialog;
mod xi_thread;

use std::cell::RefCell;
use std::mem;
use std::ptr::null_mut;
use std::sync::mpsc::TryRecvError;
use std::rc::Rc;

use winapi::shared::minwindef::*;
use winapi::shared::ntdef::LPCWSTR;
use winapi::shared::windef::*;
use winapi::um::d2d1::*;
use winapi::um::shellscalingapi::*;
use winapi::um::winbase::*;
use winapi::um::wingdi::*;
use winapi::um::winuser::*;

use direct2d::{RenderTarget, brush};
use direct2d::math::*;
use direct2d::render_target::DrawTextOption;
use directwrite::text_format::{self, TextFormat};
use directwrite::text_layout::{self, TextLayout};

use serde_json::Value;

use hwnd_rt::HwndRtParams;
use linecache::LineCache;
use menus::Menus;
use util::{Error, ToWide, OptionalFunctions};
use window::{create_window, WndProc};
use dialog::{get_open_file_dialog_path, get_save_file_dialog_path};
use xi_thread::{start_xi_thread, XiPeer};

struct Resources {
    fg: brush::SolidColor,
    bg: brush::SolidColor,
    text_format: TextFormat,
}

impl Resources {
    fn create_text_layout(&self, factory: &directwrite::Factory, text: &str) -> TextLayout {
        let params = text_layout::ParamBuilder::new()
            .text(text)
            .font(self.text_format.clone())
            .width(1e6)
            .height(1e6)
            .build().unwrap();
        factory.create(params).unwrap()
    }
}

struct MainWinState {
    view_id: String,
    line_cache: LineCache,
    label: String,
    self_hwnd: HWND,
    d2d_factory: direct2d::Factory,
    dwrite_factory: directwrite::Factory,
    render_target: Option<RenderTarget>,
    resources: Option<Resources>,
    filename: Option<String>,
}

impl MainWinState {
    fn new() -> MainWinState {
        MainWinState {
            view_id: String::new(),
            line_cache: LineCache::new(),
            label: "hello direct2d".to_string(),
            self_hwnd: null_mut(),
            d2d_factory: direct2d::Factory::new().unwrap(),
            dwrite_factory: directwrite::Factory::new().unwrap(),
            render_target: None,
            resources: None,
            filename: None
        }
    }

    fn create_resources(&mut self) -> Resources {
        let rt = self.render_target.as_mut().unwrap();
        let text_format_params = text_format::ParamBuilder::new()
            .size(15.0)
            .family("Consolas")
            .build().unwrap();
        let text_format = self.dwrite_factory.create(text_format_params).unwrap();
        Resources {
            fg: rt.create_solid_color_brush(0xf0f0ea, &BrushProperties::default()).unwrap(),
            bg: rt.create_solid_color_brush(0x272822, &BrushProperties::default()).unwrap(),
            text_format: text_format,
        }
    }

    fn render(&mut self) {
        let res = {
            if self.resources.is_none() {
                self.resources = Some(self.create_resources());
            }
            let resources = &self.resources.as_ref().unwrap();
            let rt = self.render_target.as_mut().unwrap();
            rt.begin_draw();
            let size = rt.get_size();
            let rect = RectF::from((0.0, 0.0, size.width, size.height));
            rt.fill_rectangle(&rect, &resources.bg);

            let x0 = 6.0;
            let mut y = 6.0;
            for line_num in 0..self.line_cache.height() {
                if let Some(line) = self.line_cache.get_line(line_num) {
                    let layout = resources.create_text_layout(&self.dwrite_factory, line.text());
                    rt.draw_text_layout(
                        &Point2F::from((x0, y)),
                        &layout,
                        &resources.fg,
                        &[DrawTextOption::EnableColorFont]
                    );
                    for &offset in line.cursor() {
                        if let Some(pos) = layout.hit_test_text_position(offset as u32, true) {
                            let x = x0 + pos.point_x;
                            rt.draw_line(&Point2F::from((x, y)),
                                &Point2F::from((x, y + 17.0)),
                                &resources.fg, 1.0, None);
                        }
                    }
                }
                y += 17.0;
            }
            rt.end_draw()
        };
        if res.is_err() {
            self.render_target = None;
        }
    }
}

struct MainWin {
    peer: XiPeer,
    state: RefCell<MainWinState>,
}

impl MainWin {
    fn new(peer: XiPeer, state: MainWinState) -> MainWin {
        MainWin {
            peer: peer,
            state: RefCell::new(state)
        }
    }

    fn send_notification(&self, method: &str, params: &Value) {
        let cmd = json!({
            "method": method,
            "params": params,
        });
        self.peer.send_json(&cmd);
    }

    // Note: caller can't be borrowing the state.
    fn send_edit_cmd(&self, method: &str, params: &Value) {
        let view_id = &self.state.borrow_mut().view_id;
        let edit_params = json!({
            "method": method,
            "params": params,
            "tab": view_id,
        });
        self.send_notification("edit", &edit_params);
    }

    fn file_open(&self, hwnd_owner: HWND) {
        let filename = unsafe { get_open_file_dialog_path(hwnd_owner) };
        if let Some(filename) = filename {
            self.state.borrow_mut().filename = Some(filename.clone());
            // Note: this whole protocol has changed a lot since the
            // 0.2 version of xi-core.
            self.send_edit_cmd("open", &json!({
                "filename": filename,
            }));
        }
    }

    fn file_save(&self, hwnd_owner: HWND) {
        let filename: Option<String> = self.state.borrow_mut().filename.clone();
        if filename.is_none() {
            self.file_save_as(hwnd_owner);
        } else {
            let filename = filename.unwrap();
            self.send_edit_cmd("save", &json!({
                "filename": filename,
            }));
        }
    }

    fn file_save_as(&self, hwnd_owner: HWND) {
        if let Some(filename) = unsafe { get_save_file_dialog_path(hwnd_owner) } {
            self.send_edit_cmd("save", &json!({
                "filename": filename,
            }));

            self.state.borrow_mut().filename = Some(filename.clone());
        }
    }
}

impl WndProc for MainWin {
    fn window_proc(&self, hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        //println!("{:x} {:x} {:x}", msg, wparam, lparam);
        match msg {
            WM_CREATE => {
                self.state.borrow_mut().self_hwnd = hwnd;
                let cmd = json!({
                    "method": "new_tab",
                    "params": [],
                    "id": 0
                });
                self.peer.send_json(&cmd);
                None
            }
            WM_DESTROY => unsafe {
                self.state.borrow_mut().self_hwnd = null_mut();
                PostQuitMessage(0);
                None
            },
            WM_PAINT => unsafe {
                let mut state = self.state.borrow_mut();
                if state.render_target.is_none() {
                    let mut rect: RECT = mem::uninitialized();
                    GetClientRect(hwnd, &mut rect);
                    //println!("rect={:?}", rect);
                    let width = (rect.right - rect.left) as u32;
                    let height = (rect.bottom - rect.top) as u32;
                    let params = HwndRtParams { hwnd: hwnd, width: width, height: height };
                    state.render_target = state.d2d_factory.create_render_target(params).ok();
                }
                state.render();
                ValidateRect(hwnd, null_mut());
                Some(0)
            },
            WM_SIZE => unsafe {
                let mut state = self.state.borrow_mut();
                state.render_target.as_mut().and_then(|rt|
                    rt.hwnd_rt().map(|hrt|
                        hrt.Resize(&D2D1_SIZE_U {
                            width: LOWORD(lparam as u32) as u32,
                            height: HIWORD(lparam as u32) as u32,
                        })
                    )
                );
                None
            },
            WM_CHAR => {
                // let paramsString = format!("{:x} {:x} {:x}\n", msg, wparam, lparam);
                // let params = json!({"chars": paramsString});
                // self.send_edit_cmd("insert", &params);
                // println!("WM_CHAR {:x} {:x}", wparam, lparam);
                match wparam as i32 {
                    VK_BACK => {
                        self.send_edit_cmd("delete_backward", &json!([]));
                        Some(0)
                    },
                    VK_RETURN => {
                        self.send_edit_cmd("insert_newline", &json!([]));
                        Some(0)
                    },
                    _ => {
                        if let Some(c) = ::std::char::from_u32(wparam as u32) {
                            let params = json!({"chars": c.to_string()});
                            self.send_edit_cmd("insert", &params);
                            return Some(0)
                        }
                        None
                    }
                }
            }
            WM_KEYDOWN => {
                // Handle special keys here
                match wparam as i32 {
                    VK_UP => {
                        self.send_edit_cmd("move_up", &json!([]));
                        Some(0)
                    },
                    VK_DOWN => {
                        self.send_edit_cmd("move_down", &json!([]));
                        Some(0)
                    },
                    VK_LEFT => {
                        self.send_edit_cmd("move_left", &json!([]));
                        Some(0)
                    },
                    VK_RIGHT => {
                        self.send_edit_cmd("move_right", &json!([]));
                        Some(0)
                    },
                    VK_DELETE => {
                        self.send_edit_cmd("delete_forward", &json!([]));
                        Some(0)
                    },
                    _ => None
                }
            },
            WM_LBUTTONDOWN => {
                Some(0)
            },
            WM_COMMAND => unsafe {
                match wparam {
                    x if x == menus::MenuEntries::Exit as WPARAM => {
                        DestroyWindow(hwnd);
                    }
                    x if x == menus::MenuEntries::Open as WPARAM => {
                        self.file_open(hwnd);
                    }
                    x if x == menus::MenuEntries::Save as WPARAM => {
                        self.file_save(hwnd);
                    }
                    x if x == menus::MenuEntries::SaveAs as WPARAM => {
                        self.file_save_as(hwnd);
                    }
                    _ => return Some(1),
                }
                Some(0)
            },
            _ => None
        }
    }

    fn handle_cmd(&self, v: &Value) {
        let mut state = self.state.borrow_mut();
        //println!("got {:?}", v);
        if let Some(tab_name) = v["result"].as_str() {
            // TODO: should match up id etc. This is quick and dirty.
            state.view_id = tab_name.to_string();
        } else {
            let ref method = v["method"];
            if method == "update" {
                state.line_cache.apply_update(&v["params"]["update"]);
            }
        }
        state.label = serde_json::to_string(v).unwrap();
        unsafe { InvalidateRect(state.self_hwnd, null_mut(), 0); }
    }
}

fn create_main(optional_functions: &OptionalFunctions, xi_peer: XiPeer) -> Result<(HWND, Rc<Box<WndProc>>), Error> {
    unsafe {
        let class_name = "Xi Editor".to_wide();
        let icon = LoadIconW(0 as HINSTANCE, IDI_APPLICATION);
        let cursor = LoadCursorW(0 as HINSTANCE, IDC_IBEAM);
        let brush = CreateSolidBrush(0xffffff);
        let wnd = WNDCLASSW {
            style: 0,
            lpfnWndProc: Some(window::win_proc_dispatch),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: 0 as HINSTANCE,
            hIcon: icon,
            hCursor: cursor,
            hbrBackground: brush,
            lpszMenuName: 0 as LPCWSTR,
            lpszClassName: class_name.as_ptr(),
        };
        let class_atom = RegisterClassW(&wnd);
        if class_atom == 0 {
            return Err(Error::Null);
        }
        let main_state = MainWinState::new();
        let main_win: Rc<Box<WndProc>> = Rc::new(Box::new(
            MainWin::new(xi_peer, main_state)));

        // Simple scaling based on System Dpi (96 is equivalent to 100%)
        let dpi = if let Some(func) = optional_functions.GetDpiForSystem {
            // Only supported on windows 10
            func() as f32
        } else {
            // TODO GetDpiForMonitor is supported on windows 8.1, try falling back to that here
            96.0
        };
        let width = (500.0 * (dpi/96.0)) as i32;
        let height = (400.0 * (dpi/96.0)) as i32;

        let menus = Menus::create();
        let hmenu = menus.get_hmenubar();
        let hwnd = create_window(WS_EX_OVERLAPPEDWINDOW, class_name.as_ptr(),
            class_name.as_ptr(), WS_OVERLAPPEDWINDOW | WS_VSCROLL,
            CW_USEDEFAULT, CW_USEDEFAULT, width, height, 0 as HWND, hmenu, 0 as HINSTANCE,
            main_win.clone());
        if hwnd.is_null() {
            return Err(Error::Null);
        }
        Ok((hwnd, main_win))
    }
}

fn main() {
    let optional_functions = util::load_optional_functions();

    unsafe {
        if let Some(func) = optional_functions.SetProcessDpiAwareness {
            // This function is only supported on windows 10
            func(PROCESS_SYSTEM_DPI_AWARE); // TODO: per monitor (much harder)
        }

        let (xi_peer, rx, semaphore) = start_xi_thread();
        let (hwnd, main_win) = create_main(&optional_functions, xi_peer).unwrap();
        ShowWindow(hwnd, SW_SHOWNORMAL);
        UpdateWindow(hwnd);

        loop {
            let handles = [semaphore.get_handle()];
            let _res = MsgWaitForMultipleObjectsEx(
                handles.len() as u32,
                handles.as_ptr(),
                INFINITE,
                QS_ALLEVENTS,
                0
            );

            // Handle windows messages
            loop {
                let mut msg = mem::uninitialized();
                let res = PeekMessageW(&mut msg, null_mut(), 0, 0, PM_NOREMOVE);
                if res == 0 {
                    break;
                }
                let res = GetMessageW(&mut msg, null_mut(), 0, 0);
                if res <= 0 {
                    return;
                }
                TranslateMessage(&mut msg);
                DispatchMessageW(&mut msg);
            }

            // Handle xi events
            loop {
                match rx.try_recv() {
                    Ok(v) => main_win.handle_cmd(&v),
                    Err(TryRecvError::Disconnected) => {
                        println!("core disconnected");
                        break;
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }
        }
    }
}
