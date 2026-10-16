import { useId, useState } from "react";
import { Button } from "./ui/button";

function PathRow({
  side,
  path,
  lang,
}: {
  side: string;
  path: string;
  lang: string;
}) {
  const [expanded, setExpanded] = useState(false);
  const [feedback, setFeedback] = useState("");
  const [copying, setCopying] = useState(false);
  const id = useId();
  const de = lang === "de";
  const folder = de ? `Ordner ${side}` : `folder ${side}`;
  async function copy() {
    setFeedback("");
    setCopying(true);
    try {
      await navigator.clipboard.writeText(path);
      setFeedback(de ? "Pfad kopiert" : "Path copied");
    } catch {
      setExpanded(true);
      setFeedback(
        de
          ? "Kopieren nicht verfügbar. Vollständigen Pfad auswählen und manuell kopieren."
          : "Clipboard unavailable. Select the full path and copy it manually.",
      );
    } finally {
      setCopying(false);
    }
  }
  return (
    <div className="path-row">
      <div className="path-line">
        <b>{side}</b>
        <code title={path}>{path}</code>
        <Button
          type="button"
          variant="ghost"
          aria-expanded={expanded}
          aria-controls={id}
          aria-label={
            de
              ? `Pfad von ${folder} ${expanded ? "einklappen" : "aufklappen"}`
              : `${expanded ? "Collapse" : "Expand"} ${folder} path`
          }
          onClick={() => setExpanded(!expanded)}
        >
          {expanded
            ? de
              ? "Weniger"
              : "Less"
            : de
              ? "Vollständig"
              : "Full path"}
        </Button>
        <Button
          type="button"
          variant="ghost"
          disabled={copying}
          aria-label={
            de ? `Pfad von ${folder} kopieren` : `Copy ${folder} path`
          }
          onClick={() => void copy()}
        >
          {de ? "Kopieren" : "Copy"}
        </Button>
      </div>
      {expanded && (
        <textarea
          id={id}
          readOnly
          rows={2}
          value={path}
          aria-label={
            de ? `Vollständiger Pfad von ${folder}` : `Full ${folder} path`
          }
          onFocus={(event) => event.currentTarget.select()}
        />
      )}
      {feedback && <small role="status">{feedback}</small>}
    </div>
  );
}

export function PathPair({
  a,
  b,
  lang,
}: {
  a: string;
  b: string;
  lang: string;
}) {
  return (
    <div className="paths">
      <PathRow key={`A:${a}`} side="A" path={a} lang={lang} />
      <PathRow key={`B:${b}`} side="B" path={b} lang={lang} />
    </div>
  );
}
