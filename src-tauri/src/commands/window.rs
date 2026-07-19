use tauri::{AppHandle, Manager};

#[tauri::command]
pub fn reset_main_webview_zoom(app: AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };

    window
        .set_zoom(1.0)
        .map_err(|error| format!("Nu am putut reseta zoom-ul WebView: {error}"))?;

    #[cfg(target_os = "linux")]
    {
        window
            .with_webview(|webview| {
                use webkit2gtk::WebViewExt;
                webview.inner().set_zoom_level(1.0);
            })
            .map_err(|error| format!("Nu am putut reseta zoom-ul WebKitGTK: {error}"))?;
    }

    Ok(())
}
