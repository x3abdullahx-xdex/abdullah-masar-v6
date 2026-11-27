#[tauri::command]
pub async fn fetch_and_save_ui_update(
    app: tauri::AppHandle,
    owner: String,
    repo: String,
    branch: String,
) -> Result<usize, String> {
    use tauri::Manager;
    let url = format!(
        "https://raw.githubusercontent.com/{}/{}/{}/src/index.html",
        owner.trim(),
        repo.trim(),
        branch.trim()
    );
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header(reqwest::header::USER_AGENT, "abdullah-masar-v6/6.0")
        .send()
        .await
        .map_err(|e| format!("Network error: {}", e))?;
    if !resp.status().is_success() {
        return Err(format!("GitHub fetch failed: HTTP {}", resp.status()));
    }
    let html = resp.text().await.map_err(|e| e.to_string())?;
    if html.len() < 100_000 || !html.contains("<!DOCTYPE html>") || !html.contains("masar_os_v3_abdullah") {
        return Err("فشل فحص الأمان (Anti-Bricking): الملف غير مكتمل".into());
    }
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(dir.join("hot_ui.html"), &html).map_err(|e| e.to_string())?;
    Ok(html.len())
}

#[tauri::command]
pub fn load_hot_ui(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri::Manager;
    let target = app.path().app_data_dir().map_err(|e| e.to_string())?.join("hot_ui.html");
    if !target.exists() {
        return Ok(None);
    }
    Ok(Some(std::fs::read_to_string(&target).map_err(|e| e.to_string())?))
}

#[tauri::command]
pub fn clear_hot_ui(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    let target = app.path().app_data_dir().map_err(|e| e.to_string())?.join("hot_ui.html");
    if target.exists() {
        std::fs::remove_file(&target).map_err(|e| e.to_string())?;
    }
    Ok(())
}
