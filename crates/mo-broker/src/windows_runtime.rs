//! Fail-closed Windows Broker startup configuration.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use mo_rime::EngineConfig;

#[cfg(debug_assertions)]
pub const USAGE: &str = "usage: mo-broker --fake | (--rime | --rime-prepared) <absolute-rime.dll> <shared-data-dir> <user-data-dir>";
pub const INSTALLED_USAGE: &str = "installed mo-broker accepts no command-line arguments";

#[cfg(any(not(debug_assertions), test))]
const OPENCC_REQUIRED_FILES: [&str; 6] = [
    "emoji.json",
    "emoji.ocd2",
    "others.ocd2",
    "s2t.json",
    "STCharacters.ocd2",
    "STPhrases.ocd2",
];

#[cfg(not(debug_assertions))]
pub const USAGE: &str = INSTALLED_USAGE;

pub enum StartupMode {
    #[cfg(debug_assertions)]
    Fake,
    Rime(Box<RimeStartup>),
}

pub struct RimeStartup {
    pub dll_path: PathBuf,
    pub engine_config: EngineConfig,
    pub require_prepared_resources: bool,
}

/// Release builds only use OS Known Folders and the fixed installed layout.
#[cfg(not(debug_assertions))]
pub fn parse_startup(arguments: Vec<OsString>) -> io::Result<StartupMode> {
    parse_installed_arguments(&arguments)?;
    let layout = InstalledLayout::from_roots(mo_windows_platform::runtime_roots()?)?;
    prepare_release_startup(&std::env::current_exe()?, &layout)
}

/// Diagnostic startup is deliberately unavailable when debug assertions are off.
#[cfg(debug_assertions)]
pub fn parse_startup(arguments: Vec<OsString>) -> io::Result<StartupMode> {
    match arguments.as_slice() {
        [flag] if flag == "--fake" => Ok(StartupMode::Fake),
        [flag, dll, shared, user] if flag == "--rime" || flag == "--rime-prepared" => {
            let shared = runtime_directory(Path::new(shared), "shared data directory")?;
            let user = runtime_directory(Path::new(user), "user data directory")?;
            require_file(&shared, "default.yaml", "shared data directory")?;
            require_file(&shared, "rime_ice.schema.yaml", "shared data directory")?;
            require_file(&user, "build/default.yaml", "deployed user data")?;
            require_file(&user, "build/rime_ice.schema.yaml", "deployed user data")?;
            Ok(StartupMode::Rime(Box::new(RimeStartup {
                dll_path: PathBuf::from(dll),
                require_prepared_resources: flag == "--rime-prepared",
                engine_config: EngineConfig::new(
                    librime_path(&shared, "shared data directory")?,
                    librime_path(&user, "user data directory")?,
                ),
            })))
        }
        _ => Err(io::Error::new(io::ErrorKind::InvalidInput, USAGE)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledLayout {
    pub program_files_x64: PathBuf,
    pub install_root: PathBuf,
    pub local_app_data_root: PathBuf,
    pub broker_path: PathBuf,
    pub dll_path: PathBuf,
    pub opencc_data_dir: PathBuf,
    pub shared_data_dir: PathBuf,
    pub prebuilt_data_dir: PathBuf,
    pub user_data_dir: PathBuf,
    pub staging_dir: PathBuf,
}

impl InstalledLayout {
    pub fn from_roots(roots: mo_windows_platform::RuntimeRoots) -> io::Result<Self> {
        if !roots.program_files_x64.is_absolute() || !roots.local_app_data.is_absolute() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "runtime roots must be absolute",
            ));
        }
        let install = roots.program_files_x64.join("Mo");
        let shared = install.join("data/rime-ice");
        let user = roots.local_app_data.join("Mo/Rime");
        Ok(Self {
            program_files_x64: roots.program_files_x64,
            install_root: install.clone(),
            local_app_data_root: roots.local_app_data,
            broker_path: install.join("bin/mo-broker.exe"),
            dll_path: install.join("runtime/librime/rime.dll"),
            opencc_data_dir: install.join("runtime/librime/opencc"),
            prebuilt_data_dir: shared.join("build"),
            staging_dir: shared.join("build"),
            shared_data_dir: shared,
            user_data_dir: user,
        })
    }
}

#[cfg(any(not(debug_assertions), test))]
fn parse_installed_arguments(arguments: &[OsString]) -> io::Result<()> {
    if arguments.is_empty() {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::InvalidInput, INSTALLED_USAGE))
    }
}

#[cfg(any(not(debug_assertions), test))]
fn require_fixed_broker_image(image: &Path, layout: &InstalledLayout) -> io::Result<()> {
    // Compare final filesystem paths, not a caller-selected root or cwd.
    let image = std::fs::canonicalize(image)?;
    let expected = std::fs::canonicalize(&layout.broker_path).map_err(|_| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Broker must run from its fixed installed path",
        )
    })?;
    if image != expected {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "Broker must run from its fixed installed path",
        ));
    }
    Ok(())
}

