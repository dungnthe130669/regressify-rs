import { ArrowDown, ArrowUp, ArrowUpDown, Check, X } from "lucide-react";
import Markdown from "react-markdown";
import { Link, useSearchParams } from "react-router";
import { Filters, StatePanel, useFilters } from "../components/ui";
import { passRate, safePageUrl, suiteLabel, suitePath } from "../data/report";
import { useManifest } from "../data/store";

const columns = [
  { key: "name", label: "Test suite" },
  { key: "total", label: "Total tests" },
  { key: "passed", label: "Passed" },
  { key: "failed", label: "Failed" },
  { key: "rate", label: "Pass rate" },
] as const;

export function OverviewPage() {
  const manifest = useManifest();
  const { query, status } = useFilters();
  const [params, setParams] = useSearchParams();
  const totals = manifest.suites.reduce(
    (sum, suite) => ({
      total: sum.total + suite.total,
      passed: sum.passed + suite.passed,
      failed: sum.failed + suite.failed,
    }),
    { total: 0, passed: 0, failed: 0 },
  );
  const counts = {
    all: manifest.suites.length,
    pass: manifest.suites.filter((suite) => !suite.failed).length,
    fail: manifest.suites.filter((suite) => suite.failed).length,
    error: manifest.suites.filter((suite) => suite.errors).length,
  };
  const sort = columns.find((column) => column.key === params.get("sort"))?.key || "name";
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
        sort === "name"
          ? suiteLabel(first).localeCompare(suiteLabel(second))
          : sort === "rate"
            ? first.passed / (first.total || 1) - second.passed / (second.total || 1)
            : first[sort] - second[sort];
      return descending ? -order : order;
    });
  const toggleSort = (key: (typeof columns)[number]["key"]) => {
    const next = new URLSearchParams(params);
    next.set("sort", key);
    if (sort === key && !descending) next.set("direction", "desc");
    else next.delete("direction");
    setParams(next, { replace: true, preventScrollReset: true });
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
          <span>TOTAL TESTS</span>
          <strong>{totals.total}</strong>
        </div>
        <div>
          <span>PASSED</span>
          <strong className={totals.total > 0 && !totals.failed ? "text-pass" : ""}>
            {totals.passed}
          </strong>
        </div>
        <div>
          <span>FAILED</span>
          <strong className={totals.failed ? "text-fail" : ""}>{totals.failed}</strong>
        </div>
        <div>
          <span>PASS RATE</span>
          <strong>{passRate(totals.passed, totals.total)}%</strong>
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
        <section className="results-table-wrap" aria-label="Test suite results">
          <table className="results-table">
            <thead>
              <tr>
                {columns.map(({ key, label }) => (
                  <th
                    key={key}
                    scope="col"
                    aria-sort={sort === key ? (descending ? "descending" : "ascending") : "none"}
                  >
                    <button type="button" onClick={() => toggleSort(key)}>
                      {label}
                      {sort !== key ? (
                        <ArrowUpDown size={13} aria-hidden="true" />
                      ) : descending ? (
                        <ArrowDown size={13} aria-hidden="true" />
                      ) : (
                        <ArrowUp size={13} aria-hidden="true" />
                      )}
                    </button>
                  </th>
                ))}
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
                  <td className="mono">{suite.total}</td>
                  <td className="mono">
                    <span
                      className={`result-count${suite.total > 0 && !suite.failed ? " pass" : ""}`}
                    >
                      {suite.passed}
                    </span>
                  </td>
                  <td className="mono">
                    <span className={`result-count${suite.failed ? " fail" : ""}`}>
                      {suite.failed}
                    </span>
                  </td>
                  <td>
                    <div className="rate">
                      <progress
                        className={suite.failed ? "fail" : "pass"}
                        value={suite.passed}
                        max={suite.total || 1}
                        aria-label={`${suiteLabel(suite)} pass rate`}
                      />
                      <span className="mono">{passRate(suite.passed, suite.total)}%</span>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
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
