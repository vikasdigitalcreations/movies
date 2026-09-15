use crate::core::downloads::{NewTask, Task};
use crate::core::types::CmdResult;
use crate::state::AppState;
use tauri::State;

#[tauri::command]
pub async fn download_list(state: State<'_, AppState>) -> CmdResult<Vec<Task>> {
    Ok(state.downloads.list())
}

#[tauri::command]
pub async fn download_add(state: State<'_, AppState>, task: NewTask) -> CmdResult<Task> {
    state.downloads.add(task).await
}

#[tauri::command]
pub async fn download_pause(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    state.downloads.pause(&id);
    Ok(())
}

#[tauri::command]
pub async fn download_resume(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    state.downloads.resume(&id);
    Ok(())
}

#[tauri::command]
pub async fn download_remove(state: State<'_, AppState>, id: String, delete_file: bool) -> CmdResult<()> {
    state.downloads.remove(&id, delete_file);
    Ok(())
}
