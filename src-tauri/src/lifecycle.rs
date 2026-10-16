use anyhow::{Result, ensure};
use foldersync_engine::Engine;
use std::sync::atomic::{AtomicBool, Ordering};

pub fn should_open_window(args: &[String]) -> bool {
    !args.iter().any(|arg| arg == "--background")
}

/// Read without taking the engine lock on the window event thread.
pub struct CloseBehavior {
    quit: AtomicBool,
}
impl CloseBehavior {
    pub fn load(engine: &Engine) -> Result<Self> {
        Ok(Self {
            quit: AtomicBool::new(engine.setting("close_behavior")?.as_deref() == Some("quit")),
        })
    }
    pub fn quit_on_close(&self) -> bool {
        self.quit.load(Ordering::Acquire)
    }
    pub fn save(&self, engine: &Engine, value: &str) -> Result<()> {
        ensure!(
            ["tray", "quit"].contains(&value),
            "Unsupported close behavior"
        );
        engine.set_setting("close_behavior", value)?;
        self.quit.store(value == "quit", Ordering::Release);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use foldersync_engine::Engine;
    #[test]
    fn sign_in_launch_stays_in_tray_while_manual_relaunch_opens_the_window() {
        assert!(!super::should_open_window(&[
            "FolderSync.exe".into(),
            "--background".into()
        ]));
        assert!(super::should_open_window(&["FolderSync.exe".into()]));
        assert!(super::should_open_window(&[
            "C:/--background/FolderSync.exe".into(),
            "--background-extra".into()
        ]));
    }
    #[test]
    fn close_choice_defaults_to_tray_and_persists_without_changing_automation() {
        let d = tempfile::Builder::new()
            .prefix("FolderSync-close-preference-")
            .tempdir()
            .unwrap();
        let engine = Engine::open(d.path()).unwrap();
        let behavior = CloseBehavior::load(&engine).unwrap();
        assert!(!behavior.quit_on_close());
        behavior.save(&engine, "quit").unwrap();
        assert!(behavior.quit_on_close());
        assert!(engine.tasks().unwrap().is_empty());
        drop(engine);
        let engine = Engine::open(d.path()).unwrap();
        let behavior = CloseBehavior::load(&engine).unwrap();
        assert!(behavior.quit_on_close());
        assert!(behavior.save(&engine, "unsupported").is_err());
        assert_eq!(
            engine.setting("close_behavior").unwrap().as_deref(),
            Some("quit")
        );
        behavior.save(&engine, "tray").unwrap();
        assert!(!behavior.quit_on_close());
    }
    #[test]
    fn failed_persistence_keeps_native_close_action_and_saved_preference() {
        let d = tempfile::Builder::new()
            .prefix("FolderSync-close-write-failure-")
            .tempdir()
            .unwrap();
        let engine = Engine::open(d.path()).unwrap();
        let behavior = CloseBehavior::load(&engine).unwrap();
        behavior.save(&engine, "quit").unwrap();
        let blocker = rusqlite::Connection::open(d.path().join("foldersync.sqlite3")).unwrap();
        blocker.execute_batch("BEGIN IMMEDIATE").unwrap();
        assert!(behavior.save(&engine, "tray").is_err());
        assert!(behavior.quit_on_close());
        blocker.execute_batch("ROLLBACK").unwrap();
        assert!(CloseBehavior::load(&engine).unwrap().quit_on_close());
    }
}
