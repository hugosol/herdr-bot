//! Real Windows oplock; no product hooks or lock introspection.
use std::{ffi::c_void, fs, os::windows::{fs::OpenOptionsExt, io::AsRawHandle}};
#[repr(C)]
struct Overlapped { internal: usize, internal_high: usize, offset: u32, offset_high: u32, event: *mut c_void }
#[link(name = "kernel32")]
unsafe extern "system" {
    fn CreateEventW(attributes: *const c_void, manual: i32, initial: i32, name: *const u16) -> *mut c_void;
    fn DeviceIoControl(file: *mut c_void, code: u32, input: *const c_void, input_size: u32, output: *mut c_void, output_size: u32, bytes: *mut u32, overlapped: *mut Overlapped) -> i32;
    fn WaitForSingleObject(handle: *mut c_void, milliseconds: u32) -> u32;
    fn CancelIoEx(file: *mut c_void, overlapped: *mut Overlapped) -> i32;
    fn GetOverlappedResult(file: *mut c_void, overlapped: *mut Overlapped, bytes: *mut u32, wait: i32) -> i32;
    fn CloseHandle(handle: *mut c_void) -> i32;
}
pub struct Gate { file: Option<fs::File>, overlapped: Box<Overlapped> }
impl Gate {
    pub fn new(path: &std::path::Path) -> Self {
        let file = fs::OpenOptions::new().read(true).write(true).share_mode(7).custom_flags(0x40000000).open(path).unwrap();
        let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
        assert!(!event.is_null(), "{}", std::io::Error::last_os_error());
        let mut gate = Self { file: Some(file), overlapped: Box::new(Overlapped { internal: 0, internal_high: 0, offset: 0, offset_high: 0, event }) };
        let result = unsafe { DeviceIoControl(gate.file.as_ref().unwrap().as_raw_handle(), 0x90000, std::ptr::null(), 0, std::ptr::null_mut(), 0, std::ptr::null_mut(), &mut *gate.overlapped) };
        assert_eq!(result, 0);
        assert_eq!(std::io::Error::last_os_error().raw_os_error(), Some(997), "oplock must be pending");
        gate
    }
    pub fn reached(&self) -> bool { unsafe { WaitForSingleObject(self.overlapped.event, 0) == 0 } }
}
impl Drop for Gate {
    fn drop(&mut self) {
        if let Some(file) = self.file.take() {
            let mut bytes = 0;
            unsafe {
                CancelIoEx(file.as_raw_handle(), &mut *self.overlapped);
                GetOverlappedResult(file.as_raw_handle(), &mut *self.overlapped, &mut bytes, 1);
            }
            drop(file); // Acknowledge the break; release the competing open.
        }
        unsafe { CloseHandle(self.overlapped.event); }
    }
}
