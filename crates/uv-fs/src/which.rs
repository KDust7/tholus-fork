use std::ffi::OsStr;
use std::path::{Path, PathBuf};

#[cfg(not(target_family = "wasm"))]
pub fn which<T: AsRef<OsStr>>(binary_name: T) -> Result<PathBuf, which::Error> {
    which::which(binary_name)
}

#[cfg(target_family = "wasm")]
pub fn which<T: AsRef<OsStr>>(_binary_name: T) -> Result<PathBuf, which::Error> {
    Err(which::Error::CannotFindBinaryPath)
}

#[cfg(not(target_family = "wasm"))]
pub fn which_all<T: AsRef<OsStr>>(
    binary_name: T,
) -> Result<impl Iterator<Item = PathBuf>, which::Error> {
    which::which_all(binary_name)
}

#[cfg(target_family = "wasm")]
pub fn which_all<T: AsRef<OsStr>>(
    _binary_name: T,
) -> Result<impl Iterator<Item = PathBuf>, which::Error> {
    Ok(std::iter::empty())
}

#[cfg(not(target_family = "wasm"))]
pub fn which_in_global<T: AsRef<OsStr>, U: AsRef<OsStr>>(
    binary_name: T,
    paths: Option<U>,
) -> Result<impl Iterator<Item = PathBuf>, which::Error> {
    which::which_in_global(binary_name, paths)
}

#[cfg(target_family = "wasm")]
pub fn which_in_global<T: AsRef<OsStr>, U: AsRef<OsStr>>(
    _binary_name: T,
    _paths: Option<U>,
) -> Result<impl Iterator<Item = PathBuf>, which::Error> {
    Ok(std::iter::empty())
}

#[cfg(windows)]
#[allow(unsafe_code)] // We need to do an FFI call through the windows-* crates.
fn get_binary_type(path: &Path) -> windows::core::Result<u32> {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Storage::FileSystem::GetBinaryTypeW;
    use windows::core::PCWSTR;

    // References:
    // https://github.com/denoland/deno/blob/01a6379505712be34ebf2cdc874fa7f54a6e9408/runtime/permissions/which.rs#L131-L154
    // https://github.com/conradkleinespel/rooster/blob/afa78dc9918535752c4af59d2f812197ad754e5a/src/quale.rs#L51-L77
    let mut binary_type = 0u32;
    let name = path
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<u16>>();
    // SAFETY: winapi call
    unsafe { GetBinaryTypeW(PCWSTR(name.as_ptr()), &raw mut binary_type)? };
    Ok(binary_type)
}

/// Check whether a path in PATH is a valid executable.
///
/// Derived from `which`'s `Checker`.
pub fn is_executable(path: &Path) -> bool {
    #[cfg(any(unix, target_os = "wasi", target_os = "redox"))]
    {
        if rustix::fs::access(path, rustix::fs::Access::EXEC_OK).is_err() {
            return false;
        }
    }

    #[cfg(target_os = "windows")]
    {
        let Ok(file_type) = uv_vfs::fs::symlink_metadata(path).map(|metadata| metadata.file_type())
        else {
            return false;
        };
        if !file_type.is_file() && !file_type.is_symlink() {
            return false;
        }
        if path.extension().is_none() && get_binary_type(path).is_err() {
            return false;
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        #[cfg(unix)]
        use std::os::unix::fs::PermissionsExt;

        if !uv_vfs::fs::metadata(path)
            .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        {
            return false;
        }
    }

    true
}
