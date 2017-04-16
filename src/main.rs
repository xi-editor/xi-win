#![feature(rc_raw)] // will stabilize in 1.17
#![windows_subsystem = "windows"]

extern crate winapi;
extern crate user32;
extern crate gdi32;
extern crate direct2d;
extern crate directwrite;

extern crate xi_core_lib;
extern crate xi_rpc;

mod hwnd_rt;
mod util;
mod window;
mod xi_thread;

use std::cell::RefCell;
use std::mem;
use std::ptr::null_mut;
use std::sync::Arc;
use std::sync::atomic::{AtomicPtr, Ordering};
use std::rc::Rc;

use user32::*;
use winapi::*;
use direct2d::{RenderTarget, brush};
use direct2d::math::*;
use direct2d::render_target::DrawTextOption;
use directwrite::text_format::{self, TextFormat};

use hwnd_rt::HwndRtParams;
use util::{Error, ToWide};
use window::{create_window, WndProc};
use xi_thread::{start_xi_thread, XI_FROM_CORE, XI_MAGIC, XiPeer};

extern "system" {
    // defined in shcore library
    pub fn SetProcessDpiAwareness(value: PROCESS_DPI_AWARENESS) -> HRESULT;
}

struct Resources {
    fg: brush::SolidColor,
    bg: brush::SolidColor,
    text_format: TextFormat,
}

struct MainWinState {
    d2d_factory: direct2d::Factory,
    dwrite_factory: directwrite::Factory,
    render_target: Option<RenderTarget>,
    resources: Option<Resources>,
}

impl MainWinState {
    fn new() -> MainWinState {
        MainWinState {
            d2d_factory: direct2d::Factory::new().unwrap(),
            dwrite_factory: directwrite::Factory::new().unwrap(),
            render_target: None,
            resources: None,
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
            rt.draw_line(&Point2F::from((10.0, 50.0)), &Point2F::from((90.0, 90.0)),
                &resources.fg, 1.0, None);
            let msg = "Hello DWrite";
            rt.draw_text(
                msg,
                &resources.text_format,
                &RectF::from((10.0, 10.0, 300.0, 90.0)),
                &resources.fg,
                &[DrawTextOption::EnableColorFont]
            );
            rt.end_draw()
        };
        if res.is_err() {
            self.render_target = None;
        }
    }
}

struct MainWin {
    shared_hwnd: Arc<AtomicPtr<HWND__>>,
    peer: XiPeer,
    state: RefCell<MainWinState>,
}

impl MainWin {
    fn new(shared_hwnd: Arc<AtomicPtr<HWND__>>, peer: XiPeer, state: MainWinState) -> MainWin {
        MainWin {
            shared_hwnd: shared_hwnd,
            peer: peer,
            state: RefCell::new(state)
        }
    }
}

impl WndProc for MainWin {
    fn window_proc(&self, hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        //println!("{:x} {:x} {:x}", msg, wparam, lparam);
        match msg {
            WM_DESTROY => unsafe {
                self.shared_hwnd.store(null_mut(), Ordering::Release);
                PostQuitMessage(0);
                None
            },
            WM_PAINT => unsafe {
                let mut state = self.state.borrow_mut();
                if state.render_target.is_none() {
                    let mut rect: RECT = mem::uninitialized();
                    user32::GetClientRect(hwnd, &mut rect);
                    //println!("rect={:?}", rect);
                    let width = (rect.right - rect.left) as u32;
                    let height = (rect.bottom - rect.top) as u32;
                    let params = HwndRtParams { hwnd: hwnd, width: width, height: height };
                    state.render_target = state.d2d_factory.create_render_target(params).ok();
                }
                state.render();
                user32::ValidateRect(hwnd, null_mut());
                Some(0)
            },
            WM_SIZE => unsafe {
                let mut state = self.state.borrow_mut();
                state.render_target.as_mut().and_then(|rt|
                    rt.hwnd_rt().map(|mut hrt|
                        hrt.Resize(&D2D1_SIZE_U {
                            width: LOWORD(lparam as u32) as u32,
                            height: HIWORD(lparam as u32) as u32,
                        })
                    )
                );
                None
            },
            WM_LBUTTONDOWN => {
                self.peer.send(r#"{"method": "new_tab", "params": [], "id": "0"}"#.to_string());
                Some(0)
            },
            XI_FROM_CORE => unsafe {
                if wparam == XI_MAGIC {
                    // An assumption that only valid utf-8 is sent. Should be valid.
                    let buf = Box::from_raw(lparam as *mut String);
                    print!("{}", &buf);
                } else {
                    println!("wrong magic, got {:x} {:x}", wparam, lparam);
                }
                Some(0)
            },
            _ => None
        }
    }
}

fn create_main(shared_hwnd: Arc<AtomicPtr<HWND__>>, xi_peer: XiPeer) -> Result<HWND, Error> {
    unsafe {
        let class_name = "my_window".to_wide();
        let icon = LoadIconW(0 as HINSTANCE, IDI_APPLICATION);
        let cursor = LoadCursorW(0 as HINSTANCE, IDC_IBEAM);
        let brush = gdi32::CreateSolidBrush(0xffffff);
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
            MainWin::new(shared_hwnd, xi_peer, main_state)));
        let width = 500;  // TODO: scale by dpi
        let height = 400;
        let hwnd = create_window(winapi::WS_EX_OVERLAPPEDWINDOW, class_name.as_ptr(),
            class_name.as_ptr(), WS_OVERLAPPEDWINDOW | winapi::WS_VSCROLL,
            CW_USEDEFAULT, CW_USEDEFAULT, width, height, 0 as HWND, 0 as HMENU, 0 as HINSTANCE,
            main_win);
        if hwnd.is_null() {
            return Err(Error::Null);
        }
        Ok(hwnd)
    }
}

fn main() {
    unsafe {
        SetProcessDpiAwareness(Process_System_DPI_Aware);  // TODO: per monitor (much harder)
        let shared_hwnd = Arc::new(AtomicPtr::new(null_mut()));
        let xi_peer = start_xi_thread(shared_hwnd.clone());
        let hwnd = create_main(shared_hwnd.clone(), xi_peer).unwrap();
        shared_hwnd.store(hwnd, Ordering::Release);
        ShowWindow(hwnd, SW_SHOWNORMAL);
        UpdateWindow(hwnd);
        let mut msg = mem::uninitialized();
        loop {
            let bres = GetMessageW(&mut msg, null_mut(), 0, 0);
            if bres <= 0 {
                break;
            }
            TranslateMessage(&mut msg);
            DispatchMessageW(&mut msg);
        }
    }
}
