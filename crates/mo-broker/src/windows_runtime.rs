//! Fail-closed Windows Broker startup configuration.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use mo_rime::EngineConfig;

pub const USAGE: &str =
    "usage: mo-broker --fake | --rime <absolute-rime.dll> <shared-data-dir> <user-data-dir>";

pub enum StartupMode {
    Fake,
    Rime(Box<RimeStartup>),
}

pub struct RimeStartup {
    pub dll_path: PathBuf,
    pub engine_config: EngineConfig,
}

pub fn parse_startup(arguments: Vec<OsString>) -> io::Result<StartupMode> {
    match arguments.as_slice() {
        [flag] if flag == "--fake" => Ok(StartupMode::Fake),
        [flag, dll, shared, user] if flag == "--rime" => {
            let shared = runtime_directory(Path::new(shared), "shared data directory")?;
            let user = runtime_directory(Path::new(user), "user data directory")?;
            require_file(&shared, "default.yaml", "shared data directory")?;
            require_file(&shared, "rime_ice.schema.yaml", "shared data directory")?;
            require_file(&user, "build/default.yaml", "deployed user data")?;
            require_file(&user, "build/rime_ice.schema.yaml", "deployed user data")?;
            Ok(StartupMode::Rime(Box::new(RimeStartup {
                dll_path: PathBuf::from(dll),
                engine_config: EngineConfig::new(
                    librime_path(&shared, "shared data directory")?,
                    librime_path(&user, "user data directory")?,
                ),
            })))
        }
        _ => Err(io::Error::new(io::ErrorKind::InvalidInput, USAGE)),
    }
}

fn runtime_directory(path: &Path, label: &str) -> io::Result<PathBuf> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must be absolute: {}", path.display()),
        ));
    }
    let resolved = std::fs::canonicalize(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to resolve {label} {}: {error}", path.display()),
        )
    })?;
    if !resolved.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} is not a directory: {}", resolved.display()),
        ));
    }
    Ok(resolved)
}

fn require_file(root: &Path, relative: &str, label: &str) -> io::Result<()> {
    let path = root.join(relative);
    if !path.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{label} is missing required file: {}", path.display()),
        ));
    }
    Ok(())
}

fn librime_path(path: &Path, label: &str) -> io::Result<String> {
    let path = path.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} is not valid Unicode: {}", path.display()),
        )
    })?;
    // `std::fs::canonicalize` returns a verbatim `\\?\` path on Windows.
    // librime and librime-lua accept normal absolute DOS/UNC paths, but some
    // of their path joins do not preserve verbatim-prefix semantics.
    Ok(if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{rest}")
    } else if let Some(rest) = path.strip_prefix(r"\\?\") {
        rest.to_owned()
    } else {
        path.to_owned()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_mode_must_be_explicit() {
        let error = parse_startup(Vec::new()).err().expect("missing mode error");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(error.to_string(), USAGE);
        assert!(matches!(
            parse_startup(vec![OsString::from("--fake")]),
            Ok(StartupMode::Fake)
        ));
    }

    #[test]
    fn librime_paths_drop_windows_verbatim_prefixes() {
        assert_eq!(
            librime_path(Path::new(r"\\?\C:\ProgramData\Mo"), "test").unwrap(),
            r"C:\ProgramData\Mo"
        );
        assert_eq!(
            librime_path(Path::new(r"\\?\UNC\server\share\Mo"), "test").unwrap(),
            r"\\server\share\Mo"
        );
    }
}
