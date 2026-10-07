//! Embed a Common Controls 6 manifest. `TaskDialogIndirect` (pulled in by the
//! tray stack) is only exported by comctl32 v6; without this resource the
//! loader binds v5 and the process dies with STATUS_ENTRYPOINT_NOT_FOUND
//! before any window is created. rustc writes a sidecar manifest but does not
//! embed it with the current linker.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
  <dependency>
    <dependentAssembly>
      <assemblyIdentity type="win32" name="Microsoft.Windows.Common-Controls" version="6.0.0.0" processorArchitecture="*" publicKeyToken="6595b64144ccf1df" language="*" />
    </dependentAssembly>
  </dependency>
</assembly>
"#;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let manifest_path = out_dir.join("nanatype.manifest");
    let rc_path = out_dir.join("nanatype.rc");
    let res_path = out_dir.join("nanatype.res");
    fs::write(&manifest_path, MANIFEST).expect("write manifest");
    let manifest_rc = manifest_path.display().to_string().replace('\\', "/");
    let rc = format!("1 24 \"{manifest_rc}\"\r\n");
    fs::write(&rc_path, rc).expect("write rc");
    let rc_exe = find_rc().unwrap_or_else(|| {
        panic!("rc.exe was not found; install the Windows SDK so the Common Controls manifest can be embedded");
    });
    let status = Command::new(&rc_exe)
        .arg("/nologo")
        .arg("/fo")
        .arg(&res_path)
        .arg(&rc_path)
        .status()
        .unwrap_or_else(|error| panic!("failed to run {}: {error}", rc_exe.display()));
    if !status.success() {
        panic!("rc.exe failed with {status}");
    }
    println!("cargo:rustc-link-arg-bin=nanatype={}", res_path.display());
}

fn find_rc() -> Option<PathBuf> {
    if let Ok(path) = env::var("RC_PATH") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Some(path);
        }
    }
    let kits = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    let mut versions = fs::read_dir(kits)
        .ok()?
        .filter_map(|entry| entry.ok())
        .collect::<Vec<_>>();
    versions.sort_by_key(|entry| entry.file_name());
    versions.reverse();
    for version in versions {
        let rc = version.path().join("x64").join("rc.exe");
        if rc.is_file() {
            return Some(rc);
        }
    }
    None
}
