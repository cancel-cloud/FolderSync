import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import {
  ArrowLeft,
  ArrowLeftRight,
  ArrowRight,
  Copy,
  FolderOpen,
  LayoutDashboard,
  Plus,
  Settings,
  ShieldCheck,
  RefreshCw,
  Pause,
  Square,
  FileClock,
  CheckCircle2,
} from "lucide-react";
import { PathPair } from "./components/path-pair";
import { Confirmation } from "./components/ui/confirmation";
import { Button } from "./components/ui/button";
import type {
  Task,
  Mode,
  Plan,
  Version,
  Entry,
  RestorePreview,
  TaskStatus,
  OperationPage,
  ApplyOutcome,
  RunProgress,
  RecoveryTask,
} from "./types";
import { newTask } from "./types";
import {
  actionLabel,
  operationLabel,
  statusLabel,
  reasonLabel,
  entryLabel,
  formatDate,
  formatBytes,
  modifiedLabel,
  triggerLabel,
  messageLabel,
  deletionReasonLabel,
} from "./i18n";
import "./style.css";
import packageInfo from "../package.json";
import { FilterHelp, filterExamples } from "./components/filter-help";
const englishModeLabels: Record<Mode, string> = {
  TwoWay: "Two-way sync",
  Mirror: "Mirror A → B",
  Copy: "Copy A → B",
  Move: "Move A → B",
};
const englishModeDescriptions: Record<Mode, string> = {
  TwoWay: "Keep changes in both folders. Conflicts stay protected.",
  Mirror: "Make B reflect A. Review destination deletions before applying.",
  Copy: "Add and update files in B. Keep destination-only files.",
  Move: "Verify each transfer to B before removing its source.",
};
const lines = (s: string) =>
  s
    .split("\n")
    .map((x) => x.trim())
    .filter(Boolean);