#[cfg(any(not(debug_assertions), test))]
fn prepare_release_startup(image: &Path, layout: &InstalledLayout) -> io::Result<StartupMode> {
    // Reject relocated images before touching Program Files. Once identity is
    // fixed, audit every machine asset before creating user data or binding IPC.
    require_fixed_broker_image(image, layout)?;
    mo_windows_platform::validate_installation_tree(
        &layout.program_files_x64,
        &layout.install_root,
    )?;
    prepare_installed_startup(image, layout)
}

#[cfg(any(not(debug_assertions), test))]
fn prepare_installed_startup(image: &Path, layout: &InstalledLayout) -> io::Result<StartupMode> {
    // Private preparation remains independently testable with disposable
    // assets. The release entry point above always performs the trust audit.
    require_fixed_broker_image(image, layout)?;
    let shared = runtime_directory(&layout.shared_data_dir, "installed shared data")?;
    let prebuilt = runtime_directory(&layout.prebuilt_data_dir, "installed prebuilt data")?;
    let opencc = runtime_directory(&layout.opencc_data_dir, "installed OpenCC data")?;
    for file in OPENCC_REQUIRED_FILES {
        require_file(&opencc, file, "installed OpenCC data")?;
    }
    for file in ["default.yaml", "rime_ice.schema.yaml"] {
        require_file(&shared, file, "installed shared data")?;
        require_file(&prebuilt, file, "installed prebuilt data")?;
    }
    let dll = std::fs::canonicalize(&layout.dll_path)?;
    if !dll.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "installed librime image is not a file",
        ));
    }
    // Machine assets are validated before the first-run path mutates the
    // current user's profile. Create only Mo-owned direct descendants, one
    // component at a time, and reject pre-existing reparse points/files.
    ensure_managed_user_directories(layout)?;
    let user = runtime_directory(&layout.user_data_dir, "managed user data")?;
    let staging = runtime_directory(&layout.staging_dir, "managed staging data")?;
    let mut config = EngineConfig::new(
        librime_path(&shared, "installed shared data")?,
        librime_path(&user, "managed user data")?,
    );
    config.prebuilt_data_dir = Some(librime_path(&prebuilt, "installed prebuilt data")?);
    config.staging_dir = Some(librime_path(&staging, "managed staging data")?);
    Ok(StartupMode::Rime(Box::new(RimeStartup {
        dll_path: dll,
        engine_config: config,
        require_prepared_resources: true,
    })))
}

#[cfg(any(not(debug_assertions), test))]
fn ensure_managed_user_directories(layout: &InstalledLayout) -> io::Result<()> {
    let expected_mo = layout.local_app_data_root.join("Mo");
    let expected_user = expected_mo.join("Rime");
    if layout.user_data_dir != expected_user || layout.staging_dir != layout.prebuilt_data_dir {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "managed user paths and machine-only staging must remain fixed",
        ));
    }

    checked_directory(&layout.local_app_data_root, "LocalAppData root")?;
    for (path, label) in [
        (&expected_mo, "managed Mo directory"),
        (&expected_user, "managed Rime directory"),
    ] {
        match std::fs::create_dir(path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(io::Error::new(
                    error.kind(),
                    format!("failed to create {label} {}: {error}", path.display()),
                ));
            }
        }
        checked_directory(path, label)?;
    }
    reject_user_code_overrides(&expected_user)?;
    Ok(())
}

#[cfg(any(not(debug_assertions), test))]
fn reject_user_code_overrides(user_data_dir: &Path) -> io::Result<()> {
    for relative in ["rime.lua", "lua"] {
        let path = user_data_dir.join(relative);
        match std::fs::symlink_metadata(&path) {
            Ok(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!(
                        "installed mode does not allow user Lua override path: {}",
                        path.display()
                    ),
                ));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(io::Error::new(
                    error.kind(),
                    format!(
                        "failed to inspect user Lua override path {}: {error}",
                        path.display()
                    ),
                ));
            }
        }
    }
    Ok(())
}

