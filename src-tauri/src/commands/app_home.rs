use tauri::AppHandle;

use crate::app_home::{ensure_app_home, AppHomeSnapshot};

#[tauri::command]
pub fn read_app_home(app: AppHandle) -> Result<AppHomeSnapshot, String> {
    ensure_app_home(&app)
}
