import { ArrowLeft, ArrowRight, ExternalLink } from "lucide-react";
import { Link, useLocation, useParams } from "react-router";
import { ComparisonViewer } from "../components/ComparisonViewer";
import { IconButton, StatePanel, StatusBadge, useFilters } from "../components/ui";
import { filterCaptures, imagePath, safePageUrl, suitePath } from "../data/report";
import { useSuite } from "../data/store";

export default function ImagePage() {
  const { suiteId, imageId } = useParams();
  const { summary, data, error, retry } = useSuite(suiteId);
  const { query, status, params, set } = useFilters();
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
  const backParams = new URLSearchParams(params);
  backParams.delete("mode");
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
        <StatusBadge status={capture.status} error={capture.error !== null} />
      </header>
      <div className="viewer-toolbar">
        <div className="comparison-facts">
          <span>
            <span className="muted">Difference</span>{" "}
            <strong className={`mono ${capture.status === "fail" ? "text-fail" : "text-pass"}`}>
              {capture.mismatch === null ? "Not compared" : `${capture.mismatch.toFixed(2)}%`}
            </strong>
          </span>
          <span>
            <span className="muted">Threshold</span>{" "}
            <strong className="mono">{capture.threshold}%</strong>
          </span>
        </div>
        <div className="viewer-pagination">
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
      </div>
      {capture.error && (
        <div className="engine-error" role="alert">
          <strong>Engine error</strong>
          <pre>{capture.error}</pre>
        </div>
      )}
      {capture.dimensions && !capture.dimensions.same && (
        <div className="dimension-notice">
          Image dimensions differ{" "}
          <span className="mono">
            {capture.dimensions.width > 0 ? "+" : ""}
            {capture.dimensions.width}px width, {capture.dimensions.height > 0 ? "+" : ""}
            {capture.dimensions.height}px height
          </span>
        </div>
      )}
      <ComparisonViewer
        key={capture.id}
        capture={capture}
        mode={params.get("mode")}
        onMode={(mode) => set("mode", mode)}
      />
      <details className="review-details">
        <summary>Capture details</summary>
        <dl className="capture-metadata">
          <div>
            <dt>Capture</dt>
            <dd className="mono">{capture.id}</dd>
          </div>
          <div>
            <dt>Scenario</dt>
            <dd>{capture.scenario || "Unspecified"}</dd>
          </div>
          <div>
            <dt>Selector</dt>
            <dd className="mono">{capture.selector || "Document"}</dd>
          </div>
          {(
            [
              ["Reference page", capture.referenceUrl],
              ["Test page", capture.testUrl],
            ] as const
          ).map(
            ([label, url]) =>
              safePageUrl(url) && (
                <div key={label}>
                  <dt>{label}</dt>
                  <dd>
                    <a href={safePageUrl(url)} target="_blank" rel="noopener noreferrer">
                      {url}
                      <ExternalLink size={13} />
                    </a>
                  </dd>
                </div>
              ),
          )}
        </dl>
      </details>
    </div>
  );
}
