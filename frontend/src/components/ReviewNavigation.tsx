import { useState } from "react";
import { Link, useMatch } from "react-router";
import { type Capture, imagePath, suiteLabel, suitePath } from "../data/report";
import { useManifest, useSuite } from "../data/store";

function ScenarioNavigation({ suiteId, close }: { suiteId: string; close: () => void }) {
  const { data, error, retry } = useSuite(suiteId);
  const selected = useMatch("/suites/:suiteId/images/:imageId");
  const [query, setQuery] = useState("");
  const groups = new Map<string, Capture[]>();
  for (const capture of data?.tests || []) {
    const name = capture.scenario || capture.label || capture.id;
    const group = groups.get(name) || [];
    group.push(capture);
    groups.set(name, group);
  }
  const visible = [...groups].filter(([name, captures]) =>
    `${name} ${captures.map((capture) => capture.label).join(" ")}`
      .toLowerCase()
      .includes(query.trim().toLowerCase()),
  );
  const captureLink = (capture: Capture, label: string) => (
    <Link
      key={capture.id}
      to={imagePath(suiteId, capture.id)}
      onClick={close}
      aria-current={selected?.params.imageId === capture.id ? "page" : undefined}
    >
      <span className={`status-dot ${capture.status}`} />
      <span className="suite-link-name">{label}</span>
    </Link>
  );
  return (
    <section className="navigation-section">
      <div className="nav-heading">
        <h2>Test scenarios</h2>
        <span className="mono muted">{groups.size}</span>
      </div>
      <input
        className="nav-search"
        aria-label="Find a scenario"
        placeholder="Find a scenario..."
        type="search"
        value={query}
        onChange={(event) => setQuery(event.target.value)}
      />
      <nav aria-label="Test scenarios" className="suite-links scenario-links">
        {error ? (
          <div className="nav-empty">
            <p role="alert">{error}</p>
            <button type="button" className="button" onClick={retry}>
              Retry
            </button>
          </div>
        ) : !data ? (
          <p className="muted nav-empty" role="status">
            Loading scenarios...
          </p>
        ) : visible.length ? (
          visible.map(([name, captures]) => {
            const first = captures[0];
            if (!first) return null;
            return captures.length === 1 ? (
              captureLink(first, name)
            ) : (
              <details className="scenario-group" key={name} open>
                <summary>
                  <span
                    className={`status-dot ${captures.some((capture) => capture.status === "fail") ? "fail" : "pass"}`}
                  />
                  <span>{name}</span>
                  <span className="mono muted">{captures.length}</span>
                </summary>
                {captures.map((capture) => captureLink(capture, capture.label))}
              </details>
            );
          })
        ) : (
          <p className="muted nav-empty">
            {groups.size ? "No matching scenarios" : "No scenarios"}
          </p>
        )}
      </nav>
    </section>
  );
}

export function ReviewNavigation({ close }: { close: () => void }) {
  const manifest = useManifest();
  const match = useMatch("/suites/:suiteId/*");
  const [query, setQuery] = useState("");
  const suites = manifest.suites.filter((suite) =>
    `${suite.id} ${suiteLabel(suite)}`.toLowerCase().includes(query.trim().toLowerCase()),
  );
  return (
    <>
      <details className="navigation-section">
        <summary className="nav-heading">
          <span>Test suites</span>
          <span className="mono muted">{manifest.suites.length}</span>
        </summary>
        <input
          className="nav-search"
          aria-label="Find a suite"
          placeholder="Find a suite..."
          type="search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
        />
        <nav aria-label="Test suites" className="suite-links">
          {suites.map((suite) => (
            <Link
              key={suite.id}
              to={suitePath(suite.id)}
              onClick={close}
              aria-current={match?.params.suiteId === suite.id ? "page" : undefined}
            >
              <span className={`status-dot ${suite.failed ? "fail" : "pass"}`} />
              <span className="suite-link-name">{suiteLabel(suite)}</span>
              <span className="mono muted">{suite.total}</span>
            </Link>
          ))}
          {!suites.length && <p className="muted nav-empty">No matching suites</p>}
        </nav>
      </details>
      {match?.params.suiteId && (
        <ScenarioNavigation
          key={match.params.suiteId}
          suiteId={match.params.suiteId}
          close={close}
        />
      )}
    </>
  );
}
