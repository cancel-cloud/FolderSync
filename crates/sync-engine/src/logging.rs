use crate::*;
use anyhow::Result;
use std::{fs, io::Write};
impl Engine {
    pub fn recent_log(&self) -> Result<String> {
        use std::io::{Read, Seek, SeekFrom};
        let path = self.storage().join("operations.ndjson");
        let mut f = match fs::File::open(path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(String::new()),
            Err(e) => return Err(e.into()),
        };
        let size = f.metadata()?.len();
        f.seek(SeekFrom::Start(size.saturating_sub(64 * 1024)))?;
        let mut bytes = Vec::new();
        f.take(64 * 1024).read_to_end(&mut bytes)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    pub(crate) fn write_log(&self, plan: &Plan) -> Result<()> {
        let level = self.setting("log_level")?.unwrap_or_else(|| "off".into());
        if level == "off" {
            return Ok(());
        }
        let path = self.storage().join("operations.ndjson");
        let record = if level == "detailed" {
            serde_json::to_string(plan)?
        } else {
            let status = if plan.status.starts_with("Failed") {
                "Failed"
            } else if plan.status.starts_with("Attention") {
                "Attention"
            } else if plan.status.starts_with("Applied") {
                "Applied"
            } else {
                "Review required"
            };
            serde_json::to_string(
                &serde_json::json!({"run":plan.id,"task":plan.task.id,"status":status,"time":plan.created_at,"entries":plan.operations.len(),"metrics":plan.metrics}),
            )?
        };
        if fs::metadata(&path).is_ok_and(|m| m.len() > 5 * 1024 * 1024) {
            let previous = self.storage().join("operations.previous.ndjson");
            if previous.exists() {
                fs::remove_file(&previous)?;
            }
            fs::rename(&path, previous)?;
        }
        let mut log = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        writeln!(log, "{record}")?;
        log.flush()?;
        Ok(())
    }
}
