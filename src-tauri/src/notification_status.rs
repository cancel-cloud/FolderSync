//! Read-only delivery diagnostics. Windows owns banner, sound and DND policy.
use crate::Runtime;
use serde::Serialize;
use tauri::State;
use tauri_plugin_notification::NotificationExt;

#[derive(Serialize)]
pub struct NotificationStatus {
    app_enabled: bool,
    app_id: String,
    windows_setting: String,
    windows_error: Option<String>,
    // ToastNotifier.Setting does not report these independent OS preferences.
    banners: Option<bool>,
    presentation: String,
    sound: Option<bool>,
    do_not_disturb: Option<bool>,
}

#[tauri::command]
pub fn notification_status(
    app: tauri::AppHandle,
    r: State<'_, Runtime>,
) -> Result<NotificationStatus, String> {
    let engine = r.engine.lock().map_err(|_| "Engine lock failed")?;
    let app_enabled = engine
        .setting("notifications")
        .map_err(|e| e.to_string())?
        .as_deref()
        != Some("off");
    drop(engine);
    let app_id = notification_app_id(&app)?;
    let (windows_setting, windows_error) = system_setting(&app_id);
    let banners = configured_banners(&app_id);
    Ok(NotificationStatus {
        app_enabled,
        app_id,
        windows_setting,
        windows_error,
        banners,
        presentation: presentation_state().into(),
        sound: None,
        do_not_disturb: None,
    })
}

fn configured_banners(app_id: &str) -> Option<bool> {
    windows_registry::CURRENT_USER
        .open(format!(
            r"Software\Microsoft\Windows\CurrentVersion\Notifications\Settings\{app_id}"
        ))
        .ok()?
        .get_u32("ShowBanner")
        .ok()
        .map(|value| value != 0)
}

fn presentation_state() -> &'static str {
    use windows_sys::Win32::UI::Shell::*;
    let mut state = 0;
    // Read only: never changes Windows notification or focus preferences.
    if unsafe { SHQueryUserNotificationState(&mut state) } < 0 {
        return "unknown";
    }
    match state {
        QUNS_ACCEPTS_NOTIFICATIONS => "available",
        QUNS_NOT_PRESENT => "away",
        QUNS_BUSY => "busy",
        QUNS_RUNNING_D3D_FULL_SCREEN => "fullscreen",
        QUNS_PRESENTATION_MODE => "presentation",
        QUNS_QUIET_TIME => "quiet_time",
        QUNS_APP => "app",
        _ => "unknown",
    }
}

#[tauri::command]
pub fn open_notification_settings() -> Result<(), String> {
    std::process::Command::new("explorer.exe")
        .arg("ms-settings:notifications")
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

// Match the current plugin's Windows app-ID choice, including development builds.
fn notification_app_id(app: &tauri::AppHandle) -> Result<String, String> {
    #[cfg(windows)]
    {
        let exe = tauri::utils::platform::current_exe().map_err(|e| e.to_string())?;
        if exe.parent().is_some_and(|p| {
            let p = p.to_string_lossy();
            p.ends_with(r"\target\debug") || p.ends_with(r"\target\release")
        }) {
            return Ok(
                r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe"
                    .into(),
            );
        }
    }
    Ok(app.config().identifier.clone())
}

#[cfg(windows)]
fn setting_name(value: windows::UI::Notifications::NotificationSetting) -> &'static str {
    use windows::UI::Notifications::NotificationSetting as Setting;
    match value {
        Setting::Enabled => "enabled",
        Setting::DisabledForApplication => "disabled_for_application",
        Setting::DisabledForUser => "disabled_for_user",
        Setting::DisabledByGroupPolicy => "disabled_by_group_policy",
        Setting::DisabledByManifest => "disabled_by_manifest",
        _ => "unknown",
    }
}

#[cfg(windows)]
fn system_setting(app_id: &str) -> (String, Option<String>) {
    use windows::UI::Notifications::ToastNotificationManager;
    match ToastNotificationManager::CreateToastNotifierWithId(&app_id.into())
        .and_then(|notifier| notifier.Setting())
    {
        Ok(value) => (setting_name(value).into(), None),
        Err(error) => ("unknown".into(), Some(error.to_string())),
    }
}

#[cfg(not(windows))]
fn system_setting(_app_id: &str) -> (String, Option<String>) {
    ("not_windows".into(), None)
}

#[tauri::command]
pub fn send_test_notification(app: tauri::AppHandle, r: State<'_, Runtime>) -> Result<(), String> {
    let engine = r.engine.lock().map_err(|_| "Engine lock failed")?;
    if engine
        .setting("notifications")
        .map_err(|e| e.to_string())?
        .as_deref()
        == Some("off")
    {
        return Err("[FolderSync:NotificationsDisabled]".into());
    }
    let de = engine
        .setting("language")
        .map_err(|e| e.to_string())?
        .as_deref()
        == Some("de");
    drop(engine);
    let (title, body) = crate::presentation::test_notice(de);
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .sound("Default")
        .show()
        .map_err(|e| e.to_string())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    #[test]
    fn system_disabled_reasons_remain_distinct_and_future_values_unknown() {
        use windows::UI::Notifications::NotificationSetting as Setting;
        assert_eq!(setting_name(Setting::Enabled), "enabled");
        assert_eq!(
            setting_name(Setting::DisabledForApplication),
            "disabled_for_application"
        );
        assert_eq!(setting_name(Setting::DisabledForUser), "disabled_for_user");
        assert_eq!(
            setting_name(Setting::DisabledByGroupPolicy),
            "disabled_by_group_policy"
        );
        assert_eq!(
            setting_name(Setting::DisabledByManifest),
            "disabled_by_manifest"
        );
        assert_eq!(setting_name(Setting(99)), "unknown");
    }
}
