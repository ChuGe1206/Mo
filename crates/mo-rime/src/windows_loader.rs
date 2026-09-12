use std::ffi::c_void;
use std::fmt;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::ptr::NonNull;

use mo_rime_sys as sys;
use windows_sys::Win32::Foundation::{FreeLibrary, GetLastError, HMODULE};
use windows_sys::Win32::System::LibraryLoader::{
    GetProcAddress, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32, LoadLibraryExW,
};

const RIME_DLL_NAME: &str = "rime.dll";
const RIME_GET_API: &[u8] = b"rime_get_api\0";

/// Failure while resolving Mo's explicitly configured librime runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeLibraryError {
    PathMustBeAbsolute(PathBuf),
    WrongFileName(PathBuf),
    ResolveFailed {
        path: PathBuf,
        os_code: Option<i32>,
    },
    NotAFile(PathBuf),
    LoadFailed {
        path: PathBuf,
        win32_code: u32,
    },
    MissingSymbol {
        symbol: &'static str,
        win32_code: u32,
    },
}

impl fmt::Display for RuntimeLibraryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PathMustBeAbsolute(path) => {
                write!(
                    formatter,
                    "librime path must be absolute: {}",
                    path.display()
                )
            }
            Self::WrongFileName(path) => write!(
                formatter,
                "librime path must name `{RIME_DLL_NAME}` exactly: {}",
                path.display()
            ),
            Self::ResolveFailed { path, os_code } => write!(
                formatter,
                "failed to resolve librime path {} (OS error {:?})",
                path.display(),
                os_code
            ),
            Self::NotAFile(path) => {
                write!(formatter, "librime path is not a file: {}", path.display())
            }
            Self::LoadFailed { path, win32_code } => write!(
                formatter,
                "failed to load librime from {} (Win32 error {win32_code})",
                path.display()
            ),
            Self::MissingSymbol { symbol, win32_code } => write!(
                formatter,
                "librime does not export `{symbol}` (Win32 error {win32_code})"
            ),
        }
    }
}

impl std::error::Error for RuntimeLibraryError {}

/// Owns the DLL until after [`crate::Engine`] finalizes librime.
pub(crate) struct LoadedLibrary {
    module: NonNull<c_void>,
}

impl LoadedLibrary {
    pub(crate) fn load(path: &Path) -> Result<Self, RuntimeLibraryError> {
        if !path.is_absolute() {
            return Err(RuntimeLibraryError::PathMustBeAbsolute(path.to_owned()));
        }
        if !path
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case(RIME_DLL_NAME))
        {
            return Err(RuntimeLibraryError::WrongFileName(path.to_owned()));
        }

        let canonical =
            std::fs::canonicalize(path).map_err(|error| RuntimeLibraryError::ResolveFailed {
                path: path.to_owned(),
                os_code: error.raw_os_error(),
            })?;
        if !canonical.is_file() {
            return Err(RuntimeLibraryError::NotAFile(canonical));
        }
        if !canonical
            .file_name()
            .is_some_and(|name| name.eq_ignore_ascii_case(RIME_DLL_NAME))
        {
            return Err(RuntimeLibraryError::WrongFileName(canonical));
        }

        let wide = canonical
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<_>>();
        // SAFETY: `wide` is a live NUL-terminated UTF-16 absolute path. The
        // flags intentionally restrict dependency resolution to this DLL's
        // directory and System32, excluding PATH and the current directory.
        let module = unsafe {
            LoadLibraryExW(
                wide.as_ptr(),
                std::ptr::null_mut(),
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
            )
        };
        let module = NonNull::new(module).ok_or_else(|| RuntimeLibraryError::LoadFailed {
            path: canonical,
            // SAFETY: this thread has made no intervening Win32 call since the
            // failing LoadLibraryExW.
            win32_code: unsafe { GetLastError() },
        })?;
        Ok(Self { module })
    }

    pub(crate) unsafe fn rime_api(&self) -> Result<*mut sys::RimeApi, RuntimeLibraryError> {
        // SAFETY: the module is live and the byte string is NUL-terminated.
        let symbol = unsafe { GetProcAddress(self.module.as_ptr(), RIME_GET_API.as_ptr()) };
        let symbol = symbol.ok_or_else(|| RuntimeLibraryError::MissingSymbol {
            symbol: "rime_get_api",
            // SAFETY: this thread has made no intervening Win32 call since the
            // failing GetProcAddress.
            win32_code: unsafe { GetLastError() },
        })?;
        // SAFETY: GetProcAddress returned the address of librime's documented
        // C export. Reinterpreting the address changes the call ABI from the
        // generic FARPROC type to the export's actual signature.
        let get_api: unsafe extern "C" fn() -> *mut sys::RimeApi =
            unsafe { std::mem::transmute(symbol) };
        // SAFETY: the caller requested the native API and keeps `self` alive.
        Ok(unsafe { get_api() })
    }
}

impl Drop for LoadedLibrary {
    fn drop(&mut self) {
        // SAFETY: this instance owns one successful LoadLibraryExW reference.
        unsafe {
            FreeLibrary(self.module.as_ptr() as HMODULE);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_test_dir() -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("mo-rime-loader-{}-{nonce}", std::process::id()))
    }

    #[test]
    fn relative_path_is_rejected_before_resolution() {
        assert_eq!(
            LoadedLibrary::load(Path::new("rime.dll")).err(),
            Some(RuntimeLibraryError::PathMustBeAbsolute(PathBuf::from(
                "rime.dll"
            )))
        );
    }

    #[test]
    fn unexpected_dll_name_is_rejected_before_loading() {
        let path = std::env::temp_dir().join("not-rime.dll");
        assert_eq!(
            LoadedLibrary::load(&path).err(),
            Some(RuntimeLibraryError::WrongFileName(path))
        );
    }

    #[test]
    fn missing_rime_image_fails_before_loading() {
        let path = unique_test_dir().join(RIME_DLL_NAME);
        assert!(matches!(
            LoadedLibrary::load(&path),
            Err(RuntimeLibraryError::ResolveFailed { .. })
        ));
    }
}
