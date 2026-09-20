pub mod commands;
pub mod core;
mod state;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.unminimize();
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(
            tauri_plugin_log::Builder::new()
                .level(log::LevelFilter::Info)
                .max_file_size(2_000_000)
                .rotation_strategy(tauri_plugin_log::RotationStrategy::KeepSome(3))
                .target(tauri_plugin_log::Target::new(tauri_plugin_log::TargetKind::LogDir { file_name: Some("moviebox".into()) }))
                .build(),
        )
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_os::init())
        .plugin(tauri_plugin_libmpv::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let state = state::AppState::new(app.handle().clone());
            state.downloads.kick();
            app.manage(state);
            log::info!("MovieBox {} started", app.package_info().version);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::catalog::home,
            commands::catalog::search,
            commands::catalog::suggest,
            commands::catalog::details,
            commands::streams::streams,
            commands::streams::subtitles,
            commands::streams::fetch_subtitle,
            commands::streams::alternate_source,
            commands::addons::addons_list,
            commands::addons::addons_add,
            commands::addons::addons_remove,
            commands::addons::addons_toggle,
            commands::addons::addon_streams,
            commands::library::history_list,
            commands::library::history_get,
            commands::library::history_watched,
            commands::library::history_start,
            commands::library::history_progress,
            commands::library::history_mark_watched,
            commands::library::history_remove,
            commands::library::history_clear,
            commands::library::favorites_list,
            commands::library::favorites_toggle,
            commands::library::favorites_clear,
            commands::downloads::download_list,
            commands::downloads::download_add,
            commands::downloads::download_pause,
            commands::downloads::download_resume,
            commands::downloads::download_remove,
            commands::system::settings_get,
            commands::system::settings_set,
            commands::system::system_info,
            commands::system::free_space_for,
            commands::system::open_folder,
            commands::system::open_logs,
            commands::system::clear_cache,
            commands::system::keep_awake,
            commands::system::check_online,
        ])
        .run(tauri::generate_context!())
        .expect("error while running MovieBox");
}
