import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Button } from "./ui/button";

export type UpdateStatus = {
  current_version: string;
  available: { version: string; notes: string } | null;
  checking: boolean;
  installing: boolean;
  downloaded: number;
  total: number | null;
  checked: boolean;
  error: string | null;
};

export function useAppUpdates() {
  const [status, setStatus] = useState<UpdateStatus | null>(null);
  const [error, setError] = useState("");
  const [checking, setChecking] = useState(false);
  const [installing, setInstalling] = useState(false);
  useEffect(() => {
    let mounted = true;
    let eventSeen = false;
    const listener = listen<UpdateStatus>("update-status", ({ payload }) => {
      eventSeen = true;
      if (mounted) setStatus(payload);
    });
    void listener
      .then(() => invoke<UpdateStatus>("update_status"))
      .then((value) => {
        if (mounted && !eventSeen) setStatus(value);
      })
      .catch(() => {});
    return () => {
      mounted = false;
      void listener.then((dispose) => dispose());
    };
  }, []);
  async function check() {
    setChecking(true);
    setError("");
    try {
      setStatus(await invoke<UpdateStatus>("check_update"));
    } catch (error) {
      setError(String(error));
    } finally {
      setChecking(false);
    }
  }
  async function install() {
    if (!status?.available || installing) return;
    setInstalling(true);
    setError("");
    try {
      await invoke("install_update", { version: status.available.version });
    } catch (error) {
      setError(String(error));
    } finally {
      setInstalling(false);
    }
  }
  return {
    status,
    error: error || status?.error,
    checking: checking || !!status?.checking,
    installing: installing || !!status?.installing,
    check,
    install,
  };
}

export function UpdatePanel({
  updates,
  lang,
  active,
}: {
  updates: ReturnType<typeof useAppUpdates>;
  lang: string;
  active: boolean;
}) {
  const tr = (en: string, de: string) => (lang === "de" ? de : en);
  const offer = updates.status?.available;
  return (
    <div className="app-updates">
      <p>
        {tr(
          "Updates are checked on GitHub at startup and every six hours. Download and installation start only when you choose Install now.",
          "Updates werden beim Start und alle sechs Stunden auf GitHub geprüft. Download und Installation starten erst mit Jetzt installieren.",
        )}
      </p>
      <Button
        variant="outline"
        disabled={updates.checking || updates.installing}
        onClick={() => void updates.check()}
      >
        {updates.checking
          ? tr("Checking…", "Wird geprüft…")
          : tr("Check for updates", "Nach Updates suchen")}
      </Button>
      {updates.error && <p role="alert">{updates.error}</p>}
      {!offer &&
        updates.status?.checked &&
        !updates.error &&
        !updates.checking && (
          <p role="status">
            {tr("You are up to date.", "Die Anwendung ist aktuell.")}
          </p>
        )}
      {offer && (
        <section className="update-offer">
          <h3>
            {tr("Update available", "Update verfügbar")}: {offer.version}
          </h3>
          <h4>{tr("Release notes", "Versionshinweise")}</h4>
          <div className="release-notes">
            {offer.notes ||
              tr(
                "No release notes provided.",
                "Keine Versionshinweise vorhanden.",
              )}
          </div>
          <Button
            disabled={active || updates.installing || updates.checking}
            onClick={() => void updates.install()}
          >
            {updates.installing
              ? tr("Downloading and installing…", "Download und Installation…")
              : tr("Install now", "Jetzt installieren")}
          </Button>
          {active && (
            <p>
              {tr(
                "Finish active synchronization before installing.",
                "Aktive Synchronisation vor der Installation abschließen.",
              )}
            </p>
          )}
          {updates.installing && (
            <p role="status">
              {Math.round((updates.status?.downloaded || 0) / 1024)} KiB
              {updates.status?.total
                ? ` / ${Math.round(updates.status.total / 1024)} KiB`
                : ""}
            </p>
          )}
        </section>
      )}
    </div>
  );
}
