use foldersync_engine::*;
use foldersync_engine::fs::{validate_roots,scan};
#[test]
fn roots_reject_overlap_and_preserve_preview_files() {
    let d=tempfile::tempdir().unwrap(); let a=d.path().join("A");let b=d.path().join("B");
    std::fs::create_dir_all(a.join("nested")).unwrap();std::fs::create_dir(&b).unwrap();
    let mut t=Task {a:a.clone(),b:b.clone(),..Task::default()};
    assert!(validate_roots(&t,&d.path().join("state")).is_ok());
    t.b=a.join("nested");assert!(validate_roots(&t,&d.path().join("state")).is_err());
    t.b=b;t.a=d.path().join("absent");assert!(validate_roots(&t,&d.path().join("state")).is_err());
}
#[test]
fn scan_filters_and_hashes_even_metadata_collisions() {
    let d=tempfile::tempdir().unwrap();std::fs::create_dir(d.path().join("empty")).unwrap();
    std::fs::write(d.path().join("ü.txt"),"abcd").unwrap();std::fs::write(d.path().join("skip.tmp"),"secret").unwrap();
    let mut m=Metrics::default();let s=Scope {exclude:vec!["*.tmp".into()],..Scope::default()};
    let tree=scan(d.path(),&s,&mut m).unwrap();assert_eq!(tree.len(),2);assert_eq!(m.hashed_bytes,4);
    let before=&tree["ü.txt"].digest;std::fs::write(d.path().join("ü.txt"),"dcba").unwrap();
    assert_ne!(before,&scan(d.path(),&s,&mut m).unwrap()["ü.txt"].digest);
}
