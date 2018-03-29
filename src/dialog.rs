#![allow(non_upper_case_globals)]

use winapi::Interface;
use winapi::shared::minwindef::*;
use winapi::shared::ntdef::LPWSTR;
use winapi::shared::windef::*;
use winapi::shared::winerror::*;
use winapi::shared::wtypesbase::*;
use winapi::um::combaseapi::*;
use winapi::um::shobjidl::*;
use winapi::um::shobjidl_core::*;

use std::ptr::null_mut;
use xi_win_shell::util::FromWide;

pub unsafe fn get_open_file_dialog_path(hwnd_owner: HWND) -> Option<String> {
  get_file_dialog_path(hwnd_owner, true)
}

pub unsafe fn get_save_file_dialog_path(hwnd_owner: HWND) -> Option<String> {
  get_file_dialog_path(hwnd_owner, false)
}

// TODO: remove these when they get added to winapi
DEFINE_GUID!{CLSID_FileOpenDialog,
  0xDC1C5A9C, 0xE88A, 0x4DDE, 0xA5, 0xA1, 0x60, 0xF8, 0x2A, 0x20, 0xAE, 0xF7}
DEFINE_GUID!{CLSID_FileSaveDialog,
  0xC0B4E2F3, 0xBA21, 0x4773, 0x8D, 0xBA, 0x33, 0x5E, 0xC9, 0x46, 0xEB, 0x8B}

unsafe fn get_file_dialog_path(hwnd_owner: HWND, open: bool) -> Option<String> {
  let mut filename: Option<String> = None;
  let mut pfd: *mut IFileDialog = null_mut();
  let class = if open { &CLSID_FileOpenDialog } else { &CLSID_FileSaveDialog };
  let id = if open { IFileOpenDialog::uuidof() } else { IFileSaveDialog::uuidof() };
  let hr = CoCreateInstance(class,
      null_mut(),
      CLSCTX_INPROC_SERVER,
      &id,
      &mut pfd as *mut *mut IFileDialog as *mut LPVOID
      );
  if hr != S_OK {
      return None; // TODO: should be error result
  }
  (*pfd).Show(hwnd_owner);
  let mut result: *mut IShellItem = null_mut();
  (*pfd).GetResult(&mut result);
  if !result.is_null() {
      let mut display_name: LPWSTR = null_mut();
      (*result).GetDisplayName(SIGDN_FILESYSPATH, &mut display_name);
      filename = display_name.from_wide();
      CoTaskMemFree(display_name as LPVOID);
      (*result).Release();
  } else {
      //println!("result is null");
  }
  (*pfd).Release();

  filename
}
