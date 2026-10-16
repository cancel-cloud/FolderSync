#![cfg(windows)]
use foldersync_engine::*;
use std::{fs, path::PathBuf, process::Command};

fn fixture() -> (tempfile::TempDir, Engine, Task) {
    let d = tempfile::Builder::new()
        .prefix("FolderSync-Windows-safety-")
        .tempdir()
        .unwrap();
    let t = Task {
        a: d.path().join("A"),
        b: d.path().join("B"),
        ..Task::default()
    };
    fs::create_dir(&t.a).unwrap();
    fs::create_dir(&t.b).unwrap();
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    e.save_task(&t).unwrap();
    (d, e, t)
}

struct DeniedDirectory {
    path: PathBuf,
    account: String,
}
impl Drop for DeniedDirectory {
    fn drop(&mut self) {
        // Remove only the temporary deny ACE we added; inherited ACLs stay intact.
        let result = Command::new("icacls.exe")
            .arg(&self.path)
            .args(["/remove:d", &self.account])
            .output()
            .expect("restore fixture ACL");
        assert!(
            result.status.success(),
            "Fixture ACL cleanup failed: {}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
}

#[test]
fn access_denied_subdirectory_never_becomes_deletion_evidence() {
    let (_d, mut e, t) = fixture();
    let nested = t.a.join("private");
    fs::create_dir(&nested).unwrap();
    fs::write(nested.join("keep.txt"), "protected baseline bytes").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, false).unwrap();
    let account = Command::new("whoami.exe").output().unwrap();
    assert!(account.status.success());
    let account = String::from_utf8(account.stdout).unwrap().trim().to_owned();
    let denied = DeniedDirectory {
        path: nested.clone(),
        account,
    };
    let result = Command::new("icacls.exe")
        .arg(&nested)
        .arg("/deny")
        .arg(format!("{}:(RD)", denied.account))
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    assert_eq!(
        fs::read_dir(&nested).unwrap_err().kind(),
        std::io::ErrorKind::PermissionDenied
    );
    assert!(e.preview(&t.id).is_err());
    e.queue_paths(&t.id, &[], true).unwrap();
    assert!(e.process_queue(&t.id).is_err());
    assert_eq!(
        fs::read(t.b.join("private/keep.txt")).unwrap(),
        b"protected baseline bytes"
    );
    assert!(!e.queued(&t.id).unwrap().is_empty());
    drop(denied);
    let p = e.preview(&t.id).unwrap();
    assert!(p.operations.iter().all(|o| o.action == Action::Unchanged));
}

#[test]
fn junction_root_and_nested_loop_are_rejected_without_following_target() {
    let (d, mut e, t) = fixture();
    fs::write(t.a.join("keep.txt"), "original").unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, false).unwrap();
    let link = t.a.join("loop");
    let result = Command::new("cmd.exe")
        .args(["/D", "/C", "mklink", "/J"])
        .arg(&link)
        .arg(&t.a)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(e.preview(&t.id).is_err());
    let bad = Task {
        a: link.clone(),
        b: t.b.clone(),
        ..Task::default()
    };
    assert!(foldersync_engine::fs::validate_roots(&bad, &d.path().join("other-state")).is_err());
    fs::remove_dir(link).unwrap();
    assert_eq!(fs::read(t.a.join("keep.txt")).unwrap(), b"original");
    assert_eq!(fs::read(t.b.join("keep.txt")).unwrap(), b"original");
}

#[test]
fn verified_transfer_preserves_file_modification_time() {
    let (_d, mut e, t) = fixture();
    fs::write(t.a.join("time.txt"), "timestamp fixture").unwrap();
    let expected = filetime::FileTime::from_unix_time(1_700_000_000, 123_456_700);
    filetime::set_file_mtime(t.a.join("time.txt"), expected).unwrap();
    let p = e.preview(&t.id).unwrap();
    e.apply(&p.id, false).unwrap();
    let actual = filetime::FileTime::from_last_modification_time(
        &fs::metadata(t.b.join("time.txt")).unwrap(),
    );
    assert_eq!(actual, expected);
    assert_eq!(
        fs::read(t.b.join("time.txt")).unwrap(),
        b"timestamp fixture"
    );
}
