//! Desktop app of Potato Pokémon Randomizer.
//!
//! A thin shell (ADR 0004): commands translate arguments, call `ppr-service` and map errors to `ApiError`.

// Hide the console window on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod window;

fn main() {
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
