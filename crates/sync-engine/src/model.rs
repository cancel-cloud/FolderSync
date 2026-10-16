use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mode {
    TwoWay,
    Mirror,
    Copy,
    Move,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Unchanged,
    CopyToB,
    CopyToA,
    DeleteA,
    DeleteB,
    MoveToB,
    Conflict,
    Ignore,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Scope {
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    pub folder_include: Vec<String>,
    pub folder_exclude: Vec<String>,
    pub selected_folders: Vec<String>,
    pub exclude_root_files: bool,
    pub skip_hidden: bool,
    pub skip_system: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct Task {
    pub id: String,
    /// User-editable identifier; empty uses the immutable internal UUID.
    pub identifier: String,
    pub name: String,
    pub a: PathBuf,
    pub b: PathBuf,
    pub mode: Mode,
    pub group: String,
    pub scope: Scope,
    pub monitor: bool,
    pub debounce_secs: u64,
    pub retries: u32,
    pub bandwidth_kib: u64,
    pub history_patterns: Vec<String>,
    pub history_cap: u32,
    pub schedule: Schedule,
    pub login: bool,
    pub deletion_policy: DeletionPolicy,
}
impl Default for Task {
    fn default() -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            identifier: String::new(),
            name: "New task".into(),
            a: PathBuf::new(),
            b: PathBuf::new(),
            mode: Mode::TwoWay,
            group: String::new(),
            scope: Scope::default(),
            monitor: false,
            debounce_secs: 20,
            retries: 3,
            bandwidth_kib: 0,
            history_patterns: vec![],
            history_cap: 10,
            schedule: Schedule::default(),
            login: false,
            deletion_policy: DeletionPolicy::default(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct DeletionPolicy {
    pub enabled: bool,
    pub max_entries: u32,
    pub max_percent: u32,
}
impl Default for DeletionPolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            max_entries: 20,
            max_percent: 10,
        }
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeletionReason {
    Approved,
    Disabled,
    BaselineApprovalRequired,
    UnprovenDeletion,
    EntryLimit,
    PercentLimit,
    MoveRequiresReview,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeletionReview {
    pub reason: DeletionReason,
    pub entries: u64,
    pub baseline_entries: u64,
    pub max_entries: u32,
    pub max_percent: u32,
    pub baseline_run: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Schedule {
    pub kind: String,
    pub interval_minutes: u32,
    pub hour: u32,
    pub minute: u32,
    pub weekdays: Vec<u32>,
    pub month_day: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize, Eq)]
pub struct Entry {
    pub path: String,
    pub digest: String,
    pub directory: bool,
    pub size: u64,
    pub mtime_ns: String,
    #[serde(default)]
    pub file_id: Option<String>,
}
impl PartialEq for Entry {
    fn eq(&self, other: &Self) -> bool {
        crate::fs::path_key(&self.path) == crate::fs::path_key(&other.path)
            && self.digest == other.digest
            && self.directory == other.directory
            && self.size == other.size
            && self.mtime_ns == other.mtime_ns
            && self.file_id == other.file_id
    }
}
pub type Tree = BTreeMap<String, Entry>;
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Metrics {
    pub scanned_entries: u64,
    pub hashed_bytes: u64,
    pub copied_bytes: u64,
    pub queued_paths: u64,
    pub reconciliations: u64,
    pub watch_events: u64,
}
#[derive(Clone, Debug, Deserialize)]
pub struct Operation {
    pub path: String,
    pub action: Action,
    pub reason: String,
    #[serde(default)]
    pub status: String,
    pub a: Option<Entry>,
    pub b: Option<Entry>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationKind {
    Unchanged,
    Create,
    Update,
    Mkdir,
    DeleteToTrash,
    Move,
    Conflict,
    Ignore,
}
impl Operation {
    pub fn kind(&self) -> OperationKind {
        match self.action {
            Action::CopyToA | Action::CopyToB => {
                let (source, target) = if self.action == Action::CopyToA {
                    (&self.b, &self.a)
                } else {
                    (&self.a, &self.b)
                };
                if source.as_ref().is_some_and(|e| e.directory) {
                    OperationKind::Mkdir
                } else if target.is_some() {
                    OperationKind::Update
                } else {
                    OperationKind::Create
                }
            }
            Action::DeleteA | Action::DeleteB => OperationKind::DeleteToTrash,
            Action::MoveToB => OperationKind::Move,
            Action::Conflict => OperationKind::Conflict,
            Action::Ignore => OperationKind::Ignore,
            Action::Unchanged => OperationKind::Unchanged,
        }
    }
}
impl Serialize for Operation {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut record = serializer.serialize_struct("Operation", 7)?;
        record.serialize_field("path", &self.path)?;
        record.serialize_field("action", &self.action)?;
        record.serialize_field("kind", &self.kind())?;
        record.serialize_field("reason", &self.reason)?;
        record.serialize_field("status", &self.status)?;
        record.serialize_field("a", &self.a)?;
        record.serialize_field("b", &self.b)?;
        record.end()
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub id: String,
    pub task: Task,
    pub root_a: String,
    pub root_b: String,
    pub operations: Vec<Operation>,
    pub metrics: Metrics,
    pub baseline_generation: i64,
    pub created_at: u64,
    pub status: String,
    #[serde(default)]
    pub deletion_review: Option<DeletionReview>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Version {
    pub id: String,
    pub task_id: String,
    pub side: String,
    pub path: String,
    pub digest: String,
    pub stored: PathBuf,
    pub created_at: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RestorePreview {
    pub id: String,
    pub version: Version,
    pub current: Option<Entry>,
    pub root_identity: String,
    pub generation: i64,
    #[serde(default)]
    pub archived: bool,
    #[serde(default)]
    pub target_root: PathBuf,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecoveryTask {
    pub task: Task,
    pub versions: u64,
    pub archived_at: u64,
}
pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskStatus {
    pub task_id: String,
    pub last_run: Option<PlanHeader>,
    pub pending_conflicts: u64,
    pub next_run: Option<i64>,
    pub deletion_baseline_run: Option<String>,
}
/// Presentation boundary: never includes the potentially large operation list.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanHeader {
    pub id: String,
    pub task: Task,
    pub metrics: Metrics,
    pub created_at: u64,
    pub status: String,
    #[serde(default)]
    pub deletion_review: Option<DeletionReview>,
    #[serde(default)]
    pub summary: PlanSummary,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct PlanSummary {
    pub entries: u64,
    pub copies: u64,
    pub deletes: u64,
    pub conflicts: u64,
    #[serde(default)]
    pub ignored: u64,
    pub unchanged: u64,
    pub to_a: u64,
    pub to_b: u64,
    pub remove_a: u64,
    pub remove_b: u64,
    pub explicit: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct OperationQuery {
    pub offset: u32,
    pub limit: u32,
    pub filter: String,
    pub show_unchanged: bool,
    pub sort: String,
}
impl Default for OperationQuery {
    fn default() -> Self {
        Self {
            offset: 0,
            limit: 200,
            filter: String::new(),
            show_unchanged: false,
            sort: "path".into(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OperationPage {
    pub operations: Vec<Operation>,
    pub total: u64,
}
