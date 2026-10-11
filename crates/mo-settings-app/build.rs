use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../../assets/icons/mo-app-icon.ico");
    println!("cargo:rerun-if-changed=mo-settings.manifest");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc")
    {
        return;
    }
    let kits = env::var_os("WindowsSdkDir")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env::var_os("ProgramFiles(x86)").expect("Windows ProgramFiles(x86)"))
                .join("Windows Kits/10")
        });
    let mut versions: Vec<_> = fs::read_dir(kits.join("bin"))
        .expect("Windows SDK bin directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    versions.sort();
    let compiler = versions
        .into_iter()
        .rev()
        .map(|path| path.join("x64/rc.exe"))
        .find(|path| path.is_file())
        .expect("Windows SDK resource compiler");
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo output directory"));
    let icon = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest directory"))
        .join("../../assets/icons/mo-app-icon.ico")
        .canonicalize()
        .expect("Mo icon");
    let rc = output.join("mo-settings.rc");
    let resource = output.join("mo-settings.res");
    let manifest =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").expect("Cargo manifest directory"))
            .join("mo-settings.manifest");
    fs::write(
        &rc,
        format!(
            "1 ICON \"{}\"\n1 24 \"{}\"\n",
            icon.to_string_lossy().replace('\\', "/"),
            manifest.to_string_lossy().replace('\\', "/")
        ),
    )
    .expect("write icon resource");
    let result = Command::new(compiler)
        .arg("/nologo")
        .arg("/c65001")
        .arg(format!("/fo{}", resource.display()))
        .arg(&rc)
        .status()
        .expect("run resource compiler");
    assert!(result.success(), "Mo icon resource compilation failed");
    println!(
        "cargo:rustc-link-arg-bin=mo-settings={}",
        resource.display()
    );
}
