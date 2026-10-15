import { ArrowLeft, ArrowRight, Image, List } from "lucide-react";
import { Link, useLocation, useParams, useSearchParams } from "react-router";
import { CaptureLog } from "../components/CaptureLog";
import { ComparisonViewer } from "../components/ComparisonViewer";
import { IconButton, StatePanel, StatusBadge, useFilters } from "../components/ui";
import { filterCaptures, imagePath, suiteLabel, suitePath } from "../data/report";
import { useSuite } from "../data/store";

export default function ImagePage() {
  const { suiteId, imageId } = useParams();
  const { summary, data, error, retry } = useSuite(suiteId);
  const { query, status, params, set } = useFilters();
  const [, setParams] = useSearchParams();
  const location = useLocation();
  if (!summary) return <StatePanel title="Suite not found" back />;
  if (error) return <StatePanel title="Comparison unavailable" detail={error} retry={retry} />;
  if (!data) return <StatePanel title="Loading comparison" loading />;
  const index = data.tests.findIndex((test) => test.id === imageId);
  const capture = data.tests[index];
  if (!capture)
    return (
      <StatePanel
        title="Image not found"
        detail="This capture is not included in this snapshot."
        back
      />
    );
  const filtered = filterCaptures(data.tests, query, status);
  const sequence = filtered.some((test) => test.id === capture.id) ? filtered : data.tests;
  const sequenceIndex = sequence.findIndex((test) => test.id === capture.id);
  const previous = sequence[sequenceIndex - 1];
  const next = sequence[sequenceIndex + 1];
  const showingLog = params.get("view") === "log";
  const suiteView =
    (showingLog ? params.get("suiteView") : params.get("view")) === "list" ? "list" : "";
  const setLogView = (showLog: boolean) => {
    const nextParams = new URLSearchParams(params);
    const nextView = showLog ? "log" : suiteView;
    if (nextView) nextParams.set("view", nextView);
    else nextParams.delete("view");
    if (showLog && suiteView) nextParams.set("suiteView", suiteView);
    else nextParams.delete("suiteView");
    setParams(nextParams, { replace: true, preventScrollReset: true });
  };
  const backParams = new URLSearchParams(params);
  backParams.delete("mode");
  if (showingLog) {
    if (suiteView) backParams.set("view", suiteView);
    else backParams.delete("view");
  }
  for (const key of ["suiteView", "logRole", "logLevel", "logStage", "logSearch", "logChunk"])
    backParams.delete(key);
  const backSearch = backParams.size ? `?${backParams}` : "";
  return (
    <div className="page image-page">
      <header className="page-heading">
        <div>
          <Link className="back-link" to={`${suitePath(summary.id)}${backSearch}`}>
            <ArrowLeft size={14} />
            Results
          </Link>
          <h1>{capture.label}</h1>
        </div>
        <div className="viewer-pagination">
          <StatusBadge status={capture.status} error={capture.error !== null} />
          <span className="mono muted">
            {sequenceIndex + 1} / {sequence.length}
          </span>
          {previous ? (
            <Link
              className="icon-button"
              aria-label="Previous image"
              title="Previous image"
              to={`${imagePath(summary.id, previous.id)}${location.search}`}
            >
              <ArrowLeft size={17} />
            </Link>
          ) : (
            <IconButton label="Previous image" disabled>
              <ArrowLeft size={17} />
            </IconButton>
          )}
          {next ? (
            <Link
              className="icon-button"
              aria-label="Next image"
              title="Next image"
              to={`${imagePath(summary.id, next.id)}${location.search}`}
            >
              <ArrowRight size={17} />
            </Link>
          ) : (
            <IconButton label="Next image" disabled>
              <ArrowRight size={17} />
            </IconButton>
          )}
        </div>
      </header>
      <fieldset className="log-view-tabs segmented" aria-label="Capture review view">
        <button type="button" aria-pressed={!showingLog} onClick={() => setLogView(false)}>
          <Image size={15} />
          Comparison
        </button>
        <button type="button" aria-pressed={showingLog} onClick={() => setLogView(true)}>
          <List size={15} />
          Capture Log
        </button>
      </fieldset>
      {showingLog ? (
        <CaptureLog
          key={`${summary.id}/${capture.id}/${params.get("logRole")}`}
          capture={capture}
          params={params}
          set={set}
        />
      ) : (
        <ComparisonViewer
          key={`${summary.id}/${capture.id}`}
          capture={capture}
          suiteId={summary.id}
          suiteName={suiteLabel(summary)}
          runId={data.runId}
          mode={params.get("mode")}
          onMode={(mode) => set("mode", mode)}
        />
      )}
    </div>
  );
}
