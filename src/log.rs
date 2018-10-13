// Copyright 2018 The xi-editor Authors.
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

//! Responsible for creating log file when stdout is invalid

use std::{ptr, thread};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::fs::File;
use std::mem::zeroed;
use std::os::windows::io::{FromRawHandle, RawHandle};

use winapi::um::winnt::HANDLE;
use winapi::um::{winbase, processenv, namedpipeapi};

pub fn init() {
    let handle = unsafe { processenv::GetStdHandle(winbase::STD_OUTPUT_HANDLE) };

    if handle == -1isize as HANDLE || handle == 0isize as HANDLE {
        unsafe {
            init_file_stdout();
        }

        println!("Created log file for stdout/stderr!");
    }
}


unsafe fn init_file_stdout() {
    let (stdout_read_pipe, stdout_write_pipe) = create_read_write_pipe();
    let (stderr_read_pipe, stderr_write_pipe) = create_read_write_pipe();

    let stdout_input = File::from_raw_handle(stdout_read_pipe);
    let stderr_input = File::from_raw_handle(stderr_read_pipe);

    create_handler_threads(stdout_input, stderr_input);

    processenv::SetStdHandle(winbase::STD_OUTPUT_HANDLE, stdout_write_pipe);
    processenv::SetStdHandle(winbase::STD_ERROR_HANDLE, stderr_write_pipe);
}

unsafe fn create_read_write_pipe() -> (RawHandle, RawHandle) {
    let mut read_pipe: RawHandle = zeroed();
    let mut write_pipe: RawHandle = zeroed();

    if namedpipeapi::CreatePipe(&mut read_pipe, &mut write_pipe, ptr::null_mut(), 4096) == 0 {
        panic!("Failed to create pipe!");
    }

    (read_pipe, write_pipe)
}

fn create_handler_threads(stdout: File, stderr: File) {
    let log_file = File::create("log.txt").unwrap_or_else(|e| {
        panic!("Failed to create log file: {}", e);
    });
    let file_mutex = Arc::new(Mutex::new(log_file));

    create_thread("stdout handler", stdout, file_mutex.clone());
    create_thread("stderr handler", stderr, file_mutex.clone());
}

fn create_thread(name: &str, input: File, output: Arc<Mutex<File>>) {
    thread::Builder::new()
        .name(String::from(name))
        .spawn({
            move || {
                let mut buffer = [0; 4096];
                let mut input = input;

                loop {
                    let bytes = read_from_pipe(&mut input, &mut buffer);
                    let mut file = output.lock().unwrap();
                    let buffer = &buffer[..bytes];
                    file.write(buffer).unwrap();
                }
            }
        })
        .unwrap();
}

fn read_from_pipe(input: &mut File, buffer: &mut [u8]) -> usize {
    match input.read(buffer) {
        Ok(bytes) => bytes,
        Err(e) => panic!("Failed to read from pipe: {}", e),
    }
}
