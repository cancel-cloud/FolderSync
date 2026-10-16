#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use foldersync_engine::{
    ApplyOutcome, Engine, OperationPage, OperationQuery, PlanHeader, ProgressState, RecoveryTask,
    RestorePreview, RunProgress, Task, TaskStatus, Version,
};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tauri::{Emitter, Manager, State};
use tauri_plugin_notification::NotificationExt;
mod app_storage;
mod background;
mod lifecycle;
mod notification_status;
mod presentation;
mod progress_runtime;
mod startup;
mod storage_location;
fn command_error(error: &anyhow::Error) -> String {
    if error.is::<foldersync_engine::Cancelled>() {
        "[FolderSync:Cancelled]".into()
    } else {
        format!("{error:#}")
    }
}
#[derive(Clone)]
struct Runtime {
    engine: Arc<Mutex<Engine>>,
    cancel: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
    progress: ProgressState,
    close_behavior: Arc<lifecycle::CloseBehavior>,
}
async fn work<T: Send + 'static>(
    r: Runtime,
    f: impl FnOnce(&mut Engine) -> anyhow::Result<T> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut e = r
            .engine
            .lock()
            .map_err(|_| "Engine lock failed".to_string())?;
        f(&mut e).map_err(|e| command_error(&e))
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
async fn tasks(r: State<'_, Runtime>) -> Result<Vec<Task>, String> {
    work(r.inner().clone(), |e| e.tasks()).await
}
#[tauri::command]
async fn statuses(r: State<'_, Runtime>) -> Result<Vec<TaskStatus>, String> {
    work(r.inner().clone(), |e| e.statuses()).await
}
#[tauri::command]
async fn save_task(r: State<'_, Runtime>, task: Task) -> Result<(), String> {
    work(r.inner().clone(), move |e| e.save_task(&task)).await
}
#[tauri::command]
async fn assign_group(
    r: State<'_, Runtime>,
    ids: Vec<String>,
    group: String,
) -> Result<(), String> {
    work(r.inner().clone(), move |e| e.assign_group(&ids, &group)).await
}
#[tauri::command]
async fn disable_tasks(r: State<'_, Runtime>, ids: Vec<String>) -> Result<(), String> {
    work(r.inner().clone(), move |e| e.disable_tasks(&ids)).await
}
#[tauri::command]
async fn delete_tasks(r: State<'_, Runtime>, ids: Vec<String>) -> Result<(), String> {
    work(r.inner().clone(), move |e| e.delete_tasks(&ids)).await
}
#[tauri::command]
async fn preview(r: State<'_, Runtime>, id: String) -> Result<PlanHeader, String> {
    work(r.inner().clone(), move |e| {
        Ok(PlanHeader::from(&e.preview(&id)?))
    })
    .await
}
#[tauri::command]
async fn apply(
    r: State<'_, Runtime>,
    id: String,
    destructive: bool,
) -> Result<ApplyOutcome, String> {
    work(r.inner().clone(), move |e| {
        e.apply_reviewed(&id, destructive)
    })
    .await
}
#[tauri::command]
fn progress(r: State<'_, Runtime>) -> Option<RunProgress> {
    r.progress.snapshot()
}
#[tauri::command]
async fn resolve(
    r: State<'_, Runtime>,
    id: String,
    path: String,
    choice: String,
) -> Result<PlanHeader, String> {
    work(r.inner().clone(), move |e| {
        Ok(PlanHeader::from(&e.resolve(&id, &path, &choice)?))
    })
    .await
}
#[tauri::command]
async fn history(r: State<'_, Runtime>, id: String) -> Result<Vec<PlanHeader>, String> {
    work(r.inner().clone(), move |e| e.history_headers(&id)).await
}
#[tauri::command]
async fn operation_page(
    r: State<'_, Runtime>,
    id: String,
    query: OperationQuery,
) -> Result<OperationPage, String> {
    work(r.inner().clone(), move |e| e.operation_page(&id, &query)).await
}
#[tauri::command]
async fn versions(r: State<'_, Runtime>, id: String) -> Result<Vec<Version>, String> {
    work(r.inner().clone(), move |e| e.versions(&id)).await
}
#[tauri::command]
async fn recovery_tasks(r: State<'_, Runtime>) -> Result<Vec<RecoveryTask>, String> {
    work(r.inner().clone(), |e| e.recovery_tasks()).await
}
#[tauri::command]
async fn restore_preview(r: State<'_, Runtime>, id: String) -> Result<RestorePreview, String> {
    work(r.inner().clone(), move |e| e.restore_preview(&id)).await
}
#[tauri::command]
async fn restore_apply(r: State<'_, Runtime>, id: String) -> Result<(), String> {
    work(r.inner().clone(), move |e| e.restore_apply(&id)).await
}
#[tauri::command]
async fn delete_task(r: State<'_, Runtime>, id: String) -> Result<(), String> {
    work(r.inner().clone(), move |e| e.delete_task(&id)).await
}
#[tauri::command]
async fn export_tasks(r: State<'_, Runtime>, path: String) -> Result<(), String> {
    work(r.inner().clone(), move |e| {
        std::fs::write(path, e.export()?)?;
        Ok(())
    })
    .await
}
#[tauri::command]
async fn import_tasks(r: State<'_, Runtime>, path: String) -> Result<usize, String> {
    work(r.inner().clone(), move |e| {
        e.import(&std::fs::read_to_string(path)?)
    })
    .await
}
#[tauri::command]
async fn setting(r: State<'_, Runtime>, key: String) -> Result<Option<String>, String> {
    work(r.inner().clone(), move |e| e.setting(&key)).await
}
#[tauri::command]
async fn set_setting(
    app: tauri::AppHandle,
    r: State<'_, Runtime>,
    key: String,
    value: String,
) -> Result<(), String> {
    let allowed = match key.as_str() {
        "language" => ["en", "de"].contains(&value.as_str()),
        "notifications" | "paused" => ["on", "off"].contains(&value.as_str()),
        "log_level" => ["off", "basic", "detailed"].contains(&value.as_str()),
        "close_behavior" => ["tray", "quit"].contains(&value.as_str()),
        "task_view" => ["tiles", "list"].contains(&value.as_str()),
        _ => false,
    };
    if !allowed {
        return Err("Unsupported preference or value".into());
    }
    let language = if key == "language" {
        Some(value == "de")
    } else {
        None
    };
    let behavior = r.close_behavior.clone();
    work(r.inner().clone(), move |e| {
        if key == "close_behavior" {
            behavior.save(e, &value)
        } else {
            e.set_setting(&key, &value)
        }
    })
    .await?;
    if let Some(de) = language
        && let Some(tray) = app.tray_by_id("main-tray")
    {
        let menu = tray_menu(&app, de).map_err(|e| e.to_string())?;
        tray.set_menu(Some(menu)).map_err(|e| e.to_string())?;
        tray.set_tooltip(Some(if de {
            "FolderSync — Ordnersynchronisation"
        } else {
            "FolderSync — folder synchronization"
        }))
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}
#[tauri::command]
async fn logs(r: State<'_, Runtime>) -> Result<String, String> {
    work(r.inner().clone(), |e| e.recent_log()).await
}
#[tauri::command]
async fn storage(r: State<'_, Runtime>) -> Result<String, String> {
    work(r.inner().clone(), |e| Ok(e.storage().display().to_string())).await
}
#[tauri::command]
async fn open_storage(r: State<'_, Runtime>, task_id: Option<String>) -> Result<(), String> {
    let path = work(r.inner().clone(), move |e| {
        if let Some(ref id) = task_id {
            anyhow::ensure!(
                e.tasks()?.iter().any(|t| &t.id == id)
                    || e.recovery_tasks()?.iter().any(|t| &t.task.id == id),
                "Unknown saved task"
            );
        }
        storage_location::directory(e.storage(), task_id.as_deref())
    })
    .await?;
    std::process::Command::new("explorer.exe")
        .arg(path)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}
#[tauri::command]
fn stop(r: State<'_, Runtime>) {
    r.cancel.store(true, Ordering::Relaxed);
}
#[tauri::command]
fn pause(r: State<'_, Runtime>, value: bool) {
    r.paused.store(value, Ordering::Relaxed);
}
fn request_quit(app: tauri::AppHandle, r: Runtime) {
    r.paused.store(true, Ordering::Relaxed);
    r.cancel.store(true, Ordering::Relaxed);
    tauri::async_runtime::spawn_blocking(move || {
        if let Ok(guard) = r.engine.lock() {
            drop(guard)
        }
        app.exit(0);
    });
}
#[tauri::command]
fn quit(app: tauri::AppHandle, r: State<'_, Runtime>) {
    request_quit(app, r.inner().clone());
}
#[tauri::command]
fn pattern_test(
    scope: foldersync_engine::Scope,
    path: String,
    directory: bool,
) -> Result<bool, String> {
    foldersync_engine::fs::Matcher::new(&scope)
        .map(|m| m.allows(&path, directory))
        .map_err(|e| e.to_string())
}
fn notification_task(action: &tauri_plugin_notification::ActionPerformed) -> Option<String> {
    if action.action_id() == "dismiss" {
        return None;
    }
    action
        .notification()?
        .extra()
        .get("task_id")?
        .as_str()
        .map(str::to_owned)
}
fn tray_menu(app: &tauri::AppHandle, de: bool) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let menu = tauri::menu::Menu::new(app)?;
    for (id, label) in presentation::menu_labels(de) {
        menu.append(&tauri::menu::MenuItem::with_id(
            app,
            id,
            label,
            true,
            None::<&str>,
        )?)?;
    }
    Ok(menu)
}
fn main() {
    // Invoked only by the uninstaller after its explicit delete-data choice.
    // Resolve the Windows known folder instead of trusting an environment path.
    let args: Vec<_> = std::env::args().collect();
    if args.len() == 2 && args[1] == "--remove-local-data" {
        let result = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Cannot resolve Windows application data"))
            .and_then(|parent| app_storage::cleanup_storage(&parent));
        std::process::exit(if result.is_ok() { 0 } else { 1 });
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _cwd| {
            if !lifecycle::should_open_window(&args) {
                return;
            }
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .setup(|app| {
            let storage = app_storage::resolve_storage(&app.path().app_data_dir()?)?;
            let engine = Engine::open(&storage)?;
            let de = engine.setting("language")?.as_deref() == Some("de");
            let cancel = engine.cancel.clone();
            let progress = engine.progress.clone();
            let close_behavior = Arc::new(lifecycle::CloseBehavior::load(&engine)?);
            app.manage(Runtime {
                engine: Arc::new(Mutex::new(engine)),
                cancel,
                paused: Arc::new(AtomicBool::new(false)),
                progress: progress.clone(),
                close_behavior,
            });
            let handle = app.handle().clone();
            app.notification().on_action(move |action| {
                if let Some(id) = notification_task(action) {
                    if let Some(w) = handle.get_webview_window("main") {
                        let _ = w.show();
                        let _ = w.unminimize();
                        let _ = w.set_focus();
                    }
                    let _ = handle.emit("open-task", id);
                }
            })?;
            background::start(app.handle().clone(), app.state::<Runtime>().inner().clone());
            progress_runtime::start(app.handle().clone(), progress);
            let menu = tray_menu(app.handle(), de)?;
            let mut tray = tauri::tray::TrayIconBuilder::with_id("main-tray")
                .tooltip(if de {
                    "FolderSync — Ordnersynchronisation"
                } else {
                    "FolderSync — folder synchronization"
                })
                .menu(&menu);
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.on_menu_event(|app, event| {
                let r = app.state::<Runtime>();
                match event.id.as_ref() {
                    "open" => {
                        if let Some(w) = app.get_webview_window("main") {
                            let _ = w.show();
                            let _ = w.set_focus();
                        }
                    }
                    "pause" => {
                        let p = !r.paused.load(Ordering::Relaxed);
                        r.paused.store(p, Ordering::Relaxed);
                        let _ = app.emit("automation-paused", p);
                    }
                    "stop" => r.cancel.store(true, Ordering::Relaxed),
                    "quit" => {
                        request_quit(app.clone(), r.inner().clone());
                    }
                    _ => {}
                }
            })
            .build(app)?;
            if lifecycle::should_open_window(&std::env::args().collect::<Vec<_>>())
                && let Some(w) = app.get_webview_window("main")
            {
                w.show()?;
            }
            Ok(())
        })
        .on_window_event(|w, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if w.label() != "main" {
                    return;
                }
                api.prevent_close();
                if let Some(r) = w.try_state::<Runtime>()
                    && r.close_behavior.quit_on_close()
                {
                    // Renderer checks for an unsaved task draft before invoking
                    // the same safe Quit command used by Settings and the tray.
                    let _ = w.emit("request-close", ());
                } else {
                    let _ = w.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            startup::startup_enabled,
            startup::set_startup,
            notification_status::notification_status,
            notification_status::send_test_notification,
            notification_status::open_notification_settings,
            open_storage,
            disable_tasks,
            delete_tasks,
            tasks,
            statuses,
            save_task,
            assign_group,
            preview,
            apply,
            progress,
            resolve,
            history,
            operation_page,
            versions,
            recovery_tasks,
            restore_preview,
            restore_apply,
            delete_task,
            export_tasks,
            import_tasks,
            setting,
            set_setting,
            storage,
            logs,
            stop,
            pause,
            quit,
            pattern_test
        ])
        .run(tauri::generate_context!())
        .expect("FolderSync failed to start");
}

#[cfg(test)]
mod notification_tests {
    use super::*;
    #[test]
    fn command_cancellation_has_a_reserved_marker_and_os_errors_keep_details() {
        let cancelled = anyhow::Error::new(foldersync_engine::Cancelled).context("validation");
        assert_eq!(command_error(&cancelled), "[FolderSync:Cancelled]");
        assert_eq!(command_error(&anyhow::anyhow!("Cancelled")), "Cancelled");
        assert_eq!(
            command_error(&anyhow::anyhow!("destination locked")),
            "destination locked"
        );
    }
    #[test]
    fn activation_routes_to_the_originating_task_without_an_apply_action() {
        let action=serde_json::from_value(serde_json::json!({"actionId":"tap","inputValue":null,"notification":{"id":1,"extra":{"task_id":"fixture-task"}}})).unwrap();
        assert_eq!(notification_task(&action).as_deref(), Some("fixture-task"));
        let dismiss=serde_json::from_value(serde_json::json!({"actionId":"dismiss","notification":{"id":1,"extra":{"task_id":"fixture-task"}}})).unwrap();
        assert!(notification_task(&dismiss).is_none());
    }
}
