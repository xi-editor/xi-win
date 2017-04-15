#![feature(rc_raw)] // will stabilize in 1.17

extern crate winapi;
extern crate user32;
extern crate gdi32;

mod util;
mod window;

use std::mem;
use std::rc::Rc;

use user32::*;
use winapi::*;

use util::{Error, ToWide};
use window::{create_window, WndProc};

struct MainWin;

impl WndProc for MainWin {
    fn window_proc(&self, _hwnd: HWND, msg: UINT, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
        println!("{:x} {:x} {:x}", msg, wparam, lparam);
        None
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
        let main_win: Rc<Box<WndProc>> = Rc::new(Box::new(MainWin));
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
