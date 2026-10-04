use std::path::Path;

/// DLLs the Windows exe imports that a fresh machine may lack (SPEC Step 0):
/// the Visual C++ runtime llama.cpp's C++ and OpenMP code link.
/// `tauri.windows.conf.json` bundles them next to the exe.
const WINDOWS_DLLS: [&str; 4] = [
    "msvcp140.dll",
    "vcruntime140.dll",
    "vcruntime140_1.dll",
    "vcomp140.dll",
];

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        stage_windows_dlls();
    }
    tauri_build::build()
}

/// Copies the build machine's DLLs out of System32 into `redist/`. The
/// bundlers cannot take them from System32 directly: WiX and NSIS are 32-bit,
/// and Windows sends their reads there to SysWOW64 and its 32-bit copies.
fn stage_windows_dlls() {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string());
    let system = Path::new(&root).join("System32");
    let redist = Path::new(env!("CARGO_MANIFEST_DIR")).join("redist");
    std::fs::create_dir_all(&redist).expect("could not create redist/");
    for name in WINDOWS_DLLS {
        let from = system.join(name);
        let to = redist.join(name);
        println!("cargo:rerun-if-changed={}", from.display());
        let bytes = std::fs::read(&from).unwrap_or_else(|e| {
            panic!(
                "{} is missing ({e}): install the Visual C++ redistributable",
                from.display()
            )
        });
        // Rewriting an unchanged copy would rerun this script on every build.
        if std::fs::read(&to).ok().as_ref() != Some(&bytes) {
            std::fs::write(&to, bytes).expect("could not write to redist/");
        }
    }
}