export default function App() {
  const [tasks, setTasks] = useState<Task[]>([]),
    [statuses, setStatuses] = useState<TaskStatus[]>([]),
    [page, setPage] = useState("dashboard"),
    [task, setTask] = useState<Task | null>(null),
    [original, setOriginal] = useState(""),
    [plan, setPlan] = useState<Plan | null>(null),
    [runs, setRuns] = useState<Plan[]>([]),
    [versions, setVersions] = useState<Version[]>([]),
    [error, setError] = useState(""),
    [message, setMessage] = useState(""),
    [busy, setBusy] = useState(false),
    [storage, setStorage] = useState(""),
    [lang, setLang] = useState("en"),
    [paused, setPaused] = useState(false),
    [filter, setFilter] = useState(""),
    [showUnchanged, setShowUnchanged] = useState(false),
    [selected, setSelected] = useState<string[]>([]),
    [confirm, setConfirm] = useState<{
      title: string;
      body: string;
      action: () => Promise<void>;
    } | null>(null),
    [approved, setApproved] = useState(false),
    [startup, setStartup] = useState(false),
    [notifications, setNotifications] = useState(true),
    [logLevel, setLogLevel] = useState("off"),
    [closeBehavior, setCloseBehavior] = useState("tray"),
    [logText, setLogText] = useState<string | null>(null),
    [samplePath, setSamplePath] = useState("project/file.txt"),
    [sampleResult, setSampleResult] = useState(""),
    [operationPage, setOperationPage] = useState(0),
    [sort, setSort] = useState("path");
  const [settingsTab, setSettingsTab] = useState("general");
  const [taskView, setTaskView] = useState("tiles");
  const [showFilterHelp, setShowFilterHelp] = useState(false);
  const [notificationFeedback, setNotificationFeedback] = useState("");
  const [notificationStatus, setNotificationStatus] = useState<{
    app_enabled: boolean;
    app_id: string;
    windows_setting: string;
    windows_error?: string | null;
    banners?: boolean | null;
    presentation?: string;
  } | null>(null);
  const [progress, setProgress] = useState<RunProgress | null>(null);
  const [failedPlanId, setFailedPlanId] = useState("");
  const [recoveryTasks, setRecoveryTasks] = useState<RecoveryTask[]>([]);
  const [archivedDetail, setArchivedDetail] = useState(false);
  const [groupFilter, setGroupFilter] = useState("*");
  const [collapsedGroups, setCollapsedGroups] = useState<string[]>([]);
  const [selectedGroup, setSelectedGroup] = useState("");
  const closeContext = useRef({ dirty: false, language: "en" });
  useLayoutEffect(() => {
    closeContext.current = {
      dirty:
        page === "editor" && task !== null && JSON.stringify(task) !== original,
      language: lang,
    };
  }, [page, task, original, lang]);
  useEffect(() => {
    const listener = listen<null>("request-close", () => {
      const quit = async () => {
        await invoke("quit");
      };
      if (closeContext.current.dirty) {
        const de = closeContext.current.language === "de";
        setConfirm({
          title: de
            ? "Ungespeicherte Änderungen verwerfen und beenden?"
            : "Discard unsaved changes and quit?",
          body: de
            ? "Die Änderungen an dieser Aufgabe wurden nicht gespeichert. Aktive Arbeit wird sicher gestoppt, bevor FolderSync beendet wird."
            : "This task's changes have not been saved. Active work will stop safely before FolderSync quits.",
          action: quit,
        });
      } else {
        void quit().catch((e) => setError(String(e)));
      }
    });
    return () => {
      void listener.then((unlisten) => unlisten());
    };
  }, []);
  const groupNames = [...new Set(tasks.map((t) => t.group))].sort();
  const filteredTasks = tasks.filter(
    (t) => groupFilter === "*" || `group:${t.group}` === groupFilter,
  );
  const visibleTasks = filteredTasks.filter(
    (t) => !collapsedGroups.includes(t.group),
  );
  const taskGroups = groupNames
    .map((name) => ({
      name,
      tasks: filteredTasks.filter((t) => t.group === name),
    }))
    .filter((group) => group.tasks.length > 0);
  const [operationRows, setOperationRows] = useState<
    (OperationPage & { key: string }) | null
  >(null);
  const operationKey = JSON.stringify([
    plan,
    filter,
    showUnchanged,
    sort,
    operationPage,
  ]);
  useEffect(() => {
    if (!plan || page !== "preview") return;
    let current = true;
    void invoke<OperationPage>("operation_page", {
      id: plan.id,
      query: {
        offset: operationPage * 200,
        limit: 200,
        filter,
        show_unchanged: showUnchanged,
        sort,
      },
    })
      .then((result) => {
        if (current) setOperationRows({ ...result, key: operationKey });
      })
      .catch((e) => {
        if (current) setError(String(e));
      });
    return () => {
      current = false;
    };
  }, [plan, page, filter, showUnchanged, sort, operationPage, operationKey]);
  const rows = operationRows?.key === operationKey ? operationRows : null;
  const tr = (en: string, de: string) => (lang === "de" ? de : en);
  const date = (seconds: number) => formatDate(seconds, lang);
  const bytes = (value: number) => formatBytes(value, lang);
  const meta = (entry: Entry | null) => entryLabel(entry, lang);
  const deletionReady =
    !!task &&
    !!statuses.find((s) => s.task_id === task.id)?.deletion_baseline_run &&
    JSON.stringify(tasks.find((t) => t.id === task.id)?.scope) ===
      JSON.stringify(task.scope) &&
    ["TwoWay", "Mirror"].includes(task.mode);
  const modeLabels: Record<Mode, string> =
    lang === "de"
      ? {
          TwoWay: "Zwei-Wege-Synchronisation",
          Mirror: "Spiegeln A → B",
          Copy: "Kopieren A → B",
          Move: "Verschieben A → B",
        }
      : englishModeLabels;
  const modeDescriptions: Record<Mode, string> =
    lang === "de"
      ? {
          TwoWay:
            "Änderungen in beiden Ordnern abgleichen. Konflikte bleiben geschützt.",
          Mirror: "B an A angleichen. Löschungen am Ziel vorher prüfen.",
          Copy: "Dateien in B ergänzen und aktualisieren. Zusätzliche Zieldateien behalten.",
          Move: "Jede Übertragung nach B prüfen, bevor die Quelle entfernt wird.",
        }
      : englishModeDescriptions;
  useEffect(() => {
    if (page === "settings") {
      void invoke<typeof notificationStatus>("notification_status")
        .then(setNotificationStatus)
        .catch(() => setNotificationStatus(null));
    }
  }, [page, notifications]);
  async function refresh() {
    const [t, s] = await Promise.all([
      invoke<Task[]>("tasks"),
      invoke<TaskStatus[]>("statuses"),
    ]);
    setTasks(t);
    setStatuses(s);
    setSelected((current) =>
      current.filter((id) => t.some((task) => task.id === id)),
    );
  }
  useEffect(() => {
    let mounted = true;
    void Promise.all([
      invoke<Task[]>("tasks"),
      invoke<TaskStatus[]>("statuses"),
      invoke<string>("storage"),
      invoke<string | null>("setting", { key: "language" }),
      invoke<boolean>("startup_enabled"),
      invoke<string | null>("setting", { key: "notifications" }),
      invoke<string | null>("setting", { key: "log_level" }),
      invoke<string | null>("setting", { key: "close_behavior" }),
      invoke<string | null>("setting", { key: "task_view" }),
    ])
      .then(
        ([
          tasks,
          statuses,
          path,
          language,
          enabled,
          notifications,
          logging,
          closing,
          view,
        ]) => {
          if (mounted) {
            setTasks(tasks);
            setStatuses(statuses);
            setStorage(path);
            if (language) setLang(language);
            setStartup(Boolean(enabled));
            setNotifications(notifications !== "off");
            setLogLevel(logging || "off");
            setCloseBehavior(closing === "quit" ? "quit" : "tray");
            setTaskView(view === "list" ? "list" : "tiles");
          }
        },
      )
      .catch((e) => {
        if (mounted) setError(String(e));
      });
    const p = listen<boolean>("automation-paused", (e) => setPaused(e.payload));
    let progressEventSeen = false;
    const progressListener = listen<RunProgress | null>(
      "run-progress",
      (event) => {
        progressEventSeen = true;
        if (mounted) setProgress(event.payload);
      },
    );
    void progressListener
      .then(() => invoke<RunProgress | null>("progress"))
      .then((value) => {
        if (mounted && !progressEventSeen) setProgress(value);
      })
      .catch((e) => {
        if (mounted) setError(String(e));
      });
    const f = listen<{ status: string }>("run-finished", (e) => {
      setMessage(e.payload.status);
      void refresh();
    });
    const b = listen<string>("background-error", (e) => setError(e.payload));
    const openTask = listen<string>("open-task", (e) => {
      void perform(async () => {
        const tasks = await invoke<Task[]>("tasks");
        const task = tasks.find((t) => t.id === e.payload);
        if (task) await details(task);
      });
    });
    return () => {
      mounted = false;
      void p.then((f) => f());
      void progressListener.then((f) => f());
      void f.then((f) => f());
      void b.then((f) => f());
      void openTask.then((f) => f());
    };
  }, []);
  useEffect(() => {
    document.documentElement.lang = lang;
  }, [lang]);
  async function perform(action: () => Promise<void>) {
    setBusy(true);
    setError("");
    setMessage("");
    try {
      await action();
    } catch (e) {
      if (e === "[FolderSync:Cancelled]")
        setMessage(
          "Run cancelled. Completed operations remain committed. Create a fresh preview to continue.",
        );
      else setError(String(e));
    } finally {
      setBusy(false);
    }
  }
  function storageLocation(selectedTask?: Task | null) {
    return (
      <div className="storage-location">
        <div>
          {selectedTask && (
            <strong>
              {tr("Task ID: ", "Aufgaben-ID: ")}
              {selectedTask.identifier || selectedTask.id}
            </strong>
          )}
          <code>
            {storage}
            {selectedTask ? "\\versions" : ""}
          </code>
        </div>
        <Button
          variant="outline"
          disabled={busy || !storage}
          aria-label={
            selectedTask
              ? tr("Open backup folder", "Sicherungsordner öffnen")
              : tr("Open local data folder", "Lokalen Datenordner öffnen")
          }
          onClick={() =>
            void perform(async () => {
              await invoke("open_storage", {
                taskId: selectedTask?.id ?? null,
              });
            })
          }
        >
          <FolderOpen size={16} />
          {tr("Open", "Öffnen")}
        </Button>
      </div>
    );
  }
  function navigate(next: string) {
    if (page === "editor" && task && JSON.stringify(task) !== original) {
      setConfirm({
        title: tr(
          "Discard unsaved changes?",
          "Ungespeicherte Änderungen verwerfen?",
        ),
        body: tr(
          "Your saved task will remain as it was.",
          "Die gespeicherte Aufgabe bleibt unverändert.",
        ),
        action: async () => {
          setPage(next);
          setTask(null);
        },
      });
    } else setPage(next);
  }
  function edit(t: Task) {
    setTask(structuredClone(t));
    setOriginal(JSON.stringify(t));
    setPage("editor");
  }
  function requestSaveTask() {
    if (!task) return;
    const snapshot = structuredClone(task);
    const previous = original ? (JSON.parse(original) as Task) : null;
    const saveTask = () =>
      perform(async () => {
        await invoke("save_task", { task: snapshot });
        setSelected([]);
        await refresh();
        setOriginal(JSON.stringify(snapshot));
        setPage("dashboard");
        setMessage(
          tr(
            "Task saved. Preview the first run.",
            "Aufgabe gespeichert. Prüfen Sie den ersten Lauf.",
          ),
        );
      });
    if (
      snapshot.deletion_policy.enabled &&
      (JSON.stringify(snapshot.deletion_policy) !==
        JSON.stringify(previous?.deletion_policy) ||
        snapshot.monitor !== previous?.monitor ||
        snapshot.login !== previous?.login ||
        JSON.stringify(snapshot.schedule) !==
          JSON.stringify(previous?.schedule))
    ) {
      setConfirm({
        title: tr(
          "Enable automatic deletion?",
          "Automatische Entfernung aktivieren?",
        ),
        body: tr(
          `Future automatic runs may remove up to ${snapshot.deletion_policy.max_entries} entries and ${snapshot.deletion_policy.max_percent}% of the indexed common baseline per run, only when deletion is proven. A: ${snapshot.a}. B: ${snapshot.b}. Protected versions are retained. Unproven removals and Move require review.`,
          `Künftige automatische Läufe dürfen je Lauf bis zu ${snapshot.deletion_policy.max_entries} Einträge und ${snapshot.deletion_policy.max_percent}% des indizierten gemeinsamen Ausgangszustands entfernen, sofern die Entfernung nachgewiesen ist. A: ${snapshot.a}. B: ${snapshot.b}. Geschützte Versionen bleiben erhalten. Unbelegte Entfernungen und Verschieben erfordern Prüfung.`,
        ),
        action: saveTask,
      });
    } else void saveTask();
  }
  async function preview(t: Task) {
    await perform(async () => {
      try {
        const p = await invoke<Plan>("preview", { id: t.id });
        setPlan(p);
        setTask(t);
        setPage("preview");
        setApproved(false);
        setOperationPage(0);
      } finally {
        await refresh();
      }
    });
  }
  async function details(t: Task, archived = false) {
    await perform(async () => {
      setTask(t);
      setArchivedDetail(archived);
      setRuns(await invoke<Plan[]>("history", { id: t.id }));
      setVersions(await invoke<Version[]>("versions", { id: t.id }));
      setPage("details");
    });
  }
  async function openRecovery() {
    await perform(async () => {
      setRecoveryTasks(await invoke<RecoveryTask[]>("recovery_tasks"));
      navigate("recovery");
    });
  }
  async function choose(side: "a" | "b") {
    const path = await open({
      directory: true,
      multiple: false,
      title: tr(
        `Select folder ${side.toUpperCase()}`,
        `Ordner ${side.toUpperCase()} auswählen`,
      ),
    });
    if (typeof path === "string" && task) setTask({ ...task, [side]: path });
  }
  async function exportTasks() {
    const path = await save({
      defaultPath: "foldersync-tasks.json",
      filters: [{ name: "FolderSync JSON", extensions: ["json"] }],
    });
    if (path)
      await perform(async () => {
        await invoke("export_tasks", { path });
        setMessage("Task configuration exported.");
      });
  }
  async function importTasks() {
    const path = await open({
      multiple: false,
      filters: [{ name: "FolderSync JSON", extensions: ["json"] }],
    });
    if (typeof path === "string")
      await perform(async () => {
        const n = await invoke<number>("import_tasks", { path });
        await refresh();
        setMessage(`Imported ${n} tasks. All automatic triggers are disabled.`);
      });
  }
  async function restore(v: Version) {
    await perform(async () => {
      const p = await invoke<RestorePreview>("restore_preview", { id: v.id });
      setConfirm({
        title: tr(
          "Restore protected version?",
          "Geschützte Version wiederherstellen?",
        ),
        body: tr(
          `Destination: ${p.target_root}/${v.path}. Current: ${meta(p.current)}. Restore: ${v.digest.slice(0, 16)}. The current version will also be protected.`,
          `Ziel: ${p.target_root}/${v.path}. Aktuell: ${meta(p.current)}. Wiederherstellen: ${v.digest.slice(0, 16)}. Die aktuelle Version wird ebenfalls geschützt.`,
        ),
        action: async () => {
          await perform(async () => {
            await invoke("restore_apply", { id: p.id });
            setMessage(
              p.archived
                ? tr(
                    "Protected version restored. The deleted task remains inactive.",
                    "Geschützte Version wiederhergestellt. Die gelöschte Aufgabe bleibt inaktiv.",
                  )
                : "Protected version restored. Preview the task to reconcile this change.",
            );
            if (task)
              setVersions(await invoke<Version[]>("versions", { id: task.id }));
          });
        },
      });
    });
  }
  function patch(p: Partial<Task>) {
    if (task) setTask({ ...task, ...p });
  }
  const summary = plan?.summary;
  const needsConflictChoices =
    plan?.task.conflict_policy === "RequireChoices" &&
    !!summary &&
    summary.conflicts > (summary.ignored || 0);
  const archivedPlan = !!plan && !tasks.some((t) => t.id === plan.task.id);
  const direction = plan?.summary;
  return (
    <div className="app">
      <aside>
        <div className="brand">
          <div className="mark">
            <ArrowLeftRight size={22} />
          </div>
          <div>
            FolderSync
            <small>{tr("LOCAL WORKSPACE", "LOKALER ARBEITSBEREICH")}</small>
          </div>
        </div>
        <nav aria-label={tr("Main navigation", "Hauptnavigation")}>
          <button
            className={page === "dashboard" ? "active" : ""}
            onClick={() => navigate("dashboard")}
          >
            <LayoutDashboard size={18} />
            {tr("Tasks", "Aufgaben")}
            <span>{tasks.length}</span>
          </button>
          <button
            className={page === "settings" ? "active" : ""}
            onClick={() => navigate("settings")}
          >
            <Settings size={18} />
            {tr("Settings", "Einstellungen")}
          </button>
          <button
            className={
              page === "recovery" || (page === "details" && archivedDetail)
                ? "active"
                : ""
            }
            onClick={() => void openRecovery()}
          >
            <FileClock size={18} />
            {tr("Recovery", "Wiederherstellung")}
          </button>
        </nav>
      </aside>
      <div className="workspace">
        <header>
          <div className="breadcrumb">
            {tr("Workspace", "Arbeitsbereich")}
            <span>/</span>
            {page === "dashboard"
              ? tr("Tasks", "Aufgaben")
              : page === "editor"
                ? tr("Task configuration", "Aufgabenkonfiguration")
                : page === "preview"
                  ? tr("Operation review", "Aktionsprüfung")
                  : page === "details" || page === "recovery"
                    ? tr("History & recovery", "Verlauf & Wiederherstellung")
                    : tr("Settings", "Einstellungen")}
          </div>
          <div className="header-actions">
            {page === "dashboard" && selected.length > 0 && (
              <div
                className="selection-actions"
                aria-label={tr(
                  "Selected task actions",
                  "Aktionen für ausgewählte Aufgaben",
                )}
              >
                <span>
                  {selected.length} {tr("selected", "ausgewählt")}
                </span>
                <Button
                  variant="outline"
                  disabled={busy}
                  onClick={() =>
                    void perform(async () => {
                      await invoke("disable_tasks", { ids: [...selected] });
                      setSelected([]);
                      await refresh();
                    })
                  }
                >
                  {tr("Disable selected", "Auswahl deaktivieren")}
                </Button>
                <Button
                  variant="destructive"
                  disabled={busy}
                  onClick={() => {
                    const ids = [...selected];
                    setConfirm({
                      title: tr(
                        "Delete selected tasks?",
                        "Ausgewählte Aufgaben löschen?",
                      ),
                      body: tr(
                        `Remove ${ids.length} task configurations and their sync baselines? Files in A/B and protected backups stay intact.`,
                        `${ids.length} Aufgabenkonfigurationen und ihre Sync-Ausgangszustände entfernen? Dateien in A/B und geschützte Sicherungen bleiben erhalten.`,
                      ),
                      action: () =>
                        perform(async () => {
                          await invoke("delete_tasks", { ids });
                          setSelected([]);
                          await refresh();
                        }),
                    });
                  }}
                >
                  {tr("Delete selected", "Auswahl löschen")}
                </Button>
              </div>
            )}
            {page === "settings" && (
              <span className="preference-status" role="status">
                {busy ? tr("Saving…", "Speichern…") : ""}
              </span>
            )}
            <span className="status-dot" />
            {paused
              ? tr("Automation paused", "Automatik pausiert")
              : tr("Ready to sync", "Bereit zum Synchronisieren")}
            <Button
              variant="ghost"
              aria-label={tr(
                "Pause or resume automation",
                "Automatik pausieren oder fortsetzen",
              )}
              onClick={() =>
                void perform(async () => {
                  await invoke("pause", { value: !paused });
                  setPaused(!paused);
                })
              }
            >
              <Pause size={15} />
            </Button>
            <Button
              variant="ghost"
              aria-label={tr("Stop active work", "Aktiven Lauf stoppen")}
              onClick={() => void invoke("stop")}
            >
              <Square size={15} />
            </Button>
          </div>
        </header>
        <main
          aria-busy={busy}
          className={page === "settings" ? "settings-main" : undefined}
        >
          {error && (
            <div className="alert error" role="alert">
              <strong>
                {tr(
                  "Action could not complete",
                  "Aktion konnte nicht abgeschlossen werden",
                )}
              </strong>
              <p>{error}</p>
              <button onClick={() => setError("")}>
                {tr("Dismiss", "Schließen")}
              </button>
            </div>
          )}
          {message && (
            <div
              className={
                message.startsWith("Run cancelled.") || message === "Cancelled"
                  ? "alert warning"
                  : "alert success"
              }
              role="status"
            >
              {messageLabel(message, lang)}
            </div>
          )}
          {(progress || (busy && page !== "settings")) && (
            <div className="working" role="status">
              <RefreshCw size={16} className="spin" />
              {tr(
                "Working… files are verified before replacement.",
                "In Arbeit… Dateien werden vor dem Ersetzen geprüft.",
              )}
              {progress && (
                <div className="run-progress">
                  <strong>
                    {
                      {
                        Discovering: tr(
                          "Discovering files",
                          "Dateien erfassen",
                        ),
                        Validating: tr(
                          "Validating reviewed inputs",
                          "Geprüfte Eingaben validieren",
                        ),
                        Applying: tr(
                          "Applying reviewed operations",
                          "Geprüfte Aktionen anwenden",
                        ),
                        Transferring: tr("Copying file", "Datei kopieren"),
                        Protecting: tr(
                          "Protecting previous version",
                          "Vorherige Version schützen",
                        ),
                        Verifying: tr(
                          "Verifying staged copy",
                          "Zwischengespeicherte Kopie prüfen",
                        ),
                      }[progress.phase]
                    }
                  </strong>
                  {progress.path && <code>{progress.path}</code>}
                  {(progress.max_transfers ?? 1) > 1 && (
                    <small>
                      {progress.active_transfers ?? 0} /{" "}
                      {progress.max_transfers}{" "}
                      {tr("active transfers", "aktive Übertragungen")}
                    </small>
                  )}
                  {progress.total_entries > 0 && (
                    <small>
                      {progress.completed_entries} / {progress.total_entries}{" "}
                      {tr("reviewed entries", "geprüfte Einträge")}
                    </small>
                  )}
                  {progress.phase === "Discovering" &&
                    progress.total_entries === 0 && (
                      <small>
                        {progress.completed_entries}{" "}
                        {tr("entries examined", "Einträge geprüft")}
                      </small>
                    )}
                  {progress.file_total > 0 && (
                    <>
                      <progress
                        aria-label={tr(
                          "Current file progress",
                          "Fortschritt der aktuellen Datei",
                        )}
                        value={progress.file_bytes}
                        max={progress.file_total}
                      />
                      <small>
                        {bytes(progress.file_bytes)} /{" "}
                        {bytes(progress.file_total)}{" "}
                        {["Transferring", "Protecting"].includes(progress.phase)
                          ? tr(
                              "staged bytes; verification and commit follow",
                              "zwischengespeicherte Bytes; Prüfung und Übernahme folgen",
                            )
                          : tr(
                              "hashed bytes in current file",
                              "gehashte Bytes der aktuellen Datei",
                            )}
                      </small>
                    </>
                  )}
                </div>
              )}
              <Button variant="ghost" onClick={() => void invoke("stop")}>
                {tr("Cancel", "Abbrechen")}
              </Button>
            </div>
          )}
          {page === "dashboard" && (
            <>
              <div className="page-heading">
                <div>
                  <div className="eyebrow">
                    {tr("FOLDER SYNCHRONIZATION", "ORDNERSYNCHRONISATION")}
                  </div>
                  <h1>{tr("Your sync tasks", "Ihre Sync-Aufgaben")}</h1>
                  <p>
                    {tr(
                      "Review changes. Keep control of every transfer.",
                      "Änderungen prüfen. Jede Übertragung im Blick behalten.",
                    )}
                  </p>
                </div>
                <Button onClick={() => edit(newTask())}>
                  <Plus size={17} />
                  {tr("New task", "Neue Aufgabe")}
                </Button>
              </div>
              {tasks.length > 0 && (
                <>
                  <div className="toolbar dashboard-toolbar">
                    <label>
                      {tr("Filter by group", "Nach Gruppe filtern")}
                      <select
                        disabled={busy}
                        aria-label={tr(
                          "Filter by group",
                          "Nach Gruppe filtern",
                        )}
                        value={groupFilter}
                        onChange={(e) => {
                          setGroupFilter(e.target.value);
                          setSelected([]);
                        }}
                      >
                        <option value="*">
                          {tr("All groups", "Alle Gruppen")}
                        </option>
                        {groupNames.map((name) => (
                          <option key={name} value={`group:${name}`}>
                            {name || tr("Ungrouped", "Ohne Gruppe")}
                          </option>
                        ))}
                      </select>
                    </label>
                    <label>
                      <input
                        type="checkbox"
                        aria-label={tr(
                          "Select all visible tasks",
                          "Alle sichtbaren Aufgaben auswählen",
                        )}
                        disabled={busy || visibleTasks.length === 0}
                        checked={
                          visibleTasks.length > 0 &&
                          visibleTasks.every((t) => selected.includes(t.id))
                        }
                        onChange={(e) =>
                          setSelected(
                            e.target.checked
                              ? visibleTasks.map((t) => t.id)
                              : [],
                          )
                        }
                      />
                      {tr("Select tasks", "Aufgaben auswählen")}
                    </label>
                    <Button
                      variant="outline"
                      disabled={!selected.length || busy}
                      onClick={() =>
                        void perform(async () => {
                          for (const id of selected) {
                            await invoke("preview", { id });
                          }
                          setMessage(
                            `${selected.length} previews saved. Open Details to review each before applying.`,
                          );
                        })
                      }
                    >
                      {tr("Preview selected", "Auswahl prüfen")}
                    </Button>
                    <Button
                      variant="ghost"
                      onClick={() => void perform(refresh)}
                    >
                      <RefreshCw size={15} />
                      {tr("Refresh", "Aktualisieren")}
                    </Button>
                    <div
                      className="view-switch"
                      aria-label={tr("Task view", "Aufgabenansicht")}
                    >
                      {(["tiles", "list"] as const).map((view) => (
                        <Button
                          key={view}
                          variant={taskView === view ? "default" : "outline"}
                          aria-pressed={taskView === view}
                          disabled={busy}
                          onClick={() =>
                            void perform(async () => {
                              await invoke("set_setting", {
                                key: "task_view",
                                value: view,
                              });
                              setTaskView(view);
                            })
                          }
                        >
                          {view === "tiles"
                            ? tr("Tiles", "Kacheln")
                            : tr("List", "Liste")}
                        </Button>
                      ))}
                    </div>
                  </div>
                  <div className="toolbar group-assignment">
                    <label>
                      {tr(
                        "Group for selected tasks",
                        "Gruppe für ausgewählte Aufgaben",
                      )}
                      <input
                        aria-label={tr(
                          "Group for selected tasks",
                          "Gruppe für ausgewählte Aufgaben",
                        )}
                        value={selectedGroup}
                        maxLength={128}
                        disabled={busy}
                        placeholder={tr(
                          "Empty = ungrouped",
                          "Leer = ohne Gruppe",
                        )}
                        onChange={(e) => setSelectedGroup(e.target.value)}
                      />
                    </label>
                    <Button
                      variant="outline"
                      disabled={busy || !selected.length}
                      onClick={() =>
                        void perform(async () => {
                          await invoke("assign_group", {
                            ids: selected,
                            group: selectedGroup,
                          });
                          setSelected([]);
                          setGroupFilter("*");
                          setCollapsedGroups([]);
                          await refresh();
                          setMessage(
                            tr(
                              "Task groups saved.",
                              "Aufgabengruppen gespeichert.",
                            ),
                          );
                        })
                      }
                    >
                      {tr("Assign group", "Gruppe zuweisen")}
                    </Button>
                    <span>
                      {selected.length} {tr("selected", "ausgewählt")}
                    </span>
                  </div>
                  {taskGroups.length === 0 && (
                    <p>
                      {tr(
                        "No tasks in this group.",
                        "Keine Aufgaben in dieser Gruppe.",
                      )}
                    </p>
                  )}
                  <div className={`task-list task-view-${taskView}`}>
                    {taskGroups.map((group) => (
                      <section className="task-group" key={group.name}>
                        <Button
                          variant="ghost"
                          disabled={busy}
                          aria-expanded={!collapsedGroups.includes(group.name)}
                          onClick={() => {
                            setCollapsedGroups((current) =>
                              current.includes(group.name)
                                ? current.filter((name) => name !== group.name)
                                : [...current, group.name],
                            );
                            setSelected((current) =>
                              current.filter(
                                (id) => !group.tasks.some((t) => t.id === id),
                              ),
                            );
                          }}
                        >
                          {group.name || tr("Ungrouped", "Ohne Gruppe")} (
                          {group.tasks.length})
                        </Button>
                        <div className="task-group-items">
                          {!collapsedGroups.includes(group.name) &&
                            group.tasks.map((t) => (
                              <article className="task-card" key={t.id}>
                                <div className="task-top">
                                  <input
                                    type="checkbox"
                                    aria-label={tr(
                                      `Select ${t.name}`,
                                      `${t.name} auswählen`,
                                    )}
                                    disabled={busy}
                                    checked={selected.includes(t.id)}
                                    onChange={(e) =>
                                      setSelected(
                                        e.target.checked
                                          ? [...selected, t.id]
                                          : selected.filter((x) => x !== t.id),
                                      )
                                    }
                                  />
                                  <div className="task-icon">
                                    {t.mode === "TwoWay" ? (
                                      <ArrowLeftRight size={20} />
                                    ) : t.mode === "Copy" ? (
                                      <Copy size={20} />
                                    ) : (
                                      <ArrowRight size={20} />
                                    )}
                                  </div>
                                  <div className="task-title">
                                    <h2>{t.name}</h2>
                                    <small>ID: {t.identifier || t.id}</small>
                                    <span>
                                      {modeLabels[t.mode]}
                                      {t.group && ` · ${t.group}`}
                                    </span>
                                  </div>
                                  <span className="badge">
                                    {triggerLabel(t, lang)}
                                  </span>
                                </div>
                                <PathPair a={t.a} b={t.b} lang={lang} />
                                <div className="task-bottom">
                                  <span>
                                    {statusLabel(
                                      statuses.find((s) => s.task_id === t.id)
                                        ?.last_run?.status || "",
                                      lang,
                                    ) ||
                                      tr(
                                        "Awaiting first run",
                                        "Wartet auf ersten Lauf",
                                      )}
                                    <small>
                                      {statuses.find((s) => s.task_id === t.id)
                                        ?.last_run
                                        ? date(
                                            statuses.find(
                                              (s) => s.task_id === t.id,
                                            )!.last_run!.created_at,
                                          )
                                        : "—"}{" "}
                                      ·{" "}
                                      {statuses.find((s) => s.task_id === t.id)
                                        ?.pending_conflicts || 0}{" "}
                                      {tr(
                                        "pending conflicts",
                                        "offene Konflikte",
                                      )}
                                    </small>
                                    <small>
                                      {tr("Next", "Nächster Lauf")}:{" "}
                                      {statuses.find((s) => s.task_id === t.id)
                                        ?.next_run
                                        ? date(
                                            statuses.find(
                                              (s) => s.task_id === t.id,
                                            )!.next_run!,
                                          )
                                        : "—"}
                                    </small>
                                  </span>
                                  <div>
                                    <Button
                                      variant="ghost"
                                      onClick={() => void details(t)}
                                    >
                                      {tr("Details", "Details")}
                                    </Button>
                                    <Button
                                      variant="outline"
                                      onClick={() => edit(t)}
                                    >
                                      {tr("Configure", "Konfigurieren")}
                                    </Button>
                                    <Button
                                      disabled={busy}
                                      onClick={() => void preview(t)}
                                    >
                                      {tr("Preview / Run", "Vorschau / Start")}
                                    </Button>
                                  </div>
                                </div>
                              </article>
                            ))}
                        </div>
                      </section>
                    ))}
                  </div>
                </>
              )}
              {tasks.length === 0 && (
                <div className="welcome">
                  <div className="welcome-icon">
                    <FolderOpen size={32} />
                  </div>
                  <h2>
                    {tr(
                      "Connect your first folder pair",
                      "Verbinden Sie Ihr erstes Ordnerpaar",
                    )}
                  </h2>
                  <p>
                    {tr(
                      "Choose how files should flow. Automatic triggers start off.",
                      "Wählen Sie die Richtung. Automatische Auslöser sind zunächst aus.",
                    )}
                  </p>
                </div>
              )}
              <div className="section-heading">
                <h3>{tr("Create a task", "Aufgabe erstellen")}</h3>
                <span>
                  {tr(
                    "Four ways to keep folders in step",
                    "Vier Wege für abgestimmte Ordner",
                  )}
                </span>
              </div>
              <div className="mode-grid">
                {(Object.keys(modeLabels) as Mode[]).map((mode, i) => (
                  <button
                    key={mode}
                    className="mode-card"
                    onClick={() => edit(newTask(mode))}
                  >
                    <span className="mode-number">0{i + 1}</span>
                    <h3>{modeLabels[mode]}</h3>
                    <p>{modeDescriptions[mode]}</p>
                    <span className="mode-arrow">↗</span>
                  </button>
                ))}
              </div>
            </>
          )}
          {page === "editor" && task && (
            <>
              <div className="page-heading">
                <div>
                  <button
                    className="back"
                    onClick={() => navigate("dashboard")}
                  >
                    <ArrowLeft size={16} />
                    {tr("All tasks", "Alle Aufgaben")}
                  </button>
                  <h1>
                    {tr("Configure folder pair", "Ordnerpaar konfigurieren")}
                  </h1>
                  <p>
                    {tr(
                      "Changes are saved separately from execution.",
                      "Speichern und Ausführen sind getrennte Schritte.",
                    )}
                  </p>
                </div>
                <Button disabled={busy} onClick={requestSaveTask}>
                  {tr("Save task", "Aufgabe speichern")}
                </Button>
              </div>
              <section className="panel">
                <h2>{tr("Task essentials", "Aufgabe")}</h2>
                <div className="form-grid">
                  <label>
                    {tr("Task name", "Aufgabenname")}
                    <input
                      value={task.name}
                      onChange={(e) => patch({ name: e.target.value })}
                      placeholder={tr(
                        "Engineering workspace",
                        "Entwicklungsarbeitsplatz",
                      )}
                    />
                  </label>
                  <label>
                    {tr("Task ID", "Aufgaben-ID")}
                    <input
                      value={task.identifier || ""}
                      aria-label={tr("Task ID", "Aufgaben-ID")}
                      placeholder={task.id}
                      onChange={(e) => patch({ identifier: e.target.value })}
                    />
                    <small>
                      {tr(
                        "Unique among active tasks. Leave blank for an automatic ID.",
                        "Eindeutig unter aktiven Aufgaben. Leer lassen für eine automatische ID.",
                      )}
                    </small>
                  </label>
                  <label>
                    {tr("Group", "Gruppe")}
                    <input
                      value={task.group}
                      onChange={(e) => patch({ group: e.target.value })}
                    />
                  </label>
                  <label className="full">
                    {tr("Synchronization mode", "Synchronisationsmodus")}
                    <select
                      value={task.mode}
                      disabled={tasks.some((t) => t.id === task.id)}
                      onChange={(e) => patch({ mode: e.target.value as Mode })}
                    >
                      {Object.entries(modeLabels).map(([v, label]) => (
                        <option key={v} value={v}>
                          {label}
                        </option>
                      ))}
                    </select>
                    <small>{modeDescriptions[task.mode]}</small>
                  </label>
                </div>
                {(["a", "b"] as const).map((side) => (
                  <label className="folder-field" key={side}>
                    <span className="folder-label">
                      <b>{side.toUpperCase()}</b>
                      {tr("Folder", "Ordner")} {side.toUpperCase()}
                    </span>
                    <div>
                      <input
                        aria-label={tr(
                          `Folder ${side.toUpperCase()} path`,
                          `Pfad von Ordner ${side.toUpperCase()}`,
                        )}
                        value={task[side]}
                        onChange={(e) => patch({ [side]: e.target.value })}
                        placeholder="C:\workspace or \\server\share\folder"
                      />
                      <Button
                        variant="outline"
                        onClick={() => void choose(side)}
                      >
                        <FolderOpen size={17} />
                        {tr("Browse…", "Durchsuchen…")}
                      </Button>
                    </div>
                  </label>
                ))}
                <p className="hint">
                  <ShieldCheck size={15} />
                  {tr(
                    "Same, nested and unsafe folder pairs are rejected. No files change when you save.",
                    "Gleiche und verschachtelte Ordner werden abgelehnt. Speichern ändert keine Dateien.",
                  )}
                </p>
              </section>
              <section className="panel">
                <div className="panel-heading">
                  <h2>{tr("Scope & filters", "Umfang & Filter")}</h2>
                  <Button
                    variant="outline"
                    onClick={() => setShowFilterHelp(true)}
                  >
                    {tr("Filter help & examples", "Filterhilfe & Beispiele")}
                  </Button>
                </div>
                <p>
                  {tr(
                    "One glob per line. Exclusions win. Paths are relative to each root.",
                    "Ein Glob pro Zeile. Ausschlüsse haben Vorrang. Pfade relativ zum Stammordner.",
                  )}
                </p>
                <div className="form-grid">
                  {(
                    [
                      "include",
                      "exclude",
                      "folder_include",
                      "folder_exclude",
                      "selected_folders",
                    ] as const
                  ).map((k) => (
                    <label key={k}>
                      {
                        {
                          include: tr("Include files", "Dateien einschließen"),
                          exclude: tr("Exclude files", "Dateien ausschließen"),
                          folder_include: tr(
                            "Include folders",
                            "Ordner einschließen",
                          ),
                          folder_exclude: tr(
                            "Exclude folders",
                            "Ordner ausschließen",
                          ),
                          selected_folders: tr(
                            "Selected top-level folders (empty = all)",
                            "Ausgewählte Hauptordner (leer = alle)",
                          ),
                        }[k]
                      }
                      <textarea
                        rows={2}
                        placeholder={filterExamples[k]}
                        value={task.scope[k].join("\n")}
                        onChange={(e) =>
                          patch({
                            scope: {
                              ...task.scope,
                              [k]: lines(e.target.value),
                            },
                          })
                        }
                      />
                    </label>
                  ))}
                </div>
                <div className="checks">
                  {(
                    [
                      "exclude_root_files",
                      "skip_hidden",
                      "skip_system",
                    ] as const
                  ).map((k) => (
                    <label key={k}>
                      <input
                        type="checkbox"
                        checked={task.scope[k]}
                        onChange={(e) =>
                          patch({
                            scope: { ...task.scope, [k]: e.target.checked },
                          })
                        }
                      />
                      {
                        {
                          exclude_root_files: tr(
                            "Exclude root files",
                            "Dateien im Stammordner ausschließen",
                          ),
                          skip_hidden: tr(
                            "Skip hidden",
                            "Versteckte überspringen",
                          ),
                          skip_system: tr(
                            "Skip system",
                            "Systemdateien überspringen",
                          ),
                        }[k]
                      }
                    </label>
                  ))}
                </div>
              </section>
              <section className="panel">
                <h2>{tr("Automation", "Automatik")}</h2>
                <div className="toolbar">
                  <label>
                    {tr(
                      "Test relative path against these filters",
                      "Relativen Pfad mit diesen Filtern prüfen",
                    )}
                    <input
                      value={samplePath}
                      onChange={(e) => setSamplePath(e.target.value)}
                    />
                  </label>
                  <Button
                    variant="outline"
                    onClick={() =>
                      void perform(async () =>
                        setSampleResult(
                          (await invoke<boolean>("pattern_test", {
                            scope: task.scope,
                            path: samplePath,
                            directory: false,
                          }))
                            ? tr("Included", "Eingeschlossen")
                            : tr("Excluded", "Ausgeschlossen"),
                        ),
                      )
                    }
                  >
                    {tr("Test filters", "Filter prüfen")}
                  </Button>
                  <span role="status">{sampleResult}</span>
                </div>
                <p>
                  {tr(
                    "Only enable monitoring after reviewing a successful first run. Removals require manual review unless proven baseline deletion is explicitly enabled below.",
                    "Überwachung erst nach geprüftem Erstlauf aktivieren. Entfernungen erfordern manuelle Prüfung, sofern nachgewiesene Ausgangszustands-Entfernung unten nicht ausdrücklich freigegeben ist.",
                  )}
                </p>
                <div className="checks">
                  <label>
                    <input
                      type="checkbox"
                      checked={task.monitor}
                      onChange={(e) => patch({ monitor: e.target.checked })}
                    />
                    {tr("Watch both folders", "Beide Ordner überwachen")}
                  </label>
                </div>
                <div className="form-grid">
                  <label>
                    {tr("Debounce (seconds)", "Wartezeit (Sekunden)")}
                    <input
                      type="number"
                      min={1}
                      max={3600}
                      value={task.debounce_secs}
                      onChange={(e) =>
                        patch({ debounce_secs: Number(e.target.value) })
                      }
                    />
                  </label>
                  <label>
                    {tr(
                      "Bandwidth limit (KiB/s; 0 = unlimited)",
                      "Bandbreite (KiB/s; 0 = unbegrenzt)",
                    )}
                    <input
                      type="number"
                      min={0}
                      value={task.bandwidth_kib}
                      onChange={(e) =>
                        patch({ bandwidth_kib: Number(e.target.value) })
                      }
                    />
                  </label>
                  <label>
                    {tr(
                      "Parallel transfers (1–4)",
                      "Parallele Übertragungen (1–4)",
                    )}
                    <input
                      type="number"
                      min={1}
                      max={4}
                      value={task.transfer_concurrency}
                      onChange={(e) =>
                        patch({ transfer_concurrency: Number(e.target.value) })
                      }
                    />
                  </label>
                </div>
                <p className="hint">
                  {tr(
                    "Default: one file at a time. Parallel transfers share the total bandwidth limit. Folders and removals run in order; task runs remain serialized. File progress shows one active transfer.",
                    "Standard: eine Datei zur Zeit. Parallele Übertragungen teilen die gesamte Bandbreite. Ordner und Entfernungen laufen geordnet; Aufgabenläufe bleiben nacheinander. Der Dateifortschritt zeigt eine aktive Übertragung.",
                  )}
                </p>
              </section>
              <section className="panel">
                <h2>{tr("Comparison & conflicts", "Vergleich & Konflikte")}</h2>
                <div className="form-grid">
                  <label>
                    {tr("Content comparison", "Inhaltsvergleich")}
                    <select
                      value={task.comparison}
                      onChange={(e) =>
                        patch({
                          comparison: e.target.value as Task["comparison"],
                        })
                      }
                    >
                      <option value="Incremental">
                        {tr(
                          "Incremental content verification",
                          "Inhalte inkrementell prüfen",
                        )}
                      </option>
                      <option value="FullContent">
                        {tr(
                          "Full content verification on every trigger",
                          "Alle Inhalte bei jedem Auslöser prüfen",
                        )}
                      </option>
                    </select>
                  </label>
                  <label>
                    {tr("Conflict handling", "Konfliktbehandlung")}
                    <select
                      value={task.conflict_policy}
                      onChange={(e) =>
                        patch({
                          conflict_policy: e.target
                            .value as Task["conflict_policy"],
                        })
                      }
                    >
                      <option value="ContinueIndependent">
                        {tr(
                          "Preserve conflicts; continue independent transfers",
                          "Konflikte erhalten; unabhängige Übertragungen fortsetzen",
                        )}
                      </option>
                      <option value="RequireChoices">
                        {tr(
                          "Hold transfers until every conflict has a choice",
                          "Übertragungen bis zur Entscheidung für jeden Konflikt anhalten",
                        )}
                      </option>
                    </select>
                  </label>
                </div>
                <p>
                  {tr(
                    "Both comparison modes verify file contents. Incremental uses change notifications with recovery scans; Full verifies both entire trees on every trigger and may be slower. Manual Preview always verifies both trees.",
                    "Beide Vergleichsarten prüfen Dateiinhalte. Inkrementell nutzt Änderungsmeldungen mit Wiederherstellungsabgleichen; vollständig prüft beide Ordner bei jedem Auslöser und kann langsamer sein. Manuelle Vorschau prüft immer beide Ordner.",
                  )}
                </p>
                <p>
                  {tr(
                    "Holding transfers changes no files until each conflict has an explicit copy or Ignore choice. Ignore keeps originals and conflict attention; it is not a resolution. Keep originals leaves the run held.",
                    "Angehaltene Übertragungen ändern keine Dateien, bis jeder Konflikt eine ausdrückliche Kopier- oder Überspringen-Auswahl hat. Überspringen erhält Originale und Konflikthinweis; es löst den Konflikt nicht. Originale behalten hält den Lauf weiter an.",
                  )}
                </p>
              </section>
              <section className="panel">
                <h2>{tr("Propagate deletions", "Löschungen übernehmen")}</h2>
                <p>
                  {tr(
                    "Propagates proven deletions after a successful manual baseline. If you delete an unchanged file on one side, automation can remove its counterpart. Both per-run limits apply; offline folders and conflicts never count as deletion proof.",
                    "Übernimmt nachgewiesene Löschungen nach einem erfolgreichen manuellen Erstabgleich: Löschst du eine unveränderte Datei auf einer Seite, darf die Automatik ihr Gegenstück entfernen. Beide Grenzen gelten je Lauf. Offline-Ordner und Konflikte sind kein Löschbeweis.",
                  )}
                </p>
                <p>
                  {tr(
                    "Removed/replaced file contents are protected here, separately for this task. Restore them from Details → Protected versions. These are recovery copies, not a third synchronized folder.",
                    "Entfernte/ersetzte Dateiinhalte werden hier je Aufgabe gesichert. Wiederherstellen unter Details → Geschützte Versionen. Dies sind Sicherungskopien, kein dritter Sync-Ordner.",
                  )}
                </p>
                {storageLocation(task)}
                <small>
                  {tr(
                    "Older flat backup files remain available through the recovery catalog. Retention applies: matching files keep the configured number of versions, others one; empty directories store no file content.",
                    "Ältere Sicherungen ohne Aufgaben-Unterordner bleiben über die Wiederherstellung verfügbar. Aufbewahrung: passende Dateien behalten die eingestellte Versionszahl, andere eine; leere Ordner enthalten keine zu sichernden Dateiinhalte.",
                  )}
                </small>
                <label className="check-row">
                  <input
                    type="checkbox"
                    checked={task.deletion_policy.enabled}
                    disabled={
                      busy || (!deletionReady && !task.deletion_policy.enabled)
                    }
                    onChange={(e) =>
                      patch({
                        deletion_policy: {
                          ...task.deletion_policy,
                          enabled: e.target.checked,
                        },
                      })
                    }
                  />
                  {tr(
                    "Propagate proven deletions automatically",
                    "Löschungen automatisch übernehmen",
                  )}
                </label>
                <div className="form-grid">
                  <label>
                    {tr(
                      "Maximum removed entries per run",
                      "Maximal entfernte Einträge je Lauf",
                    )}
                    <input
                      type="number"
                      min={1}
                      max={100000}
                      disabled={!task.deletion_policy.enabled}
                      value={task.deletion_policy.max_entries}
                      onChange={(e) =>
                        patch({
                          deletion_policy: {
                            ...task.deletion_policy,
                            max_entries: Number(e.target.value),
                          },
                        })
                      }
                    />
                  </label>
                  <label>
                    {tr(
                      "Maximum percentage of indexed baseline",
                      "Maximaler Anteil am indizierten Ausgangszustand",
                    )}
                    <input
                      type="number"
                      min={1}
                      max={100}
                      disabled={!task.deletion_policy.enabled}
                      value={task.deletion_policy.max_percent}
                      onChange={(e) =>
                        patch({
                          deletion_policy: {
                            ...task.deletion_policy,
                            max_percent: Number(e.target.value),
                          },
                        })
                      }
                    />
                  </label>
                </div>
                <small>
                  {tr(
                    "Protected recovery copies remain outside both roots. Unproven deletions, Move and exceeded limits wait for review. Enabling this policy does not itself enable monitoring or schedules.",
                    "Geschützte Wiederherstellungskopien bleiben außerhalb beider Ordner. Unbelegte Entfernungen, Verschieben und überschrittene Grenzen warten auf Prüfung. Diese Freigabe aktiviert weder Überwachung noch Zeitpläne.",
                  )}
                </small>
              </section>
              <section className="panel">
                <h2>{tr("Schedules & startup", "Zeitplan & Start")}</h2>
                <label className="check-row">
                  <input
                    type="checkbox"
                    checked={task.login}
                    onChange={(e) => patch({ login: e.target.checked })}
                  />
                  {tr(
                    "Run when FolderSync starts (including login startup)",
                    "Beim Start von FolderSync ausführen (auch bei Anmeldung)",
                  )}
                </label>
                <div className="form-grid">
                  <label>
                    {tr("Schedule", "Zeitplan")}
                    <select
                      value={task.schedule.kind}
                      onChange={(e) =>
                        patch({
                          schedule: { ...task.schedule, kind: e.target.value },
                        })
                      }
                    >
                      <option value="">{tr("Disabled", "Aus")}</option>
                      <option value="periodic">
                        {tr("Periodic", "Periodisch")}
                      </option>
                      <option value="daily">{tr("Daily", "Täglich")}</option>
                      <option value="weekly">
                        {tr("Weekly", "Wöchentlich")}
                      </option>
                      <option value="monthly">
                        {tr("Monthly", "Monatlich")}
                      </option>
                    </select>
                  </label>
                  {task.schedule.kind === "periodic" ? (
                    <label>
                      {tr("Interval (minutes)", "Intervall (Minuten)")}
                      <input
                        type="number"
                        min={1}
                        value={task.schedule.interval_minutes}
                        onChange={(e) =>
                          patch({
                            schedule: {
                              ...task.schedule,
                              interval_minutes: Number(e.target.value),
                            },
                          })
                        }
                      />
                    </label>
                  ) : (
                    task.schedule.kind && (
                      <label>
                        {tr("Local time", "Ortszeit")}
                        <input
                          type="time"
                          value={`${String(task.schedule.hour).padStart(2, "0")}:${String(task.schedule.minute).padStart(2, "0")}`}
                          onChange={(e) => {
                            const [hour, minute] = e.target.value
                              .split(":")
                              .map(Number);
                            patch({
                              schedule: { ...task.schedule, hour, minute },
                            });
                          }}
                        />
                      </label>
                    )
                  )}
                  {task.schedule.kind === "monthly" && (
                    <label>
                      {tr(
                        "Day of month (missing days skipped)",
                        "Monatstag (fehlende Tage überspringen)",
                      )}
                      <input
                        type="number"
                        min={1}
                        max={31}
                        value={task.schedule.month_day}
                        onChange={(e) =>
                          patch({
                            schedule: {
                              ...task.schedule,
                              month_day: Number(e.target.value),
                            },
                          })
                        }
                      />
                    </label>
                  )}
                  <label>
                    {tr("Retry attempts", "Wiederholungsversuche")}
                    <input
                      type="number"
                      min={0}
                      max={10}
                      value={task.retries}
                      onChange={(e) =>
                        patch({ retries: Number(e.target.value) })
                      }
                    />
                  </label>
                </div>
                {task.schedule.kind === "weekly" && (
                  <div className="checks">
                    {(lang === "de"
                      ? ["So", "Mo", "Di", "Mi", "Do", "Fr", "Sa"]
                      : ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"]
                    ).map((day, i) => (
                      <label key={day}>
                        <input
                          type="checkbox"
                          checked={task.schedule.weekdays.includes(i)}
                          onChange={(e) =>
                            patch({
                              schedule: {
                                ...task.schedule,
                                weekdays: e.target.checked
                                  ? [...task.schedule.weekdays, i]
                                  : task.schedule.weekdays.filter(
                                      (n) => n !== i,
                                    ),
                              },
                            })
                          }
                        />
                        {day}
                      </label>
                    ))}
                  </div>
                )}
                <p className="hint">
                  {tr(
                    "After sleep, one missed occurrence runs. Repeated DST times run once; missing times advance.",
                    "Nach Ruhezustand wird ein verpasster Lauf nachgeholt. Doppelte Zeitumstellungen laufen einmal; fehlende Zeiten werden vorgerückt.",
                  )}
                </p>
              </section>
              <section className="panel">
                <h2>{tr("Recovery", "Wiederherstellung")}</h2>
                <p>
                  {tr(
                    "Replaced and deleted files are protected outside both folders.",
                    "Ersetzte und gelöschte Dateien werden außerhalb beider Ordner geschützt.",
                  )}
                </p>
                {storageLocation(task)}
                <div className="form-grid">
                  <label>
                    {tr(
                      "History patterns (empty = all files)",
                      "Verlaufsmuster (leer = alle Dateien)",
                    )}
                    <textarea
                      value={task.history_patterns.join("\n")}
                      onChange={(e) =>
                        patch({ history_patterns: lines(e.target.value) })
                      }
                    />
                  </label>
                  <label>
                    {tr(
                      "Versions per matching file",
                      "Versionen je passender Datei",
                    )}
                    <input
                      type="number"
                      min={1}
                      max={1000}
                      value={task.history_cap}
                      onChange={(e) =>
                        patch({ history_cap: Number(e.target.value) })
                      }
                    />
                  </label>
                </div>
                <p className="hint">
                  {tr(
                    "After successful runs this cap applies to matching files. Other files retain one recovery version.",
                    "Nach erfolgreichen Läufen gilt diese Anzahl für passende Dateien. Andere Dateien behalten eine Wiederherstellungsversion.",
                  )}
                </p>
              </section>
            </>
          )}
          {page === "preview" && plan && summary && (
            <>
              <div className="page-heading">
                <div>
                  <button
                    className="back"
                    onClick={() => navigate("dashboard")}
                  >
                    <ArrowLeft size={16} />
                    {tr("All tasks", "Alle Aufgaben")}
                  </button>
                  <div className="eyebrow">
                    {["Preview", "Review required"].includes(plan.status)
                      ? tr(
                          "PREVIEW — NO FILE CHANGES",
                          "VORSCHAU — KEINE DATEIÄNDERUNGEN",
                        )
                      : statusLabel(plan.status, lang)}
                  </div>
                  <h1>{plan.task.name}</h1>
                  <p>
                    {date(plan.created_at)} · {modeLabels[plan.task.mode]}
                  </p>
                </div>
                {!archivedPlan &&
                  ["Preview", "Review required"].includes(plan.status) && (
                    <Button
                      disabled={
                        busy ||
                        needsConflictChoices ||
                        failedPlanId === plan.id ||
                        (summary.deletes > 0 && !approved)
                      }
                      onClick={() =>
                        setConfirm({
                          title: tr(
                            "Apply reviewed operations?",
                            "Geprüfte Aktionen anwenden?",
                          ),
                          body: tr(
                            `${summary.copies} transfers, ${summary.deletes} removals, ${summary.conflicts} unresolved conflicts. Existing versions will be protected. This changes files in the selected folders.`,
                            `${summary.copies} Übertragungen, ${summary.deletes} Entfernungen, ${summary.conflicts} offene Konflikte. Bestehende Versionen werden geschützt. Dateien in den ausgewählten Ordnern werden geändert.`,
                          ),
                          action: async () => {
                            await perform(async () => {
                              setFailedPlanId(plan.id);
                              const result = await invoke<ApplyOutcome>(
                                "apply",
                                {
                                  id: plan.id,
                                  destructive: approved || summary.explicit,
                                },
                              );
                              setPlan(result.plan);
                              await refresh();
                              if (result.plan.status === "Cancelled") {
                                setMessage(
                                  "Run cancelled. Completed operations remain committed. Create a fresh preview to continue.",
                                );
                                return;
                              }
                              if (result.error) throw new Error(result.error);
                              setFailedPlanId("");
                              setMessage(
                                "Run finished. See the actual result and protected versions.",
                              );
                            });
                          },
                        })
                      }
                    >
                      {tr("Apply reviewed plan", "Geprüften Plan anwenden")}
                    </Button>
                  )}
              </div>
              {needsConflictChoices && (
                <p className="alert warning" role="status">
                  {tr(
                    "This task holds all transfers. Choose a copy direction or Ignore for every conflict before applying.",
                    "Diese Aufgabe hält alle Übertragungen an. Vor dem Anwenden für jeden Konflikt eine Kopierrichtung oder Überspringen wählen.",
                  )}
                </p>
              )}
              {(summary.ignored || 0) > 0 && (
                <p className="alert warning">
                  {tr(
                    `Ignored this run: ${summary.ignored}. Conflicts remain unresolved.`,
                    `In diesem Lauf übersprungen: ${summary.ignored}. Konflikte bleiben offen.`,
                  )}
                </p>
              )}
              <div className="stats">
                <div>
                  <b>{summary.copies}</b>
                  <span>{tr("Transfers", "Übertragungen")}</span>
                </div>
                <div>
                  <b>{summary.deletes}</b>
                  <span>{tr("Removals", "Entfernungen")}</span>
                </div>
                <div>
                  <b>{summary.conflicts}</b>
                  <span>{tr("Pending conflicts", "Offene Konflikte")}</span>
                </div>
                <div>
                  <b>{summary.unchanged}</b>
                  <span>{tr("Unchanged", "Unverändert")}</span>
                </div>
              </div>
              {direction && (
                <p className="hint" role="status">
                  A → B: {direction.to_b} · B → A: {direction.to_a} ·{" "}
                  {tr("Removals from A", "Entfernungen aus A")}:{" "}
                  {direction.remove_a} ·{" "}
                  {tr("Removals from B", "Entfernungen aus B")}:{" "}
                  {direction.remove_b}
                </p>
              )}
              <div className="panel">
                <PathPair a={plan.task.a} b={plan.task.b} lang={lang} />
                <small>
                  {tr("Run ID", "Lauf-ID")}: {plan.id}
                </small>
              </div>
              {plan.deletion_review && (
                <section
                  className="panel"
                  aria-label={tr(
                    "Automatic deletion decision",
                    "Entscheidung zur automatischen Entfernung",
                  )}
                >
                  <h3>
                    {tr(
                      "Automatic deletion policy",
                      "Richtlinie für automatische Entfernung",
                    )}
                  </h3>
                  <p>
                    {deletionReasonLabel(plan.deletion_review.reason, lang)}
                  </p>
                  <p>
                    {tr(
                      `${plan.deletion_review.entries} removals / ${plan.deletion_review.baseline_entries} indexed baseline entries. Limits: ${plan.deletion_review.max_entries} entries and ${plan.deletion_review.max_percent}% per run.`,
                      `${plan.deletion_review.entries} Entfernungen / ${plan.deletion_review.baseline_entries} indizierte Ausgangseinträge. Grenzen: ${plan.deletion_review.max_entries} Einträge und ${plan.deletion_review.max_percent}% je Lauf.`,
                    )}
                  </p>
                  {plan.deletion_review.baseline_run && (
                    <small>
                      {tr("Manual baseline run", "Manueller Ausgangslauf")}:{" "}
                      {plan.deletion_review.baseline_run}
                    </small>
                  )}
                </section>
              )}
              {!archivedPlan &&
                summary.deletes > 0 &&
                ["Preview", "Review required"].includes(plan.status) && (
                  <label className="alert warning">
                    <input
                      type="checkbox"
                      checked={approved}
                      onChange={(e) => setApproved(e.target.checked)}
                    />
                    {tr(
                      "I reviewed every removal, including Move source removals, and approve this exact plan.",
                      "Ich habe jede Entfernung geprüft, einschließlich der Quelldateien bei Verschieben, und genehmige diesen Plan.",
                    )}
                  </label>
                )}
              <div className="toolbar">
                <input
                  aria-label={tr("Filter operations", "Aktionen filtern")}
                  placeholder={tr("Filter paths…", "Pfade filtern…")}
                  value={filter}
                  onChange={(e) => {
                    setFilter(e.target.value);
                    setOperationPage(0);
                  }}
                />
                <select
                  aria-label={tr("Sort operations", "Aktionen sortieren")}
                  value={sort}
                  onChange={(e) => {
                    setSort(e.target.value);
                    setOperationPage(0);
                  }}
                >
                  <option value="path">{tr("Path", "Pfad")}</option>
                  <option value="action">{tr("Action", "Aktion")}</option>
                </select>
                <label>
                  <input
                    type="checkbox"
                    checked={showUnchanged}
                    onChange={(e) => {
                      setShowUnchanged(e.target.checked);
                      setOperationPage(0);
                    }}
                  />
                  {tr("Show unchanged", "Unveränderte anzeigen")}
                </label>
                {!archivedPlan && (
                  <Button
                    variant="outline"
                    onClick={() => void preview(plan.task)}
                    disabled={busy}
                  >
                    {tr("Fresh preview", "Neue Vorschau")}
                  </Button>
                )}
              </div>
              {!rows && (
                <p role="status">
                  {tr("Loading operations…", "Aktionen werden geladen…")}
                </p>
              )}
              <div className="table-wrap">
                <table>
                  <caption>
                    {tr(
                      "Metadata describes the reviewed inputs. Operation status records execution. Use Fresh preview to read current files.",
                      "Metadaten beschreiben die geprüften Eingaben. Der Aktionsstatus zeigt die Ausführung. Neue Vorschau liest die aktuellen Dateien erneut.",
                    )}
                  </caption>
                  <thead>
                    <tr>
                      <th>{tr("Relative path", "Relativer Pfad")}</th>
                      <th>{tr("Action", "Aktion")}</th>
                      <th>{tr("Folder A", "Ordner A")}</th>
                      <th>{tr("Folder B", "Ordner B")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {rows?.operations.map((o) => (
                      <tr key={o.path}>
                        <td>
                          <strong>{o.path}</strong>
                          <small>{reasonLabel(o.reason, lang)}</small>
                        </td>
                        <td>
                          <span className={`op ${o.action}`}>
                            {operationLabel(o, lang)}
                          </span>
                          {o.kind &&
                            [
                              "CopyToA",
                              "CopyToB",
                              "MoveToB",
                              "DeleteA",
                              "DeleteB",
                            ].includes(o.action) && (
                              <small>{actionLabel(o.action, lang)}</small>
                            )}
                          {!["Preview", "Review required"].includes(
                            plan.status,
                          ) && (
                            <small>
                              {statusLabel(o.status || plan.status, lang)}
                            </small>
                          )}
                          {!archivedPlan &&
                            (o.action === "Conflict" ||
                              o.action === "Ignore") &&
                            ["Preview", "Review required"].includes(
                              plan.status,
                            ) && (
                              <select
                                aria-label={tr(
                                  `Resolve ${o.path}`,
                                  `${o.path} auflösen`,
                                )}
                                value={o.action === "Ignore" ? "ignore" : ""}
                                onChange={(e) =>
                                  void perform(async () => {
                                    setPlan(
                                      await invoke<Plan>("resolve", {
                                        id: plan.id,
                                        path: o.path,
                                        choice: e.target.value,
                                      }),
                                    );
                                  })
                                }
                              >
                                <option value="">
                                  {tr(
                                    "Choose an action (keep originals)",
                                    "Aktion wählen (Originale behalten)",
                                  )}
                                </option>
                                <option value="keep">
                                  {tr(
                                    "Keep both / unresolved",
                                    "Beide behalten / offen",
                                  )}
                                </option>
                                <option value="ignore">
                                  {tr(
                                    "Ignore for this run (keep conflict)",
                                    "Diesen Lauf überspringen (Konflikt behalten)",
                                  )}
                                </option>
                                {o.a && (
                                  <option value="a">
                                    {tr(
                                      "Choose A → B (preview)",
                                      "A → B wählen (Vorschau)",
                                    )}
                                  </option>
                                )}
                                {o.b && (
                                  <option value="b">
                                    {tr(
                                      "Choose B → A (preview)",
                                      "B → A wählen (Vorschau)",
                                    )}
                                  </option>
                                )}
                              </select>
                            )}
                        </td>
                        <td title={o.a?.digest}>
                          {meta(o.a)}
                          {o.a && (
                            <small>
                              {tr("Modified", "Geändert")}:{" "}
                              {modifiedLabel(o.a, lang)}
                            </small>
                          )}
                        </td>
                        <td title={o.b?.digest}>
                          {meta(o.b)}
                          {o.b && (
                            <small>
                              {tr("Modified", "Geändert")}:{" "}
                              {modifiedLabel(o.b, lang)}
                            </small>
                          )}
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
              <div className="toolbar">
                <Button
                  variant="outline"
                  disabled={operationPage === 0}
                  onClick={() => setOperationPage(operationPage - 1)}
                >
                  {tr("Previous", "Zurück")}
                </Button>
                <span>
                  {tr("Page", "Seite")} {operationPage + 1} · 200{" "}
                  {tr("rows per page", "Zeilen je Seite")}
                </span>
                <Button
                  variant="outline"
                  disabled={(operationPage + 1) * 200 >= (rows?.total ?? 0)}
                  onClick={() => setOperationPage(operationPage + 1)}
                >
                  {tr("Next", "Weiter")}
                </Button>
              </div>
              <p className="hint">
                {plan.metrics.scanned_entries}{" "}
                {tr("entries examined", "Einträge geprüft")} ·{" "}
                {bytes(plan.metrics.hashed_bytes)} {tr("hashed", "gehasht")} ·{" "}
                {bytes(plan.metrics.copied_bytes)} {tr("copied", "kopiert")} ·{" "}
                {plan.metrics.reconciliations}{" "}
                {tr("reconciliations", "Vollabgleiche")}
                {(plan.metrics.peak_workers ?? 0) > 1 && (
                  <>
                    {" "}
                    · {plan.metrics.peak_workers}{" "}
                    {tr(
                      "peak concurrent transfers",
                      "maximal gleichzeitige Übertragungen",
                    )}
                  </>
                )}
              </p>
            </>
          )}
          {page === "recovery" && (
            <>
              <div className="page-heading">
                <div>
                  <h1>
                    {tr(
                      "Deleted-task recovery",
                      "Wiederherstellung gelöschter Aufgaben",
                    )}
                  </h1>
                  <p>
                    {tr(
                      "Protected versions remain available after deleting a task. Restoring does not restart synchronization or restore its baseline.",
                      "Geschützte Versionen bleiben nach dem Löschen einer Aufgabe verfügbar. Wiederherstellung aktiviert weder Synchronisation noch Ausgangszustand erneut.",
                    )}
                  </p>
                </div>
              </div>
              {recoveryTasks.length === 0 ? (
                <section className="panel">
                  <p>
                    {tr(
                      "No deleted tasks with protected versions.",
                      "Keine gelöschten Aufgaben mit geschützten Versionen.",
                    )}
                  </p>
                </section>
              ) : (
                recoveryTasks.map((r) => (
                  <section className="panel" key={r.task.id}>
                    <h2>{r.task.name}</h2>
                    <p>
                      {r.versions}{" "}
                      {tr("protected versions", "geschützte Versionen")} ·{" "}
                      {date(r.archived_at)}
                    </p>
                    <PathPair a={r.task.a} b={r.task.b} lang={lang} />
                    <Button
                      variant="outline"
                      aria-label={tr(
                        `Open recovery for ${r.task.name}`,
                        `Wiederherstellung für ${r.task.name} öffnen`,
                      )}
                      onClick={() => void details(r.task, true)}
                    >
                      {tr("Open recovery", "Wiederherstellung öffnen")}
                    </Button>
                  </section>
                ))
              )}
            </>
          )}
          {page === "details" && task && (
            <>
              <div className="page-heading">
                <div>
                  <button
                    className="back"
                    onClick={() => navigate("dashboard")}
                  >
                    <ArrowLeft size={16} />
                    {tr("All tasks", "Alle Aufgaben")}
                  </button>
                  <h1>{task.name}</h1>
                  <p>
                    {tr(
                      "Run history and protected originals",
                      "Laufverlauf und geschützte Originale",
                    )}
                  </p>
                </div>
                {!archivedDetail && (
                  <Button onClick={() => void preview(task)}>
                    {tr("New preview", "Neue Vorschau")}
                  </Button>
                )}
              </div>
              {archivedDetail && (
                <section className="panel">
                  <h2>
                    {tr(
                      "Deleted task — recovery only",
                      "Gelöschte Aufgabe — nur Wiederherstellung",
                    )}
                  </h2>
                  <p>
                    {tr(
                      "The synchronization baseline and triggers were removed. Original root identities are retained only for reviewed recovery. Create a new task to synchronize again.",
                      "Sync-Ausgangszustand und Auslöser wurden entfernt. Ursprüngliche Ordneridentitäten bleiben nur für geprüfte Wiederherstellung erhalten. Für erneute Synchronisation eine neue Aufgabe erstellen.",
                    )}
                  </p>
                  <PathPair a={task.a} b={task.b} lang={lang} />
                </section>
              )}
              <section className="panel">
                <h2>
                  <FileClock size={20} />
                  {tr("Run history", "Laufverlauf")}
                </h2>
                {runs.length === 0 ? (
                  <p>{tr("No runs yet.", "Noch keine Läufe.")}</p>
                ) : (
                  runs.map((p) => (
                    <button
                      className="history-row"
                      key={p.id}
                      onClick={() => {
                        setPlan(p);
                        setPage("preview");
                        setApproved(false);
                        setOperationPage(0);
                      }}
                    >
                      <span>
                        {date(p.created_at)}
                        <small>{p.id}</small>
                      </span>
                      <span className="badge">
                        {statusLabel(p.status, lang)}
                      </span>
                      <span>
                        {p.summary.conflicts} {tr("conflicts", "Konflikte")} ·{" "}
                        {p.summary.entries} {tr("entries", "Einträge")}
                      </span>
                    </button>
                  ))
                )}
              </section>
              <section className="panel">
                <h2>
                  <ShieldCheck size={20} />
                  {tr("Protected versions", "Geschützte Versionen")}
                </h2>
                {storageLocation(task)}
                {versions.length === 0 ? (
                  <p>
                    {tr(
                      "No protected versions yet.",
                      "Noch keine geschützten Versionen.",
                    )}
                  </p>
                ) : (
                  versions.map((v) => (
                    <div className="version-row" key={v.id}>
                      <strong>
                        {v.side} / {v.path}
                      </strong>
                      <span>{date(v.created_at)}</span>
                      <code title={v.stored}>{v.digest.slice(0, 16)}</code>
                      <Button variant="outline" onClick={() => void restore(v)}>
                        {tr("Review restore", "Wiederherstellung prüfen")}
                      </Button>
                    </div>
                  ))
                )}
              </section>
              {!archivedDetail && (
                <div className="toolbar">
                  <Button
                    variant="outline"
                    onClick={() => {
                      const t = {
                        ...structuredClone(task),
                        id: crypto.randomUUID(),
                        identifier: "",
                        name: `${task.name} ${tr("(copy)", "(Kopie)")}`,
                        monitor: false,
                        login: false,
                        schedule: newTask().schedule,
                        deletion_policy: {
                          ...task.deletion_policy,
                          enabled: false,
                        },
                      };
                      edit(t);
                    }}
                  >
                    {tr("Duplicate configuration", "Konfiguration duplizieren")}
                  </Button>
                  <Button
                    variant="destructive"
                    onClick={() =>
                      setConfirm({
                        title: tr(
                          "Delete task configuration?",
                          "Aufgabenkonfiguration löschen?",
                        ),
                        body: tr(
                          "This removes the task and its synchronization baseline. Files and protected versions remain.",
                          "Die Aufgabe und ihr Sync-Ausgangszustand werden entfernt. Dateien und geschützte Versionen bleiben.",
                        ),
                        action: async () => {
                          await perform(async () => {
                            await invoke("delete_task", { id: task.id });
                            await refresh();
                            setPage("dashboard");
                          });
                        },
                      })
                    }
                  >
                    {tr("Delete task", "Aufgabe löschen")}
                  </Button>
                </div>
              )}
            </>
          )}
          {page === "settings" && (
            <>
              <div className="page-heading">
                <div>
                  <div className="eyebrow">
                    {tr("APPLICATION", "ANWENDUNG")}
                  </div>
                  <h1>{tr("Settings", "Einstellungen")}</h1>
                  <p>
                    {tr(
                      "Local preferences and portable task configuration.",
                      "Lokale Einstellungen und portable Aufgabenkonfiguration.",
                    )}
                  </p>
                </div>
              </div>
              <div
                className="settings-tabs"
                role="tablist"
                aria-label={tr("Settings sections", "Einstellungsbereiche")}
              >
                {(["general", "version"] as const).map((tab) => (
                  <button
                    key={tab}
                    type="button"
                    role="tab"
                    id={`settings-tab-${tab}`}
                    aria-controls={`settings-panel-${tab}`}
                    aria-selected={settingsTab === tab}
                    onClick={() => setSettingsTab(tab)}
                  >
                    {tab === "general" ? tr("General", "Allgemein") : "Version"}
                  </button>
                ))}
              </div>
              {settingsTab === "general" ? (
                <div
                  role="tabpanel"
                  id="settings-panel-general"
                  aria-labelledby="settings-tab-general"
                  className="settings-shell"
                >
                  <section className="panel">
                    <h2>{tr("Preferences", "Einstellungen")}</h2>
                    <label>
                      {tr("Language", "Sprache")}
                      <select
                        value={lang}
                        onChange={(e) => {
                          const value = e.target.value;
                          setLang(value);
                          void invoke("set_setting", {
                            key: "language",
                            value,
                          });
                        }}
                      >
                        <option value="en">English</option>
                        <option value="de">Deutsch</option>
                      </select>
                    </label>
                    <label className="check-row">
                      <input
                        type="checkbox"
                        checked={startup}
                        disabled={busy}
                        onChange={(e) =>
                          void perform(async () => {
                            await invoke("set_startup", {
                              enabled: e.target.checked,
                            });
                            setStartup(
                              await invoke<boolean>("startup_enabled"),
                            );
                          })
                        }
                      />
                      {tr(
                        "Start FolderSync when I sign in",
                        "FolderSync bei der Anmeldung starten",
                      )}
                    </label>
                    <label className="check-row">
                      <input
                        type="checkbox"
                        checked={notifications}
                        disabled={busy}
                        onChange={(e) => {
                          const value = e.target.checked;
                          void perform(async () => {
                            await invoke("set_setting", {
                              key: "notifications",
                              value: value ? "on" : "off",
                            });
                            setNotifications(value);
                          });
                        }}
                      />
                      {tr(
                        "Windows notifications",
                        "Windows-Benachrichtigungen",
                      )}
                    </label>
                    <div className="notification-status">
                      <p>
                        {tr("Windows delivery", "Windows-Zustellung")}:{" "}
                        <strong>
                          {notificationStatus?.windows_setting === "enabled"
                            ? tr("Enabled", "Aktiviert")
                            : notificationStatus?.windows_setting ||
                              tr("Unknown", "Unbekannt")}
                        </strong>
                      </p>
                      <p>
                        {tr(
                          "Banners and sound also depend on Windows notification settings and Do not disturb. FolderSync requests an audible notification; it cannot override Windows suppression.",
                          "Banner und Ton hängen zusätzlich von den Windows-Benachrichtigungseinstellungen und „Nicht stören“ ab. FolderSync fordert eine hörbare Benachrichtigung an; Windows kann sie unterdrücken.",
                        )}
                      </p>
                      {notificationStatus?.banners === false && (
                        <p role="status">
                          {tr(
                            "Windows has disabled banners for FolderSync. Enable banners in Windows settings.",
                            "Windows hat Banner für FolderSync deaktiviert. Banner in den Windows-Einstellungen aktivieren.",
                          )}
                        </p>
                      )}
                      {notificationStatus?.presentation &&
                        !["available", "unknown"].includes(
                          notificationStatus.presentation,
                        ) && (
                          <p>
                            {tr(
                              "Windows reports a presentation or busy state; banners can be suppressed.",
                              "Windows meldet einen Beschäftigt- oder Präsentationszustand; Banner können unterdrückt werden.",
                            )}
                          </p>
                        )}
                      <div className="toolbar">
                        <Button
                          variant="outline"
                          onClick={() =>
                            void perform(async () => {
                              await invoke("open_notification_settings");
                            })
                          }
                        >
                          {tr(
                            "Windows notification settings",
                            "Windows-Benachrichtigungseinstellungen",
                          )}
                        </Button>
                        <Button
                          variant="outline"
                          disabled={busy || !notifications}
                          onClick={() =>
                            void perform(async () => {
                              await invoke("send_test_notification");
                              setNotificationStatus(
                                await invoke<typeof notificationStatus>(
                                  "notification_status",
                                ),
                              );
                              setNotificationFeedback(
                                tr(
                                  "Test request sent to Windows. Check the notification center if no banner appears.",
                                  "Testbenachrichtigung gesendet. Falls kein Banner erscheint, das Windows-Benachrichtigungscenter prüfen.",
                                ),
                              );
                            })
                          }
                        >
                          {tr(
                            "Send test notification",
                            "Testbenachrichtigung senden",
                          )}
                        </Button>
                      </div>
                    </div>
                    {notificationFeedback && (
                      <p role="status" className="notification-feedback">
                        {notificationFeedback}
                      </p>
                    )}
                    <label>
                      {tr(
                        "Local operation logging",
                        "Lokales Aktionsprotokoll",
                      )}
                      <select
                        value={logLevel}
                        onChange={(e) => {
                          const value = e.target.value;
                          void perform(async () => {
                            await invoke("set_setting", {
                              key: "log_level",
                              value,
                            });
                            setLogLevel(value);
                          });
                        }}
                      >
                        <option value="off">{tr("Disabled", "Aus")}</option>
                        <option value="basic">
                          {tr(
                            "Basic (no file paths)",
                            "Einfach (ohne Dateipfade)",
                          )}
                        </option>
                        <option value="detailed">
                          {tr(
                            "Detailed (includes paths)",
                            "Detailliert (mit Pfaden)",
                          )}
                        </option>
                      </select>
                    </label>
                    <label>
                      {tr(
                        "When I close the window",
                        "Beim Schließen des Fensters",
                      )}
                      <select
                        aria-label={tr(
                          "When I close the window",
                          "Beim Schließen des Fensters",
                        )}
                        value={closeBehavior}
                        disabled={busy}
                        onChange={(e) => {
                          const value = e.target.value;
                          void perform(async () => {
                            await invoke("set_setting", {
                              key: "close_behavior",
                              value,
                            });
                            setCloseBehavior(value);
                          });
                        }}
                      >
                        <option value="tray">
                          {tr(
                            "Keep running in the tray",
                            "Im Infobereich weiterlaufen",
                          )}
                        </option>
                        <option value="quit">
                          {tr(
                            "Stop safely and quit",
                            "Sicher stoppen und beenden",
                          )}
                        </option>
                      </select>
                    </label>
                    <p>
                      {closeBehavior === "quit"
                        ? tr(
                            "Closing the window stops active work safely and quits FolderSync.",
                            "Beim Schließen wird aktive Arbeit sicher gestoppt und FolderSync beendet.",
                          )
                        : tr(
                            "Closing the window keeps the process in the tray. Choose Quit in the tray to stop it.",
                            "Beim Schließen bleibt der Prozess im Infobereich. Beenden im Tray stoppt die Anwendung.",
                          )}
                    </p>
                  </section>
                  <section className="panel">
                    <h2>{tr("Import & export", "Import & Export")}</h2>
                    <p>
                      {tr(
                        "FolderSync versioned JSON only. Imported automatic triggers are disabled. Duplicate task IDs receive a new automatic ID.",
                        "Nur versioniertes FolderSync-JSON. Importierte automatische Auslöser werden deaktiviert. Doppelte Aufgaben-IDs erhalten eine neue automatische ID.",
                      )}
                    </p>
                    <div className="toolbar">
                      <Button
                        variant="outline"
                        onClick={() => void exportTasks()}
                      >
                        {tr("Export tasks…", "Aufgaben exportieren…")}
                      </Button>
                      <Button
                        variant="outline"
                        onClick={() => void importTasks()}
                      >
                        {tr("Import tasks…", "Aufgaben importieren…")}
                      </Button>
                    </div>
                  </section>
                  <section className="panel">
                    <h2>{tr("Local data", "Lokale Daten")}</h2>
                    {storageLocation()}
                    <p>
                      {tr(
                        "SQLite state and protected versions are stored here. No telemetry or cloud services are used.",
                        "SQLite-Zustand und geschützte Versionen liegen hier. Keine Telemetrie oder Cloud-Dienste.",
                      )}
                    </p>
                    <div className="toolbar local-data-actions">
                      <Button
                        variant="outline"
                        onClick={() =>
                          void perform(async () =>
                            setLogText(await invoke<string>("logs")),
                          )
                        }
                      >
                        {tr(
                          "View recent local log",
                          "Letztes lokales Protokoll anzeigen",
                        )}
                      </Button>
                      {logText !== null && (
                        <pre className="local-log">
                          {logText ||
                            tr(
                              "No operation log yet. Enable logging, then apply a plan.",
                              "Noch kein Protokoll. Protokollierung aktivieren und einen Plan anwenden.",
                            )}
                        </pre>
                      )}
                      <Button
                        variant="outline"
                        onClick={() => void invoke("quit")}
                      >
                        {tr("Quit FolderSync", "FolderSync beenden")}
                      </Button>
                    </div>
                  </section>
                </div>
              ) : (
                <section
                  className="panel settings-shell"
                  role="tabpanel"
                  id="settings-panel-version"
                  aria-labelledby="settings-tab-version"
                >
                  <h2>FolderSync {packageInfo.version}</h2>
                  <p>
                    {tr(
                      "Installed application version. Automatic updates are not configured yet; this section is reserved for future update integration.",
                      "Installierte Anwendungsversion. Automatische Updates sind noch nicht eingerichtet; dieser Bereich ist für die spätere Integration vorgesehen.",
                    )}
                  </p>
                  <p>
                    {tr(
                      "Windows x64 · Rust / Tauri 2 · unsigned build",
                      "Windows x64 · Rust / Tauri 2 · unsignierter Build",
                    )}
                  </p>
                </section>
              )}
            </>
          )}
        </main>
        {page !== "settings" && (
          <footer>
            <span>
              <CheckCircle2 size={13} />
              {tr(
                "Verified transfers · protected originals",
                "Geprüfte Übertragungen · geschützte Originale",
              )}
            </span>
            <span>FolderSync {packageInfo.version}</span>
          </footer>
        )}
      </div>
      {showFilterHelp && (
        <FilterHelp lang={lang} onClose={() => setShowFilterHelp(false)} />
      )}
      {confirm && (
        <Confirmation
          title={confirm.title}
          body={confirm.body}
          cancelLabel={tr("Cancel", "Abbrechen")}
          confirmLabel={tr("Confirm", "Bestätigen")}
          onCancel={() => setConfirm(null)}
          onConfirm={() => {
            const action = confirm.action;
            setConfirm(null);
            void action();
          }}
        />
      )}
    </div>
  );
}
