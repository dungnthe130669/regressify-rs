import { ArrowDown, ArrowUp, ArrowUpRight, Check, Layers, ScanLine, X } from "lucide-react";
import Markdown from "react-markdown";
import { Link } from "react-router";
import { Filters, StatePanel, StatusBadge, useFilters } from "../components/ui";
import { passRate, safePageUrl, suiteLabel, suitePath } from "../data/report";
import { useManifest } from "../data/store";

export function OverviewPage() {
  const manifest = useManifest();
  const { query, status, params, set } = useFilters();
  const totals = manifest.suites.reduce(
    (sum, suite) => ({
      total: sum.total + suite.total,
      passed: sum.passed + suite.passed,
      failed: sum.failed + suite.failed,
      errors: sum.errors + suite.errors,
    }),
    { total: 0, passed: 0, failed: 0, errors: 0 },
  );
  const counts = {
    all: manifest.suites.length,
    pass: manifest.suites.filter((suite) => !suite.failed).length,
    fail: manifest.suites.filter((suite) => suite.failed).length,
    error: manifest.suites.filter((suite) => suite.errors).length,
  };
  const sort = params.get("sort") || "name";
  const descending = params.get("direction") === "desc";
  const suites = manifest.suites
    .filter(
      (suite) =>
        `${suite.id} ${suiteLabel(suite)}`.toLowerCase().includes(query.trim().toLowerCase()) &&
        (status === "all" ||
          (status === "error"
            ? suite.errors > 0
            : status === "fail"
              ? suite.failed > 0
              : suite.failed === 0)),
    )
    .sort((first, second) => {
      const order =
        sort === "failed"
          ? first.failed - second.failed
          : sort === "total"
            ? first.total - second.total
            : suiteLabel(first).localeCompare(suiteLabel(second));
      return descending ? -order : order;
    });
  const toggleSort = (key: string) => {
    if (sort === key) set("direction", descending ? "" : "desc");
    else set("sort", key);
  };
  return (
    <div className="page overview-page">
      <header className="page-heading">
        <div>
          <div className="eyebrow">Snapshot overview</div>
          <h1>{manifest.heading}</h1>
          <div className="report-subheading">
            <Markdown
              skipHtml
              allowedElements={["p", "strong", "em", "a", "code", "br", "ul", "ol", "li"]}
              components={{
                a: ({ children, href }) => (
                  <a href={safePageUrl(href || null)} target="_blank" rel="noopener noreferrer">
                    {children}
                  </a>
                ),
              }}
            >
              {manifest.subheading}
            </Markdown>
          </div>
        </div>
        <span className={`run-status ${totals.failed ? "fail" : "pass"}`}>
          {totals.failed ? <X size={16} /> : <Check size={16} />}
          {totals.failed
            ? "Changes detected"
            : totals.total
              ? "All checks passed"
              : "No comparisons"}
        </span>
      </header>
      <section className="metrics" aria-label="Snapshot totals">
        <div>
          <span>
            <Layers size={16} />
            Test suites
          </span>
          <strong>{manifest.suites.length}</strong>
          <small>{counts.fail} with failures</small>
        </div>
        <div>
          <span>
            <ScanLine size={16} />
            Captures
          </span>
          <strong>{totals.total}</strong>
          <small>Across all test suites</small>
        </div>
        <div>
          <span>
            <Check size={16} />
            Passed
          </span>
          <strong className="text-pass">{totals.passed}</strong>
          <small>{passRate(totals.passed, totals.total)}% pass rate</small>
        </div>
        <div>
          <span>
            <X size={16} />
            Failed
          </span>
          <strong className={totals.failed ? "text-fail" : ""}>{totals.failed}</strong>
          <small>{totals.errors} engine errors</small>
        </div>
      </section>
      <div className="section-title">
        <h2>Test suites</h2>
        <span className="mono muted">
          {suites.length} / {manifest.suites.length}
        </span>
      </div>
      <Filters counts={counts} />
      {suites.length ? (
        <div className="results-table-wrap">
          <table className="results-table">
            <thead>
              <tr>
                <th
                  aria-sort={sort === "name" ? (descending ? "descending" : "ascending") : "none"}
                >
                  <button type="button" onClick={() => toggleSort("name")}>
                    Suite{" "}
                    {sort === "name" &&
                      (descending ? <ArrowDown size={13} /> : <ArrowUp size={13} />)}
                  </button>
                </th>
                <th className="optional-column">Status</th>
                <th
                  aria-sort={sort === "total" ? (descending ? "descending" : "ascending") : "none"}
                >
                  <button type="button" onClick={() => toggleSort("total")}>
                    Captures
                  </button>
                </th>
                <th
                  aria-sort={sort === "failed" ? (descending ? "descending" : "ascending") : "none"}
                >
                  <button type="button" onClick={() => toggleSort("failed")}>
                    Failed
                  </button>
                </th>
                <th className="optional-column">Pass rate</th>
                <th>
                  <span className="sr-only">Open</span>
                </th>
              </tr>
            </thead>
            <tbody>
              {suites.map((suite) => (
                <tr key={suite.id}>
                  <td>
                    <Link className="suite-name" to={suitePath(suite.id)}>
                      <span className={`status-dot ${suite.failed ? "fail" : "pass"}`} />
                      {suiteLabel(suite)}
                    </Link>
                    {suite.errors > 0 && (
                      <span className="table-error">
                        {suite.errors} engine {suite.errors === 1 ? "error" : "errors"}
                      </span>
                    )}
                  </td>
                  <td className="optional-column">
                    <StatusBadge status={suite.failed ? "fail" : "pass"} />
                  </td>
                  <td className="mono">{suite.total}</td>
                  <td className={`mono ${suite.failed ? "text-fail" : "muted"}`}>{suite.failed}</td>
                  <td className="optional-column">
                    <div className="rate">
                      <progress
                        value={suite.passed}
                        max={suite.total || 1}
                        aria-label={`${suiteLabel(suite)} pass rate`}
                      />
                      <span className="mono">{passRate(suite.passed, suite.total)}%</span>
                    </div>
                  </td>
                  <td>
                    <Link
                      className="icon-button"
                      to={suitePath(suite.id)}
                      aria-label={`Open ${suiteLabel(suite)}`}
                      title={`Open ${suiteLabel(suite)}`}
                    >
                      <ArrowUpRight size={17} />
                    </Link>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <StatePanel
          title={manifest.suites.length ? "No matching suites" : "No comparisons yet"}
          detail={
            manifest.suites.length
              ? "No suites match the current search and status."
              : "This snapshot contains no comparison results."
          }
        />
      )}
    </div>
  );
}
