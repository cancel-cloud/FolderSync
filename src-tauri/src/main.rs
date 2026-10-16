#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use foldersync_engine::{Engine, Plan, RestorePreview, Task, TaskStatus, Version};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use tauri::{Emitter, Manager, State};
mod background;
#[derive(Clone)]
struct Runtime {
    engine: Arc<Mutex<Engine>>,
    cancel: Arc<AtomicBool>,
    paused: Arc<AtomicBool>,
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
        f(&mut e).map_err(|e| format!("{e:#}"))
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
async fn preview(r: State<'_, Runtime>, id: String) -> Result<Plan, String> {
    work(r.inner().clone(), move |e| e.preview(&id)).await
}
#[tauri::command]
async fn apply(r: State<'_, Runtime>, id: String, destructive: bool) -> Result<Plan, String> {
    work(r.inner().clone(), move |e| e.apply(&id, destructive)).await
}
#[tauri::command]
async fn resolve(
    r: State<'_, Runtime>,
    id: String,
    path: String,
    choice: String,
) -> Result<Plan, String> {
    work(r.inner().clone(), move |e| e.resolve(&id, &path, &choice)).await
}
#[tauri::command]
async fn history(r: State<'_, Runtime>, id: String) -> Result<Vec<Plan>, String> {
    work(r.inner().clone(), move |e| e.history(&id)).await
}
#[tauri::command]
async fn versions(r: State<'_, Runtime>, id: String) -> Result<Vec<Version>, String> {
    work(r.inner().clone(), move |e| e.versions(&id)).await
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
async fn set_setting(r: State<'_, Runtime>, key: String, value: String) -> Result<(), String> {
    work(r.inner().clone(), move |e| e.set_setting(&key, &value)).await
}
#[tauri::command]
async fn storage(r: State<'_, Runtime>) -> Result<String, String> {
    work(r.inner().clone(), |e| Ok(e.storage().display().to_string())).await
}
#[tauri::command]
fn stop(r: State<'_, Runtime>) {
    r.cancel.store(true, Ordering::Relaxed);
}
#[tauri::command]
fn pause(r: State<'_, Runtime>, value: bool) {
    r.paused.store(value, Ordering::Relaxed);
}
fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .setup(|app| {
            let engine = Engine::open(&app.path().app_data_dir()?)?;
            let cancel = engine.cancel.clone();
            app.manage(Runtime {
                engine: Arc::new(Mutex::new(engine)),
                cancel,
                paused: Arc::new(AtomicBool::new(false)),
            });
            background::start(app.handle().clone(), app.state::<Runtime>().inner().clone());
            let menu = tauri::menu::Menu::with_items(
                app,
                &[
                    &tauri::menu::MenuItem::with_id(
                        app,
                        "open",
                        "Open FolderSync",
                        true,
                        None::<&str>,
                    )?,
                    &tauri::menu::MenuItem::with_id(
                        app,
                        "pause",
                        "Pause / Resume automation",
                        true,
                        None::<&str>,
                    )?,
                    &tauri::menu::MenuItem::with_id(
                        app,
                        "stop",
                        "Stop active work",
                        true,
                        None::<&str>,
                    )?,
                    &tauri::menu::MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?,
                ],
            )?;
            let mut tray = tauri::tray::TrayIconBuilder::new()
                .tooltip("FolderSync — folder synchronization")
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
                        r.cancel.store(true, Ordering::Relaxed);
                        app.exit(0)
                    }
                    _ => {}
                }
            })
            .build(app)?;
            Ok(())
        })
        .on_window_event(|w, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = w.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            tasks,
            statuses,
            save_task,
            preview,
            apply,
            resolve,
            history,
            versions,
            restore_preview,
            restore_apply,
            delete_task,
            export_tasks,
            import_tasks,
            setting,
            set_setting,
            storage,
            stop,
            pause
        ])
        .run(tauri::generate_context!())
        .expect("FolderSync failed to start");
}
