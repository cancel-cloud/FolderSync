use foldersync_engine::*;
use std::fs;
#[test]
fn directory_events_keep_spelling_and_respect_filters() {
    let d = tempfile::tempdir().unwrap();
    let a = d.path().join("A");
    let b = d.path().join("B");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    let mut e = Engine::open(&d.path().join("state")).unwrap();
    let mut t = Task {
        a,
        b,
        ..Task::default()
    };
    t.scope.exclude = vec!["*.tmp".into()];
    e.save_task(&t).unwrap();
    fs::create_dir(t.a.join("UPPER")).unwrap();
    fs::write(t.a.join("UPPER/Name.txt"), "content").unwrap();
    fs::write(t.a.join("UPPER/skip.tmp"), "skip").unwrap();
    let p = e.preview_paths(&t.id, &["UPPER".into()]).unwrap();
    assert!(p.operations.iter().any(|o| o.path == "UPPER/Name.txt"));
    assert!(!p.operations.iter().any(|o| o.path.ends_with(".tmp")));
    e.apply(&p.id, false).unwrap();
    let name = fs::read_dir(t.b.join("UPPER"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .file_name();
    assert_eq!(name, "Name.txt");
}
#[cfg(windows)]
#[test]
fn hidden_directory_excludes_its_entire_subtree() {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::SetFileAttributesW;
    let d = tempfile::tempdir().unwrap();
    let dir = d.path().join("hidden");
    fs::create_dir(&dir).unwrap();
    fs::write(dir.join("f"), "private").unwrap();
    let path: Vec<_> = dir.as_os_str().encode_wide().chain(Some(0)).collect();
    assert_ne!(unsafe { SetFileAttributesW(path.as_ptr(), 2) }, 0);
    let scope = Scope {
        skip_hidden: true,
        ..Scope::default()
    };
    let result = foldersync_engine::fs::scan(d.path(), &scope, &mut Metrics::default()).unwrap();
    assert!(
        result.is_empty(),
        "Hidden descendants must not leak into the plan"
    );
}

#[cfg(windows)]
#[test]
fn system_attribute_is_shared_by_full_scan_and_incremental_planning() {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_SYSTEM, SetFileAttributesW,
    };
    let d = tempfile::Builder::new()
        .prefix("FolderSync-system-scope-")
        .tempdir()
        .unwrap();
    let a = d.path().join("A");
    let b = d.path().join("B");
    fs::create_dir(&a).unwrap();
    fs::create_dir(&b).unwrap();
    fs::create_dir(a.join("system-folder")).unwrap();
    fs::write(a.join("system-folder/child.txt"), "protected child").unwrap();
    fs::write(a.join("system-file.txt"), "protected file").unwrap();
    let marked: Vec<Vec<u16>> = [a.join("system-folder"), a.join("system-file.txt")]
        .iter()
        .map(|p| p.as_os_str().encode_wide().chain(Some(0)).collect())
        .collect();
    for path in &marked {
        assert_ne!(
            unsafe { SetFileAttributesW(path.as_ptr(), FILE_ATTRIBUTE_SYSTEM) },
            0
        );
    }
    let mut engine = Engine::open(&d.path().join("state")).unwrap();
    let mut task = Task {
        a,
        b,
        scope: Scope {
            skip_system: true,
            ..Scope::default()
        },
        ..Task::default()
    };
    engine.save_task(&task).unwrap();
    let full = engine.preview(&task.id).unwrap();
    assert!(full.operations.is_empty());
    let incremental = engine
        .preview_paths(
            &task.id,
            &["system-folder".into(), "system-file.txt".into()],
        )
        .unwrap();
    assert!(incremental.operations.is_empty());
    engine.apply(&incremental.id, false).unwrap();
    assert_eq!(fs::read_dir(&task.b).unwrap().count(), 0);
    task.scope.skip_system = false;
    engine.save_task(&task).unwrap();
    assert_eq!(engine.preview(&task.id).unwrap().operations.len(), 3);
    for path in &marked {
        assert_ne!(
            unsafe { SetFileAttributesW(path.as_ptr(), FILE_ATTRIBUTE_NORMAL) },
            0
        );
    }
}
