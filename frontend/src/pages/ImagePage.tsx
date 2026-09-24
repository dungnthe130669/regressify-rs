import { ArrowLeft, ArrowRight } from "lucide-react";
import { Link, useLocation, useParams } from "react-router";
import { ComparisonViewer } from "../components/ComparisonViewer";
import { IconButton, StatePanel, StatusBadge, useFilters } from "../components/ui";
import { filterCaptures, imagePath, suiteLabel, suitePath } from "../data/report";
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
      <ComparisonViewer
        key={capture.id}
        capture={capture}
        suiteId={summary.id}
        suiteName={suiteLabel(summary)}
        runId={data.runId}
        mode={params.get("mode")}
        onMode={(mode) => set("mode", mode)}
      />
    </div>
  );
}
