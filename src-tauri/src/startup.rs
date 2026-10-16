//! Sign-in registration belongs exclusively to the current Windows user.
use std::path::Path;
use windows_registry::CURRENT_USER;

const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const NAME: &str = "FolderSync";
const NOT_FOUND: i32 = 0x80070002_u32 as i32;

fn command(executable: &Path) -> anyhow::Result<String> {
    let path = executable
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("Startup executable path is not valid Unicode"))?;
    anyhow::ensure!(
        executable.is_absolute() && !path.contains(['"', '\0']),
        "Invalid startup executable path"
    );
    Ok(format!("\"{path}\" --background"))
}

fn registered(name: &str) -> anyhow::Result<bool> {
    let key = match CURRENT_USER.open(RUN) {
        Ok(key) => key,
        Err(error) if error.code().0 == NOT_FOUND => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    match key.get_string(name) {
        Ok(value) => Ok(!value.is_empty()),
        Err(error) if error.code().0 == NOT_FOUND => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn update(name: &str, enabled: bool, executable: &Path) -> anyhow::Result<()> {
    if enabled {
        CURRENT_USER
            .create(RUN)?
            .set_string(name, command(executable)?)?;
    } else {
        // Opening an existing key avoids creating registry state when already disabled.
        let key = match CURRENT_USER.options().read().write().open(RUN) {
            Ok(key) => key,
            Err(error) if error.code().0 == NOT_FOUND => return Ok(()),
            Err(error) => return Err(error.into()),
        };
        match key.remove_value(name) {
            Ok(()) => (),
            Err(error) if error.code().0 == NOT_FOUND => (),
            Err(error) => return Err(error.into()),
        }
    }
    Ok(())
}

#[tauri::command]
pub fn startup_enabled() -> Result<bool, String> {
    registered(NAME).map_err(|error| format!("{error:#}"))
}

#[tauri::command]
pub fn set_startup(enabled: bool) -> Result<(), String> {
    std::env::current_exe()
        .map_err(anyhow::Error::from)
        .and_then(|executable| update(NAME, enabled, &executable))
        .map_err(|error| format!("{error:#}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_user_registration_quotes_executable_and_disables_idempotently() {
        let fixture = tempfile::tempdir().unwrap();
        let name = format!(
            "FolderSync-test-{}",
            fixture.path().file_name().unwrap().to_string_lossy()
        );
        let executable = fixture.path().join("Folder Sync.exe");
        assert!(!registered(&name).unwrap());
        update(&name, true, &executable).unwrap();
        let value = CURRENT_USER.open(RUN).unwrap().get_string(&name).unwrap();
        // Remove the disposable entry before asserting, including on an unexpected value.
        update(&name, false, &executable).unwrap();
        assert_eq!(value, format!("\"{}\" --background", executable.display()));
        assert!(!registered(&name).unwrap());
        update(&name, false, &executable).unwrap();
        assert!(command(Path::new("relative.exe")).is_err());
        assert!(command(Path::new(r#"C:\bad"name.exe"#)).is_err());
    }
}
