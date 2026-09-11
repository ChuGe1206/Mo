use std::env;

fn main() {
    println!("cargo:rerun-if-env-changed=MO_LIBRIME_LIB_DIR");
    println!("cargo:rerun-if-env-changed=MO_LIBRIME_LIB_NAME");

    let dynamic = env::var_os("CARGO_FEATURE_LINK_DYNAMIC").is_some();
    let static_link = env::var_os("CARGO_FEATURE_LINK_STATIC").is_some();

    assert!(
        !(dynamic && static_link),
        "features `link-dynamic` and `link-static` are mutually exclusive"
    );

    if !dynamic && !static_link {
        return;
    }

    if let Some(directory) = env::var_os("MO_LIBRIME_LIB_DIR") {
        println!(
            "cargo:rustc-link-search=native={}",
            directory.to_string_lossy()
        );
    }

    let library = env::var("MO_LIBRIME_LIB_NAME").unwrap_or_else(|_| "rime".to_owned());
    let kind = if static_link { "static" } else { "dylib" };
    println!("cargo:rustc-link-lib={kind}={library}");
}
