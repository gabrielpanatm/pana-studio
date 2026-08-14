use tauri::{AppHandle, Manager, State};

use crate::{
    application_storage::{
        cleanup_receipt, clear_log_storage, clear_preview_storage, delete_storage_sessions,
        read_application_storage, ApplicationStorageSnapshot, DeleteStorageSessionsRequest,
        StorageCleanupReceipt,
    },
    state::AppState,
};

#[tauri::command]
pub fn read_application_storage_inventory(
    app: AppHandle,
    state: State<AppState>,
) -> Result<ApplicationStorageSnapshot, String> {
    read_application_storage(&app, state.inner())
}

#[tauri::command]
pub async fn clear_application_cache_storage(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<StorageCleanupReceipt, String> {
    let before = read_application_storage(&app, state.inner())?;
    // Preview cleanup can fail only before its first effect; run it before the
    // asynchronous WebKit request so every partial WebKit outcome still gets
    // represented in the returned receipt.
    let preview_effect = clear_preview_storage(&app, state.inner())?;
    let mut failures = preview_effect.failures;
    if before.cache.webkit_cleanup_supported && before.cache.webkit.bytes > 0 {
        if let Err(error) = clear_webkit_cache(&app).await {
            failures.push(error);
        }
    }
    let after = read_application_storage(&app, state.inner())?;
    let webkit_removed = usize::from(before.cache.webkit.bytes > after.cache.webkit.bytes);
    Ok(cleanup_receipt(
        "cache",
        preview_effect.removed_items.saturating_add(webkit_removed),
        before.cache.total_bytes,
        after.cache.total_bytes,
        after
            .cache
            .protected_preview_bytes
            .max(preview_effect.protected_bytes),
        failures,
        after,
    ))
}

#[tauri::command]
pub fn clear_application_log_storage(
    app: AppHandle,
    state: State<AppState>,
) -> Result<StorageCleanupReceipt, String> {
    clear_log_storage(&app, state.inner())
}

#[tauri::command]
pub fn delete_application_session_storage(
    request: DeleteStorageSessionsRequest,
    app: AppHandle,
    state: State<AppState>,
) -> Result<StorageCleanupReceipt, String> {
    delete_storage_sessions(&app, state.inner(), request)
}

#[cfg(target_os = "linux")]
async fn clear_webkit_cache(app: &AppHandle) -> Result<(), String> {
    let window = app.get_webview_window("main").ok_or_else(|| {
        "WebView-ul principal nu este disponibil pentru curățarea cache-ului.".to_string()
    })?;
    let (sender, receiver) = tokio::sync::oneshot::channel::<Result<(), String>>();
    window
        .with_webview(move |webview| {
            use webkit2gtk::{WebViewExt, WebsiteDataManagerExtManual, WebsiteDataTypes};

            let Some(manager) = webview.inner().website_data_manager() else {
                let _ = sender.send(Err(
                    "WebKitGTK nu a furnizat WebsiteDataManager pentru cache.".to_string(),
                ));
                return;
            };
            manager.clear(
                WebsiteDataTypes::DISK_CACHE | WebsiteDataTypes::MEMORY_CACHE,
                webkit2gtk::glib::TimeSpan::from_seconds(0),
                None::<&webkit2gtk::gio::Cancellable>,
                move |result| {
                    let _ = sender.send(result.map_err(|error| {
                        format!("Cache-ul WebKit nu a putut fi curățat: {error}")
                    }));
                },
            );
        })
        .map_err(|error| {
            format!("Curățarea cache-ului WebKit nu a putut fi programată: {error}")
        })?;
    tokio::time::timeout(std::time::Duration::from_secs(30), receiver)
        .await
        .map_err(|_| "WebKit nu a confirmat curățarea cache-ului în 30 de secunde.".to_string())?
        .map_err(|_| "WebKit nu a confirmat curățarea cache-ului.".to_string())?
}

#[cfg(not(target_os = "linux"))]
async fn clear_webkit_cache(_app: &AppHandle) -> Result<(), String> {
    Ok(())
}