#[cfg(any(not(debug_assertions), test))]
fn checked_directory(path: &Path, label: &str) -> io::Result<()> {
    use std::os::windows::fs::MetadataExt;

    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to inspect {label} {}: {error}", path.display()),
        )
    })?;
    if !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} is not a directory: {}", path.display()),
        ));
    }
    if metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{label} must not be a reparse point: {}", path.display()),
        ));
    }
    Ok(())
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
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir()
                .join(format!("mo-installed-plan-{}-{nonce}", std::process::id()));
            std::fs::create_dir(&root).unwrap();
            Self(root)
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            // Only this test's unique, successfully created disposable root.
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    #[cfg(debug_assertions)]
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
    #[cfg(debug_assertions)]
    fn debug_preparation_is_explicit_and_never_a_legacy_fallback() {
        let fixture = Fixture::new();
        let shared = fixture.0.join("shared");
        let user = fixture.0.join("user");
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::create_dir_all(user.join("build")).unwrap();
        for file in ["default.yaml", "rime_ice.schema.yaml"] {
            std::fs::write(shared.join(file), b"fixture").unwrap();
            std::fs::write(user.join("build").join(file), b"fixture").unwrap();
        }
        for (flag, required) in [("--rime", false), ("--rime-prepared", true)] {
            let mode = parse_startup(vec![
                flag.into(),
                fixture.0.join("rime.dll").into_os_string(),
                shared.clone().into_os_string(),
                user.clone().into_os_string(),
            ])
            .unwrap();
            let StartupMode::Rime(startup) = mode else {
                panic!("explicit native mode must not fall back to fake");
            };
            assert_eq!(startup.require_prepared_resources, required);
        }
    }

    #[test]
    fn installed_policy_rejects_diagnostic_and_unknown_arguments() {
        assert!(parse_installed_arguments(&[]).is_ok());
        for arguments in [
            vec![OsString::from("--fake")],
            vec![
                OsString::from("--rime"),
                OsString::from(r"C:\untrusted\rime.dll"),
            ],
            vec![OsString::from("--installed")],
            vec![OsString::from("--rime-prepared")],
        ] {
            let error = parse_installed_arguments(&arguments).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
            assert_eq!(error.to_string(), INSTALLED_USAGE);
        }
    }

    #[test]
    fn installed_layout_is_fixed_and_rejects_relative_roots() {
        let roots = mo_windows_platform::RuntimeRoots {
            program_files_x64: PathBuf::from(r"C:\Program Files"),
            local_app_data: PathBuf::from(r"C:\Users\test\AppData\Local"),
        };
        let layout = InstalledLayout::from_roots(roots.clone()).unwrap();
        assert_eq!(layout.program_files_x64, roots.program_files_x64);
        assert_eq!(layout.install_root, roots.program_files_x64.join("Mo"));
        assert_eq!(
            layout.broker_path,
            roots.program_files_x64.join("Mo/bin/mo-broker.exe")
        );
        assert_eq!(
            layout.dll_path,
            roots.program_files_x64.join("Mo/runtime/librime/rime.dll")
        );
        assert_eq!(
            layout.opencc_data_dir,
            roots.program_files_x64.join("Mo/runtime/librime/opencc")
        );
        assert_eq!(
            layout.prebuilt_data_dir,
            layout.shared_data_dir.join("build")
        );
        assert_eq!(layout.staging_dir, layout.prebuilt_data_dir);
        assert_eq!(layout.user_data_dir, roots.local_app_data.join("Mo/Rime"));
        assert_eq!(layout.local_app_data_root, roots.local_app_data);
        assert!(
            InstalledLayout::from_roots(mo_windows_platform::RuntimeRoots {
                program_files_x64: PathBuf::from("relative"),
                ..roots
            })
            .is_err()
        );
    }

    #[test]
    fn repository_image_cannot_enter_installed_mode() {
        let layout =
            InstalledLayout::from_roots(mo_windows_platform::runtime_roots().unwrap()).unwrap();
        let error = prepare_release_startup(&std::env::current_exe().unwrap(), &layout)
            .err()
            .expect("non-installed image must be rejected");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(
            error.to_string(),
            "Broker must run from its fixed installed path"
        );
    }

    #[test]
    fn release_gate_rejects_an_untrusted_tree_before_user_bootstrap() {
        let fixture = Fixture::new();
        let layout = InstalledLayout::from_roots(mo_windows_platform::RuntimeRoots {
            program_files_x64: fixture.0.join("machine"),
            local_app_data: fixture.0.join("local-app-data"),
        })
        .unwrap();
        std::fs::create_dir_all(layout.broker_path.parent().unwrap()).unwrap();
        std::fs::write(&layout.broker_path, b"fixture only; never executed").unwrap();

        let error = prepare_release_startup(&layout.broker_path, &layout)
            .err()
            .expect("user-owned machine tree must be rejected");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(error.to_string().contains("owner is not trusted"));
        assert!(!layout.user_data_dir.exists());
    }

    #[test]
    fn installed_plan_requires_complete_assets_and_separates_prebuilt_from_user_data() {
        let fixture = Fixture::new();
        let layout = InstalledLayout::from_roots(mo_windows_platform::RuntimeRoots {
            program_files_x64: fixture.0.join("machine"),
            local_app_data: fixture.0.join("local-app-data"),
        })
        .unwrap();
        std::fs::create_dir(&layout.local_app_data_root).unwrap();
        for directory in [
            layout.broker_path.parent().unwrap(),
            layout.dll_path.parent().unwrap(),
            &layout.prebuilt_data_dir,
            &layout.opencc_data_dir,
        ] {
            std::fs::create_dir_all(directory).unwrap();
        }
        for file in [&layout.broker_path, &layout.dll_path] {
            std::fs::write(file, b"fixture only; never loaded").unwrap();
        }
        for file in OPENCC_REQUIRED_FILES {
            std::fs::write(
                layout.opencc_data_dir.join(file),
                b"fixture only; never loaded",
            )
            .unwrap();
        }
        for root in [&layout.shared_data_dir, &layout.prebuilt_data_dir] {
            for file in ["default.yaml", "rime_ice.schema.yaml"] {
                std::fs::write(root.join(file), b"fixture").unwrap();
            }
        }
        let mode = prepare_installed_startup(&layout.broker_path, &layout).unwrap();
        assert!(layout.user_data_dir.is_dir());
        assert!(layout.staging_dir.is_dir());
        assert!(!layout.user_data_dir.join("build").exists());
        #[cfg(not(debug_assertions))]
        let StartupMode::Rime(startup) = mode;
        #[cfg(debug_assertions)]
        let StartupMode::Rime(startup) = mode else {
            panic!("installed mode must use Rime")
        };
        assert_eq!(
            startup.dll_path,
            std::fs::canonicalize(&layout.dll_path).unwrap()
        );
        assert!(startup.require_prepared_resources);
        assert_eq!(
            startup.engine_config.prebuilt_data_dir,
            Some(
                librime_path(
                    &std::fs::canonicalize(&layout.prebuilt_data_dir).unwrap(),
                    "test"
                )
                .unwrap()
            )
        );
        assert_eq!(
            startup.engine_config.staging_dir,
            Some(
                librime_path(&std::fs::canonicalize(&layout.staging_dir).unwrap(), "test").unwrap()
            )
        );
        assert!(
            !layout
                .user_data_dir
                .join("build/rime_ice.schema.yaml")
                .exists()
        );

        for file in OPENCC_REQUIRED_FILES {
            let path = layout.opencc_data_dir.join(file);
            std::fs::remove_file(&path).unwrap();
            assert_eq!(
                prepare_installed_startup(&layout.broker_path, &layout)
                    .err()
                    .expect("missing OpenCC resource must be rejected")
                    .kind(),
                io::ErrorKind::NotFound
            );
            std::fs::write(path, b"fixture only; never loaded").unwrap();
        }
        std::fs::remove_file(layout.prebuilt_data_dir.join("rime_ice.schema.yaml")).unwrap();
        assert_eq!(
            prepare_installed_startup(&layout.broker_path, &layout)
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::NotFound
        );
        assert_eq!(
            prepare_installed_startup(&std::env::current_exe().unwrap(), &layout)
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::PermissionDenied
        );
    }

    #[test]
    fn managed_user_bootstrap_rejects_path_escape_and_existing_files() {
        let fixture = Fixture::new();
        let local = fixture.0.join("local");
        std::fs::create_dir(&local).unwrap();
        let mut layout = InstalledLayout::from_roots(mo_windows_platform::RuntimeRoots {
            program_files_x64: fixture.0.join("machine"),
            local_app_data: local.clone(),
        })
        .unwrap();

        layout.staging_dir = fixture.0.join("outside");
        assert_eq!(
            ensure_managed_user_directories(&layout).unwrap_err().kind(),
            io::ErrorKind::PermissionDenied
        );

        layout.staging_dir = layout.prebuilt_data_dir.clone();
        std::fs::write(local.join("Mo"), b"not a directory").unwrap();
        assert_eq!(
            ensure_managed_user_directories(&layout).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        assert!(!layout.user_data_dir.exists());
    }

    #[test]
    fn installed_mode_rejects_user_lua_override_surfaces() {
        for relative in ["rime.lua", "lua"] {
            let fixture = Fixture::new();
            let local = fixture.0.join("local");
            std::fs::create_dir(&local).unwrap();
            let layout = InstalledLayout::from_roots(mo_windows_platform::RuntimeRoots {
                program_files_x64: fixture.0.join("machine"),
                local_app_data: local,
            })
            .unwrap();
            std::fs::create_dir_all(&layout.user_data_dir).unwrap();
            let trap = layout.user_data_dir.join(relative);
            if relative == "lua" {
                std::fs::create_dir(&trap).unwrap();
            } else {
                std::fs::write(&trap, b"error('must never run')").unwrap();
            }

            let error = ensure_managed_user_directories(&layout).unwrap_err();
            assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
            assert!(error.to_string().contains("user Lua override path"));
        }
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
