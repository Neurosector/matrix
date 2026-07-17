//! Auf atomaren Systemen fehlt der unversionierte Dev-Symlink
//! `libpam.so` (kein pam-devel). Wir legen ihn selbst im OUT_DIR an
//! und zeigen dem Linker dorthin — funktioniert auf dem PC-Dev-Bau
//! UND im Fedora-Builder, ohne ein einziges Extra-Paket.
fn main() {
    let out = std::env::var("OUT_DIR").unwrap();
    for kandidat in [
        "/usr/lib64/libpam.so.0",
        "/usr/lib/x86_64-linux-gnu/libpam.so.0",
        "/lib64/libpam.so.0",
    ] {
        if std::path::Path::new(kandidat).exists() {
            let link = format!("{out}/libpam.so");
            let _ = std::fs::remove_file(&link);
            std::os::unix::fs::symlink(kandidat, &link).ok();
            println!("cargo:rustc-link-search=native={out}");
            break;
        }
    }
    println!("cargo:rustc-link-lib=dylib=pam");
}
