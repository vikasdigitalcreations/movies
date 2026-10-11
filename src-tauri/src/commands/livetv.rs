//! LiveTV: the channel guide. The work is in `core::livetv`.

use crate::core::livetv::{self, LiveGuide};
use crate::core::types::CmdResult;

/// Every channel, merged from the public lists. `refresh` downloads the lists again
/// instead of answering from the copy held since they were last fetched.
#[tauri::command]
pub async fn live_channels(refresh: Option<bool>) -> CmdResult<LiveGuide> {
    livetv::guide(refresh.unwrap_or(false)).await
}
