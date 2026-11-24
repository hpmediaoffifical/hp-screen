/*! © 2026 HP Media (hpvn.media) — HP Screen. Bảo lưu mọi quyền. HP không có đại lý khác, không hợp tác với đơn vị nào, mọi sao chép đều là vi phạm. */
/// vcpkg triplet Tesseract is built for.
const TRIPLET: &str = "x64-windows-static";

fn main() {
    // Tesseract, linked statically so the program needs no extra files (`src/tess.rs`).
    // vcpkg builds it once per machine:
    //   vcpkg install "tesseract[core]:x64-windows-static"
    // and is found through VCPKG_ROOT. The fully static triplet (C++ library included) is the
    // one that fits: tauri-build already links the Visual C++ runtime statically, and the
    // "-static-md" triplet would make the program need MSVCP140.dll, which many PCs lack.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && let Err(error) = vcpkg::Config::new().target_triplet(TRIPLET).find_package("tesseract")
    {
        panic!(
            "Tesseract was not found ({error}).\n\
             Install it once with vcpkg and point VCPKG_ROOT at vcpkg's folder:\n\
             \x20   vcpkg install \"tesseract[core]:x64-windows-static\"\n\
             See README.md, section \"Chạy từ mã nguồn\"."
        );
    }
    tauri_build::build()
}
