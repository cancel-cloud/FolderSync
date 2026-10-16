use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode { TwoWay, Mirror, Copy, Move }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action { Unchanged, CopyToB, CopyToA, DeleteA, DeleteB, MoveToB, Conflict }
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Scope {
    pub include: Vec<String>, pub exclude: Vec<String>, pub folder_include: Vec<String>, pub folder_exclude: Vec<String>,
    pub selected_folders: Vec<String>, pub exclude_root_files: bool, pub skip_hidden: bool, pub skip_system: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Task {
    pub id: String, pub name: String, pub a: PathBuf, pub b: PathBuf, pub mode: Mode, pub group: String,
    pub scope: Scope, pub monitor: bool, pub debounce_secs: u64, pub retries: u32,
    pub bandwidth_kib: u64, pub history_patterns: Vec<String>, pub history_cap: u32,
    pub schedule: Schedule, pub login: bool,
}
impl Default for Task {
    fn default() -> Self { Self { id:uuid::Uuid::new_v4().to_string(), name:"New task".into(), a:PathBuf::new(), b:PathBuf::new(), mode:Mode::TwoWay, group:String::new(), scope:Scope::default(), monitor:false, debounce_secs:20, retries:3, bandwidth_kib:0, history_patterns:vec![], history_cap:10, schedule:Schedule::default(), login:false } }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Schedule { pub kind: String, pub interval_minutes: u32, pub hour: u32, pub minute: u32, pub weekdays:Vec<u32>, pub month_day:u32 }
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry { pub path:String, pub digest:String, pub directory:bool, pub size:u64, pub mtime_ns:String }
pub type Tree = BTreeMap<String, Entry>;
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Metrics { pub scanned_entries:u64, pub hashed_bytes:u64, pub copied_bytes:u64, pub queued_paths:u64, pub reconciliations:u64, pub watch_events:u64 }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Operation { pub path:String, pub action:Action, pub reason:String, pub a:Option<Entry>, pub b:Option<Entry> }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan { pub id:String, pub task:Task, pub root_a:String, pub root_b:String, pub operations:Vec<Operation>, pub metrics:Metrics, pub baseline_generation:i64, pub created_at:u64, pub status:String }
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Version { pub id:String, pub task_id:String, pub side:String, pub path:String, pub digest:String, pub stored:PathBuf, pub created_at:u64 }
pub fn now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }

