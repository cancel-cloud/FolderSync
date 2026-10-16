use anyhow::{Result, ensure};
use std::path::{Path, PathBuf};

pub fn directory(root: &Path, task_id: Option<&str>) -> Result<PathBuf> {
    let target = match task_id {
        Some(id) => {
            ensure!(
                uuid::Uuid::parse_str(id)?.to_string() == id,
                "Invalid task ID"
            );
            root.join("versions").join(id)
        }
        None => root.to_path_buf(),
    };
    for ancestor in target.ancestors() {
        match std::fs::symlink_metadata(ancestor) {
            Ok(_) => {
                crate::app_storage::reject_reparse_ancestors(ancestor)?;
                break;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    std::fs::create_dir_all(&target)?;
    crate::app_storage::reject_reparse_ancestors(&target)?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opens_only_owned_root_or_canonical_task_directory() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("FolderSync");
        assert_eq!(directory(&root, None).unwrap(), root);
        let id = uuid::Uuid::new_v4().to_string();
        assert_eq!(
            directory(&root, Some(&id)).unwrap(),
            root.join("versions").join(id)
        );
        assert!(directory(&root, Some("../../outside")).is_err());
        assert!(directory(&root, Some("C:\\outside")).is_err());
    }
}
