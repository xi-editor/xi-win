#![feature(rc_raw)] // will stabilize in 1.17

extern crate winapi;
extern crate user32;
extern crate gdi32;
extern crate direct2d;

mod hwnd_rt;
mod util;
mod window;

use std::cell::RefCell;
use std::mem;
use std::ptr::null_mut;
use std::rc::Rc;

use user32::*;
use winapi::*;
use direct2d::{RenderTarget, brush};
use direct2d::math::*;

use hwnd_rt::HwndRtParams;
use util::{Error, ToWide};
use window::{create_window, WndProc};

struct MainWinState {
    d2d_factory: direct2d::Factory,
    render_target: Option<RenderTarget>,
    blue: Option<brush::SolidColor>,
    white: Option<brush::SolidColor>,
}

impl MainWinState {
    fn new() -> MainWinState {
        MainWinState {
            d2d_factory: direct2d::Factory::new().unwrap(),
            render_target: None,
            blue: None,
            white: None,
        }
    }

    fn render(&mut self) {
        let res = {
            let rt = self.render_target.as_mut().unwrap();
            rt.begin_draw();
            if self.blue.is_none() {
                self.blue = rt.create_solid_color_brush(0x101080, &BrushProperties::default()).ok();
            }
            if self.white.is_none() {
                self.white = rt.create_solid_color_brush(0xffffff, &BrushProperties::default()).ok();
            }
            let size = rt.get_size();
            let rect = RectF::from((0.0, 0.0, size.width, size.height));
            rt.fill_rectangle(&rect, self.white.as_ref().unwrap());
            rt.draw_line(&Point2F::from((10.0, 10.0)), &Point2F::from((90.0, 50.0)),
                self.blue.as_ref().unwrap(), 1.0, None);

            rt.end_draw()
        };
        if res.is_err() {
            self.render_target = None;
        }
    }
}

struct MainWin {
    state: RefCell<MainWinState>
}

impl MainWin {
    fn new(state: MainWinState) -> MainWin {
        MainWin {
            state: RefCell::new(state)
        }
    }
}

impl WndProc for MainWin {
    fn window_proc(&self, hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        //println!("{:x} {:x} {:x}", msg, wparam, lparam);
        match msg {
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
                    rt.hwnd_rt().map(|hrt|
                        (*hrt.raw_value()).Resize(&D2D1_SIZE_U {
                            width: LOWORD(lparam as u32) as u32,
                            height: HIWORD(lparam as u32) as u32,
                        })
                    )
                );
                None
            },
            _ => None
        }
    }
}

fn create_main() -> Result<HWND, Error> {
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
        let main_win: Rc<Box<WndProc>> = Rc::new(Box::new(MainWin::new(MainWinState::new())));
        let hwnd = create_window(winapi::WS_EX_OVERLAPPEDWINDOW, class_name.as_ptr(),
            class_name.as_ptr(), WS_OVERLAPPEDWINDOW | winapi::WS_VSCROLL,
            CW_USEDEFAULT, CW_USEDEFAULT, 500, 400, 0 as HWND, 0 as HMENU, 0 as HINSTANCE,
            main_win);
        if hwnd.is_null() {
            return Err(Error::Null);
        }
        Ok(hwnd)
    }
}

fn main() {
    let hwnd = create_main().unwrap();
    unsafe {
        ShowWindow(hwnd, SW_SHOWNORMAL);
        UpdateWindow(hwnd);
        let mut msg = mem::uninitialized();
        loop {
            let bres = GetMessageW(&mut msg, hwnd, 0, 0);
            if bres <= 0 {
                break;
            }
            TranslateMessage(&mut msg);
            DispatchMessageW(&mut msg);
        }
    }
}
