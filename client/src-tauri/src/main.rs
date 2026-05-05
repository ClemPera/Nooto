// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "linux")]
    appimage_wayland_workaround();
    nooto_lib::run()
}

// Workaround for https://github.com/tauri-apps/tauri/pull/12491
// AppImage bundles an older libwayland-client that conflicts with the system's
// Wayland compositor, causing EGL_BAD_PARAMETER and a white screen.
// Re-exec with LD_PRELOAD pointing to the system library to fix this.
#[cfg(target_os = "linux")]
fn appimage_wayland_workaround() {
    use std::os::unix::process::CommandExt;

    if std::env::var("APPIMAGE").is_err() || std::env::var("_NOOTO_WAYLAND_FIX").is_ok() {
        return;
    }

    let candidates = [
        "/usr/lib/libwayland-client.so.0",
        "/usr/lib/x86_64-linux-gnu/libwayland-client.so.0",
        "/usr/lib64/libwayland-client.so.0",
    ];

    for path in candidates {
        if std::path::Path::new(path).exists() {
            let err = std::process::Command::new(std::env::current_exe().unwrap())
                .args(std::env::args_os().skip(1))
                .env("LD_PRELOAD", path)
                .env("_NOOTO_WAYLAND_FIX", "1")
                .exec();
            eprintln!("appimage wayland workaround re-exec failed: {err}");
            break;
        }
    }
}
