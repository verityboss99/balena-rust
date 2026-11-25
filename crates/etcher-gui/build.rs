use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../../assets/icon.ico");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let icon_path = manifest_dir
        .join("../../assets/icon.ico")
        .canonicalize()
        .expect("assets/icon.ico must exist");
    let icon_rc_str = icon_path.to_string_lossy().replace('\\', "/");

    let ver = env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "2.1.9".to_string());
    let parts: Vec<u16> = ver
        .split('.')
        .take(3)
        .filter_map(|p| p.parse::<u16>().ok())
        .collect();
    let (maj, min, pat) = (
        parts.first().copied().unwrap_or(2),
        parts.get(1).copied().unwrap_or(1),
        parts.get(2).copied().unwrap_or(9),
    );

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let rc_path = out_dir.join("balena-etcher.rc");
    let rc_content = format!(
        r#"1 ICON "{icon_rc_str}"

1 VERSIONINFO
FILEVERSION {maj},{min},{pat},0
PRODUCTVERSION {maj},{min},{pat},0
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904b0"
        BEGIN
            VALUE "CompanyName", "Balena Ltd."
            VALUE "FileDescription", "balenaEtcher"
            VALUE "FileVersion", "{ver}"
            VALUE "InternalName", "balena-etcher"
            VALUE "LegalCopyright", "Copyright (c) Balena Ltd."
            VALUE "OriginalFilename", "balena-etcher.exe"
            VALUE "ProductName", "balenaEtcher"
            VALUE "ProductVersion", "{ver}"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x0409, 1200
    END
END
"#
    );
    fs::write(&rc_path, rc_content).expect("failed to write balena-etcher.rc");

    let target_env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();

    if target_env == "msvc" {
        let res_path = out_dir.join("balena-etcher.res");
        let rc_tool = find_tool(&["rc.exe", "rc", "llvm-rc"]).expect(
            "Windows resource compiler (rc.exe or llvm-rc) not found in PATH",
        );
        let status = Command::new(&rc_tool)
            .arg("/fo")
            .arg(&res_path)
            .arg(&rc_path)
            .status()
            .expect("failed to invoke Windows resource compiler");
        assert!(status.success(), "{rc_tool} failed with {status}");
        println!("cargo:rustc-link-arg-bin=balena-etcher={}", res_path.display());
    } else {
        let obj_path = out_dir.join("balena-etcher.res.o");
        let candidates: &[&str] = if target_arch == "x86" {
            &["i686-w64-mingw32-windres", "llvm-windres", "windres"]
        } else {
            &["x86_64-w64-mingw32-windres", "llvm-windres", "windres"]
        };
        let windres = find_tool(candidates).expect(
            "MinGW/LLVM windres not found in PATH for embedding Windows icon",
        );
        let target_flag = if target_arch == "x86" {
            "pe-i386"
        } else {
            "pe-x86-64"
        };
        let status = Command::new(&windres)
            .arg("-i")
            .arg(&rc_path)
            .arg("-o")
            .arg(&obj_path)
            .arg("-O")
            .arg("coff")
            .arg("-F")
            .arg(target_flag)
            .status()
            .expect("failed to invoke windres");
        assert!(status.success(), "{windres} failed with {status}");
        println!("cargo:rustc-link-arg-bin=balena-etcher={}", obj_path.display());
    }
}

fn find_tool(names: &[&str]) -> Option<String> {
    let path_var = env::var_os("PATH").unwrap_or_default();
    for name in names {
        for dir in env::split_paths(&path_var) {
            let cand = dir.join(name);
            if is_file(&cand) {
                return Some(name.to_string());
            }
            if cfg!(windows) {
                let with_exe = dir.join(format!("{name}.exe"));
                if is_file(&with_exe) {
                    return Some(name.to_string());
                }
            }
        }
    }
    None
}

fn is_file(p: &Path) -> bool {
    fs::metadata(p).map(|m| m.is_file()).unwrap_or(false)
}
