//! Small, owned Windows platform primitives, separate from input transport.

use std::io;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeRoots {
    pub program_files_x64: PathBuf,
    pub local_app_data: PathBuf,
}

/// Query OS-owned Known Folder locations for the current process user.
/// This never consults ProgramFiles/LOCALAPPDATA environment variables.
pub fn runtime_roots() -> io::Result<RuntimeRoots> {
    #[cfg(windows)]
    {
        windows::runtime_roots()
    }
    #[cfg(not(windows))]
    {
        Err(io::Error::new(io::ErrorKind::Unsupported, "Windows only"))
    }
}

#[cfg(windows)]
mod windows {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use std::ptr;

    use windows_sys::Win32::Foundation::RPC_E_CHANGED_MODE;
    use windows_sys::Win32::System::Com::{
        COINIT_APARTMENTTHREADED, CoInitializeEx, CoTaskMemFree, CoUninitialize,
    };
    use windows_sys::Win32::UI::Shell::{
        FOLDERID_LocalAppData, FOLDERID_ProgramFilesX64, SHGetKnownFolderPath,
    };
    use windows_sys::core::GUID;

    use super::*;

    struct ComScope(bool);

    impl ComScope {
        fn initialize() -> io::Result<Self> {
            // SAFETY: reserved pointer is null; the guard balances every
            // successful initialization, including S_FALSE, on this thread.
            let result = unsafe { CoInitializeEx(ptr::null(), COINIT_APARTMENTTHREADED as u32) };
            if result >= 0 {
                Ok(Self(true))
            } else if result == RPC_E_CHANGED_MODE {
                // COM is already initialized in another apartment mode.
                Ok(Self(false))
            } else {
                Err(hresult_error("CoInitializeEx", result))
            }
        }
    }

    impl Drop for ComScope {
        fn drop(&mut self) {
            if self.0 {
                // SAFETY: this scope owns one successful CoInitializeEx on
                // the current thread and cannot escape runtime_roots.
                unsafe { CoUninitialize() };
            }
        }
    }

    struct ShellString(*mut u16);

    impl Drop for ShellString {
        fn drop(&mut self) {
            // SAFETY: SHGetKnownFolderPath transfers task-memory ownership;
            // its output must be freed even on failure. Null is permitted.
            unsafe { CoTaskMemFree(self.0.cast()) };
        }
    }

    fn hresult_error(operation: &str, result: i32) -> io::Error {
        io::Error::other(format!(
            "{operation} failed: HRESULT 0x{:08x}",
            result as u32
        ))
    }

    fn known_folder(id: &GUID) -> io::Result<PathBuf> {
        let mut output = ShellString(ptr::null_mut());
        // SAFETY: valid GUID/output pointers, default flags, null token means
        // the current process user. COM is initialized by the enclosing scope.
        let result = unsafe { SHGetKnownFolderPath(id, 0, ptr::null_mut(), &mut output.0) };
        if result < 0 {
            return Err(hresult_error("SHGetKnownFolderPath", result));
        }
        if output.0.is_null() {
            return Err(io::Error::other("Known Folder returned a null path"));
        }

        let mut length = 0;
        // SAFETY: on success the API guarantees a null-terminated UTF-16
        // string. Each read precedes that terminator; ownership stays alive.
        while unsafe { *output.0.add(length) } != 0 {
            length += 1;
        }
        // SAFETY: length was obtained from this allocation's terminator and
        // the owned copy is made before ShellString releases the buffer.
        let path = PathBuf::from(OsString::from_wide(unsafe {
            std::slice::from_raw_parts(output.0, length)
        }));
        if !path.is_absolute() {
            return Err(io::Error::other(
                "Known Folder returned a non-absolute path",
            ));
        }
        Ok(path)
    }

    pub(super) fn runtime_roots() -> io::Result<RuntimeRoots> {
        let _com = ComScope::initialize()?;
        Ok(RuntimeRoots {
            program_files_x64: known_folder(&FOLDERID_ProgramFilesX64)?,
            local_app_data: known_folder(&FOLDERID_LocalAppData)?,
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn known_folders_are_absolute_and_repeated_calls_balance_com() {
            let first = runtime_roots().unwrap();
            assert!(first.program_files_x64.is_absolute());
            assert!(first.local_app_data.is_absolute());
            assert_eq!(first, runtime_roots().unwrap());
        }
    }
}
