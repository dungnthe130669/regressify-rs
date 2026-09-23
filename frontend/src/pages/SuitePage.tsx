import { ArrowUpRight, Grid2X2, List } from "lucide-react";
import { useDeferredValue } from "react";
import { Link, useLocation, useParams } from "react-router";
import {
  Filters,
  IconButton,
  ReportImage,
  StatePanel,
  StatusBadge,
  useFilters,
} from "../components/ui";
import { filterCaptures, imagePath, passRate } from "../data/report";
import { useSuite } from "../data/store";

export function SuitePage() {
  const { suiteId } = useParams();
  const { data, summary, error, retry } = useSuite(suiteId);
  const { query, status, params, set } = useFilters();
  const deferredQuery = useDeferredValue(query);
  const location = useLocation();
  if (!summary)
    return (
      <StatePanel
        title="Suite not found"
        detail="This suite is not included in this snapshot."
        back
      />
    );
  if (error) return <StatePanel title="Suite unavailable" detail={error} retry={retry} />;
  if (!data) return <StatePanel title="Loading suite" loading />;
  const captures = filterCaptures(data.tests, deferredQuery, status);
  const list = params.get("view") === "list";
  return (
    <div className="page suite-page">
      <header className="page-heading">
        <div>
          <div className="eyebrow">Test suite</div>
          <h1>{summary.id}</h1>
          <p className="run-id mono">Run {data.runId || "Unknown"}</p>
        </div>
        <StatusBadge status={summary.failed ? "fail" : "pass"} />
      </header>
      <div className="suite-summary">
        <span>
          <strong>{summary.total}</strong> captures
        </span>
        <span className="text-pass">
          <strong>{summary.passed}</strong> passed
        </span>
        <span className={summary.failed ? "text-fail" : "muted"}>
          <strong>{summary.failed}</strong> failed
        </span>
        <span className="muted">{passRate(summary.passed, summary.total)}% pass rate</span>
      </div>
      <div className="suite-toolbar">
        <Filters
          suite
          counts={{
            all: summary.total,
            pass: summary.passed,
            fail: summary.failed,
            error: summary.errors,
          }}
        />
        <div className="segmented layout-control">
          <IconButton label="Grid view" aria-pressed={!list} onClick={() => set("view", "")}>
            <Grid2X2 size={17} />
          </IconButton>
          <IconButton label="List view" aria-pressed={list} onClick={() => set("view", "list")}>
            <List size={17} />
          </IconButton>
        </div>
      </div>
      <div className="capture-count muted mono" aria-live="polite">
        {captures.length} {captures.length === 1 ? "capture" : "captures"}
      </div>
      {captures.length ? (
        <div className={list ? "capture-list" : "capture-grid"}>
          {captures.map((capture) => (
            <Link
              className="capture-card"
              key={capture.id}
              to={`${imagePath(summary.id, capture.id)}${location.search}`}
            >
              <div className="capture-thumbnail">
                <ReportImage
                  path={capture.test || capture.reference}
                  alt={`${capture.label} preview`}
                />
                <span className="open-capture">
                  <ArrowUpRight size={18} />
                </span>
              </div>
              <div className="capture-info">
                <div className="capture-card-top">
                  <span className="capture-scenario">
                    {capture.scenario || capture.selector || "Capture"}
                  </span>
                  <StatusBadge status={capture.status} error={capture.error !== null} />
                </div>
                <h2>{capture.label}</h2>
                <div className="capture-detail">
                  <span className="mono">
                    {capture.mismatch === null
                      ? "Not compared"
                      : `${capture.mismatch.toFixed(2)}% difference`}
                  </span>
                  <span className="mono muted">{capture.threshold}% threshold</span>
                </div>
                {capture.error && <p className="capture-error">{capture.error}</p>}
              </div>
            </Link>
          ))}
        </div>
      ) : (
        <StatePanel
          title="No matching captures"
          detail="No captures match the current search and status."
        />
      )}
    </div>
  );
}
