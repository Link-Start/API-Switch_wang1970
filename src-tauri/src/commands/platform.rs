use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformCapabilities {
    pub shell: &'static str,
    pub is_tauri: bool,
    pub is_android_shell: bool,
    pub can_open_android_settings: bool,
    pub can_use_connection_apps: bool,
}

#[tauri::command]
pub fn get_platform_capabilities() -> PlatformCapabilities {
    #[cfg(mobile)]
    {
        return PlatformCapabilities {
            shell: "android",
            is_tauri: true,
            is_android_shell: true,
            can_open_android_settings: false,
            can_use_connection_apps: false,
        };
    }

    #[cfg(not(mobile))]
    {
        PlatformCapabilities {
            shell: "desktop",
            is_tauri: true,
            is_android_shell: false,
            can_open_android_settings: false,
            can_use_connection_apps: true,
        }
    }
}
