import { CheckCircle2, CircleAlert, LoaderCircle, RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import { ThemeControl } from "./App";
import { IconButton } from "./components/ui";
import { fetchSiteStatus, type SiteStatus } from "./data/landing";
import { passRate } from "./data/report";

function SiteStatusPanel({ url, heading }: { url: string; heading: string }) {
  const [attempt, setAttempt] = useState(0);
  const [result, setResult] = useState<SiteStatus | "loading" | "unavailable">("loading");
  useEffect(() => {
    const controller = new AbortController();
    setResult("loading");
    void attempt;
    fetchSiteStatus(url, controller.signal).then(
      (status) => {
        if (!controller.signal.aborted) setResult(status);
      },
      () => {
        if (!controller.signal.aborted) setResult("unavailable");
      },
    );
    return () => controller.abort();
  }, [url, attempt]);
  const refresh = (
    <IconButton
      label={`Refresh status for ${heading}`}
      disabled={result === "loading"}
      onClick={() => setAttempt((value) => value + 1)}
    >
      <RefreshCw size={16} />
    </IconButton>
  );
  if (typeof result === "string") {
    return (
      <div className="landing-status-message muted" aria-live="polite">
        {result === "loading" ? (
          <LoaderCircle size={17} className="landing-status-spinner" />
        ) : (
          <CircleAlert size={17} />
        )}
        <span>{result === "loading" ? "Loading status" : "Status unavailable"}</span>
        {refresh}
      </div>
    );
  }
  return (
    <div className="landing-status-result" aria-live="polite">
      <div className="landing-status-heading">
        {result.total === 0 ? (
          <span className="muted">No captures</span>
        ) : (
          <span className={result.failed ? "text-fail" : "text-pass"}>
            {result.failed ? <CircleAlert size={17} /> : <CheckCircle2 size={17} />}
            <strong>{passRate(result.passed, result.total)}%</strong> passed
          </span>
        )}
        {refresh}
      </div>
      <dl className="landing-status-counts">
        <div>
          <dt>Suites</dt>
          <dd>{result.suites}</dd>
        </div>
        <div>
          <dt>Captures</dt>
          <dd>{result.total}</dd>
        </div>
        <div>
          <dt>Failed</dt>
          <dd className={result.failed ? "text-fail" : ""}>{result.failed}</dd>
        </div>
        <div>
          <dt>Errors</dt>
          <dd className={result.errors ? "text-fail" : ""}>{result.errors}</dd>
        </div>
      </dl>
      <time
        className="landing-status-date mono muted"
        dateTime={result.generatedAt}
        title="Report generated at"
      >
        {new Date(result.generatedAt).toLocaleString(undefined, {
          dateStyle: "medium",
          timeStyle: "short",
        })}
      </time>
    </div>
  );
}

export function enhanceLandingPage(): boolean {
  if (!document.querySelector("[data-regressify-landing]")) return false;
  const controls = document.getElementById("landing-controls");
  if (controls) createRoot(controls).render(<ThemeControl />);
  for (const element of document.querySelectorAll<HTMLElement>("[data-regressify-status]")) {
    createRoot(element).render(
      <SiteStatusPanel
        url={element.dataset.regressifyStatus || ""}
        heading={element.dataset.siteHeading || "site"}
      />,
    );
  }
  return true;
}
