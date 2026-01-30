mod crypto;
mod launcher;

// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn hash_password(password: String) -> String {
    crypto::hash_password(&password)
}

#[tauri::command]
fn verify_password(password: String, hashed: String) -> bool {
    crypto::verify_password(&password, &hashed)
}

#[tauri::command]
async fn connect_ssh(address: String, username: Option<String>) -> Result<(), String> {
    launcher::launch_ssh(&address, username.as_deref())
}

#[tauri::command]
async fn connect_rdp(address: String) -> Result<(), String> {
    launcher::launch_rdp(&address)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_sql::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            greet,
            hash_password,
            verify_password,
            connect_ssh,
            connect_rdp
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greet() {
        let result = greet("World");
        assert_eq!(result, "Hello, World! You've been greeted from Rust!");
    }
}
