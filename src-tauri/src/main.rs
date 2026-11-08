//! Desktop app of Potato Pokémon Randomizer.
//!
//! A thin shell (ADR 0004): commands translate arguments, call `ppr-service` and map errors to `ApiError`.

// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod window;

fn main() {
    #[cfg(target_os = "linux")]
    if let Some(code) = restart_without_dmabuf_renderer() {
        std::process::exit(code);
    }
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .setup(|app| {
            window::create(app.handle())?;
            Ok(())
        })
        .manage(commands::CurrentRun::default())
        .manage(commands::LastLog::default())
        .manage(commands::OpenRom::default())
        .invoke_handler(tauri::generate_handler![
            commands::identify_rom,
            commands::open_rom,
            commands::editable_game,
            commands::validate_edits,
            commands::set_edits,
            commands::supported_games,
            commands::randomization_steps,
            commands::randomize,
            commands::randomize_batch,
            commands::cancel_randomize,
            commands::save_run_log,
            commands::settings_string,
            commands::read_settings_string,
            commands::save_settings_file,
            commands::load_settings_file,
            commands::save_premade_seed,
            commands::load_premade_seed,
            commands::save_edits_file,
            commands::load_edits_file,
            commands::custom_names,
            commands::save_custom_names,
            commands::open_support_page
        ])
        .run(tauri::generate_context!());
    if let Err(e) = result {
        tracing::error!("the app stopped with an error: {e}");
        std::process::exit(1);
    }
}

/// The DMA-BUF renderer of `WebKitGTK` breaks with NVIDIA's proprietary driver under compositors that enforce
/// explicit sync: GNOME closes the connection ("Explicit Sync only supported on dmabuf buffers") and the window never
/// opens. With that driver loaded and `WEBKIT_DISABLE_DMABUF_RENDERER` unset, this runs the app again with the renderer off and
/// returns its exit code. Setting the variable for this process instead would need unsafe code.
#[cfg(target_os = "linux")]
fn restart_without_dmabuf_renderer() -> Option<i32> {
    const VAR: &str = "WEBKIT_DISABLE_DMABUF_RENDERER";
    if std::env::var_os(VAR).is_some() || !std::path::Path::new("/proc/driver/nvidia/version").exists() {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let status = std::process::Command::new(exe)
        .args(std::env::args_os().skip(1))
        .env(VAR, "1")
        .status()
        .ok()?;
    Some(status.code().unwrap_or(1))
}
