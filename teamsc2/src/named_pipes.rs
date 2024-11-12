use std::error::Error;
use windows_sys::Win32::{
    Foundation::{CloseHandle, HANDLE, INVALID_HANDLE_VALUE},
    Storage::FileSystem::{CreateFileA, ReadFile, WriteFile},
};

pub struct PipeConnection {
    handle: HANDLE,
}

// PipeConnection implements connect, read, and write to the named pipe and a trait to close the handle
impl PipeConnection {
    pub fn connect(name: &str) -> Result<Self, Box<dyn Error>> {
        // Convert the string to a null-terminated C string
        let c_name = std::ffi::CString::new(name).unwrap();
        
        let handle = unsafe {
            CreateFileA(
                c_name.as_ptr().cast(),
                0x80000000 | 0x40000000, // GENERIC_READ | GENERIC_WRITE
                0,
                std::ptr::null_mut(),
                3, // OPEN_EXISTING
                0,
                0,
            )
        };

        if handle == INVALID_HANDLE_VALUE {
            return Err("Failed to connect to pipe".into());
        }

        Ok(Self { handle })
    }

    pub fn read(&self) -> Result<String, Box<dyn Error>> {
        let mut buffer = vec![0u8; 8192]; // 8KB buffer
        let mut bytes_read = 0;

        unsafe {
            if ReadFile(
                self.handle,
                buffer.as_mut_ptr() as *mut u8,
                buffer.len() as u32,
                &mut bytes_read,
                std::ptr::null_mut(),
            ) == 0
            {
                return Err("Failed to read from pipe".into());
            }
        }
        // Convert buffer to string and return
        Ok(String::from_utf8_lossy(&buffer[..bytes_read as usize]).to_string())
    }

    pub fn write(&self, message: &str) -> Result<(), Box<dyn Error>> {
        let buffer = message.as_bytes();
        let mut bytes_written = 0;

        unsafe {
            if WriteFile(
                self.handle,
                buffer.as_ptr() as *const u8,
                buffer.len() as u32,
                &mut bytes_written,
                std::ptr::null_mut(),
            ) == 0
            {
                return Err("Failed to write to pipe".into());
            }
        }

        Ok(())
    }
}

impl Drop for PipeConnection {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.handle);
        }
    }
}

