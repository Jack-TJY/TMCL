use std::path::Path;

fn main() {
    let root = Path::new("vendor/seedatlas");
    println!("cargo:rerun-if-changed=vendor/seedatlas");
    println!("cargo:rerun-if-changed=native");
    let mut build = cc::Build::new();
    let tool = build.get_compiler();
    if tool.is_like_msvc() {
        // The upstream C uses variable-length arrays and GNU C extensions.
        if !tool.is_like_clang() {
            build.compiler("clang-cl");
        }
        build
            .flag("/clang:-std=gnu11")
            .flag("/clang:-fwrapv")
            .flag("/clang:-ffp-contract=off");
        build.define("_USE_MATH_DEFINES", None);
    } else {
        build
            .flag("-std=gnu11")
            .flag("-fwrapv")
            .flag("-ffp-contract=off");
    }
    build
        .include(root)
        .file("native/bridge.c")
        .opt_level(2)
        .warnings(false);
    for name in [
        "biomenoise",
        "biomes",
        "finders",
        "generator",
        "layers",
        "noise",
        "quadbase",
        "eyesearch",
        "strongholds",
        "util",
    ] {
        build.file(root.join(format!("{name}.c")));
    }
    build.compile("tmcl_seed_engine");
}
