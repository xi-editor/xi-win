extern crate ole32;
extern crate uuid;
extern crate winapi;

use winapi::*;
use std::ptr::null_mut;
use util::{FromWide};

pub unsafe fn get_open_file_dialog_path(hwnd_owner: HWND) -> Option<String> {
  get_file_dialog_path(hwnd_owner, true)
}

pub unsafe fn get_save_file_dialog_path(hwnd_owner: HWND) -> Option<String> {
  get_file_dialog_path(hwnd_owner, false)
}

unsafe fn get_file_dialog_path(hwnd_owner: HWND, open: bool) -> Option<String> {
  let mut filename: Option<String> = None;
  let mut pfd: *mut IFileDialog = null_mut();
  let class = if open { &uuid::CLSID_FileOpenDialog } else { &uuid::CLSID_FileSaveDialog };
  let id = if open { &uuid::IID_IFileOpenDialog } else { &uuid::IID_IFileSaveDialog };
  let hr = ole32::CoCreateInstance(class,
      null_mut(),
      winapi::CLSCTX_INPROC_SERVER,
      id,
      &mut pfd as *mut *mut winapi::IFileDialog as *mut winapi::LPVOID
      );
  if hr != winapi::S_OK {
      return None; // TODO: should be error result
  }
  (*pfd).Show(hwnd_owner);
  let mut result: *mut winapi::IShellItem = null_mut();
  (*pfd).GetResult(&mut result);
  if !result.is_null() {
      let mut display_name: LPWSTR = null_mut();
      (*result).GetDisplayName(SIGDN_FILESYSPATH, &mut display_name);
      filename = display_name.from_wide();
      ole32::CoTaskMemFree(display_name as LPVOID);
      (*result).Release();
  } else {
      //println!("result is null");
  }
  (*pfd).Release();

  filename
}
