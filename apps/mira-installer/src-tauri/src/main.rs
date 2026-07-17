// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

/// Runs the main step for the installer binary entrypoint.
fn main() {
    configure_linux_webkit_environment();
    mira_installer_lib::run()
}

/// Runs the configure linux webkit environment step for the installer binary entrypoint.
#[cfg(target_os = "linux")]
fn configure_linux_webkit_environment() {
    set_env_if_missing("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    set_env_if_missing("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
    set_env_if_missing("GDK_BACKEND", "x11");
    set_env_if_missing("LIBGL_ALWAYS_SOFTWARE", "1");
}

/// Runs the configure linux webkit environment step for the installer binary entrypoint.
#[cfg(not(target_os = "linux"))]
fn configure_linux_webkit_environment() {}

/// Runs the set env if missing step for the installer binary entrypoint.
#[cfg(target_os = "linux")]
fn set_env_if_missing(key: &str, value: &str) {
    if std::env::var_os(key).is_none() {
        unsafe {
            std::env::set_var(key, value);
        }
    }
}
