export type Mode = "TwoWay" | "Mirror" | "Copy" | "Move";
export type Action =
  | "Unchanged"
  | "CopyToA"
  | "CopyToB"
  | "DeleteA"
  | "DeleteB"
  | "MoveToB"
  | "Conflict"
  | "Ignore";
export type OperationKind =
  | "Unchanged"
  | "Create"
  | "Update"
  | "Mkdir"
  | "DeleteToTrash"
  | "Move"
  | "Conflict"
  | "Ignore";
export interface Scope {
  include: string[];
  exclude: string[];
  folder_include: string[];
  folder_exclude: string[];
  selected_folders: string[];
  exclude_root_files: boolean;
  skip_hidden: boolean;
  skip_system: boolean;
}
export interface Schedule {
  kind: string;
  interval_minutes: number;
  hour: number;
  minute: number;
  weekdays: number[];
  month_day: number;
}
export interface Task {
  id: string;
  identifier: string;
  name: string;
  a: string;
  b: string;
  mode: Mode;
  group: string;
  scope: Scope;
  monitor: boolean;
  debounce_secs: number;
  retries: number;
  bandwidth_kib: number;
  transfer_concurrency: number;
  comparison: "Incremental" | "FullContent";
  conflict_policy: "ContinueIndependent" | "RequireChoices";
  history_enabled: boolean;
  history_patterns: string[];
  history_cap: number;
  schedule: Schedule;
  login: boolean;
  deletion_policy: {
    enabled: boolean;
    max_entries: number;
    max_percent: number;
  };
}
export interface Entry {
  path: string;
  digest: string;
  directory: boolean;
  size: number;
  mtime_ns: string;
}
export interface Operation {
  path: string;
  action: Action;
  kind?: OperationKind;
  reason: string;
  status?: string;
  a: Entry | null;
  b: Entry | null;
}
export interface Metrics {
  lineage_links?: number;
  peak_workers?: number;
  scanned_entries: number;
  hashed_bytes: number;
  copied_bytes: number;
  queued_paths: number;
  reconciliations: number;
  watch_events: number;
}
export interface Plan {
  id: string;
  task: Task;
  summary: {
    entries: number;
    copies: number;
    deletes: number;
    conflicts: number;
    ignored?: number;
    unchanged: number;
    to_a: number;
    to_b: number;
    remove_a: number;
    remove_b: number;
    explicit: boolean;
  };
  metrics: Metrics;
  created_at: number;
  status: string;
  deletion_review?: {
    reason:
      | "Approved"
      | "Disabled"
      | "BaselineApprovalRequired"
      | "UnprovenDeletion"
      | "EntryLimit"
      | "PercentLimit"
      | "MoveRequiresReview";
    entries: number;
    baseline_entries: number;
    max_entries: number;
    max_percent: number;
    baseline_run: string | null;
  } | null;
}
export interface OperationPage {
  operations: Operation[];
  total: number;
}
export interface ApplyOutcome {
  plan: Plan;
  error: string | null;
}
export interface RunProgress {
  active_transfers?: number;
  max_transfers?: number;
  task_id: string;
  run_id: string | null;
  phase:
    | "Discovering"
    | "Validating"
    | "Applying"
    | "Transferring"
    | "Protecting"
    | "Verifying";
  completed_entries: number;
  total_entries: number;
  path: string | null;
  file_bytes: number;
  file_total: number;
}
export interface Version {
  id: string;
  task_id: string;
  side: string;
  path: string;
  digest: string;
  stored: string;
  created_at: number;
}
export interface RestorePreview {
  id: string;
  version: Version;
  current: Entry | null;
  archived: boolean;
  target_root: string;
}
export interface RecoveryTask {
  task: Task;
  versions: number;
  archived_at: number;
}
export interface TaskStatus {
  task_id: string;
  last_run: Plan | null;
  pending_conflicts: number;
  next_run: number | null;
  deletion_baseline_run?: string | null;
}
export function newTask(mode: Mode = "TwoWay"): Task {
  return {
    id: crypto.randomUUID(),
    identifier: "",
    name: "",
    a: "",
    b: "",
    mode,
    group: "",
    scope: {
      include: [],
      exclude: [],
      folder_include: [],
      folder_exclude: [],
      selected_folders: [],
      exclude_root_files: false,
      skip_hidden: false,
      skip_system: false,
    },
    monitor: false,
    debounce_secs: 20,
    retries: 3,
    bandwidth_kib: 0,
    transfer_concurrency: 1,
    comparison: "Incremental",
    conflict_policy: "ContinueIndependent",
    history_enabled: true,
    history_patterns: [],
    history_cap: 10,
    schedule: {
      kind: "",
      interval_minutes: 15,
      hour: 23,
      minute: 0,
      weekdays: [1],
      month_day: 1,
    },
    login: false,
    deletion_policy: { enabled: false, max_entries: 20, max_percent: 10 },
  };
}
