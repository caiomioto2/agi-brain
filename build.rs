use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=assets/agi-brain.ico");
    println!("cargo:rerun-if-changed=assets/agi-brain.rc");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    assert_eq!(env::var("CARGO_CFG_TARGET_ENV").as_deref(), Ok("msvc"));
    let workspace = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let mut roots = vec![workspace.join(".tools/windows-sdk/c/bin")];
    if let Some(root) = env::var_os("WindowsSdkDir") {
        roots.push(PathBuf::from(root).join("bin"));
    }
    if let Some(root) = env::var_os("ProgramFiles(x86)") {
        roots.push(PathBuf::from(root).join("Windows Kits/10/bin"));
    }
    let mut candidates = Vec::new();
    for root in roots {
        if let Ok(entries) = fs::read_dir(root) {
            candidates.extend(entries.flatten().map(|e| e.path().join("x64/rc.exe")));
        }
    }
    candidates.sort();
    let compiler = candidates
        .into_iter()
        .rev()
        .find(|p| p.is_file())
        .unwrap_or_else(|| "rc.exe".into());
    let resource = PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("agi-brain.res");
    let status = Command::new(compiler)
        .current_dir(workspace)
        .arg("/nologo")
        .arg("/fo")
        .arg(&resource)
        .arg("assets/agi-brain.rc")
        .status()
        .expect("Install Windows SDK with the resource compiler (rc.exe)");
    assert!(
        status.success(),
        "Failed to compile the Windows icon resource"
    );
    for bin in ["agi-brain", "agi-brain-context"] {
        println!("cargo:rustc-link-arg-bin={bin}={}", resource.display());
    }
}
