import {
  autoUpdate,
  FloatingPortal,
  flip,
  offset,
  safePolygon,
  shift,
  useClick,
  useDismiss,
  useFloating,
  useFocus,
  useHover,
  useInteractions,
  useRole,
} from "@floating-ui/react";
import { ArrowDown, ArrowUp, ArrowUpDown, Check, X } from "lucide-react";
import { useState } from "react";
import { Link, useSearchParams } from "react-router";
import { Filters, IconButton, StatePanel, useFilters } from "../components/ui";
import { passRate, suiteLabel, suitePath } from "../data/report";
import { useManifest } from "../data/store";

const columns = [
  { key: "name", label: "Test suite" },
  { key: "total", label: "Total tests" },
  { key: "passed", label: "Passed" },
  { key: "failed", label: "Failed" },
  { key: "rate", label: "Pass rate" },
] as const;

function EngineErrorCount({ count }: { count: number }) {
  const [open, setOpen] = useState(false);
  const { refs, floatingStyles, context } = useFloating({
    open,
    onOpenChange: setOpen,
    placement: "top",
    strategy: "fixed",
    middleware: [offset(6), flip({ padding: 12 }), shift({ padding: 12 })],
    whileElementsMounted: autoUpdate,
  });
  const hover = useHover(context, { move: false, handleClose: safePolygon() });
  const focus = useFocus(context, { visibleOnly: false });
  const click = useClick(context);
  const dismiss = useDismiss(context);
  const role = useRole(context, { role: "tooltip" });
  const { getReferenceProps, getFloatingProps } = useInteractions([
    hover,
    focus,
    click,
    dismiss,
    role,
  ]);
  const label = `${count} engine ${count === 1 ? "error" : "errors"}`;
  return (
    <>
      <button
        type="button"
        ref={refs.setReference}
        className="table-error mono"
        aria-label={label}
        {...getReferenceProps()}
      >
        {count}
      </button>
      {open && (
        <FloatingPortal>
          <div
            ref={refs.setFloating}
            style={floatingStyles}
            className="error-tooltip"
            {...getFloatingProps()}
          >
            {label}
          </div>
        </FloatingPortal>
      )}
    </>
  );
}

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
  const needle = query.trim().toLowerCase();
  const suites = manifest.suites
    .filter(
      (suite) =>
        (!needle || `${suite.id} ${suiteLabel(suite)}`.toLowerCase().includes(needle)) &&
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
          <h1 className="overview-title">Snapshot Overview</h1>
          <time className="overview-date mono" dateTime={manifest.generatedAt}>
            {new Date(manifest.generatedAt).toLocaleString(undefined, {
              dateStyle: "medium",
              timeStyle: "short",
            })}
          </time>
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
          <div className="overview-sort">
            <label htmlFor="overview-sort">Sort by</label>
            <select
              id="overview-sort"
              aria-label="Sort suites by"
              value={sort}
              onChange={(event) =>
                toggleSort(
                  columns.find((column) => column.key === event.target.value)?.key || "name",
                )
              }
            >
              {columns.map(({ key, label }) => (
                <option key={key} value={key}>
                  {label}
                </option>
              ))}
            </select>
            <IconButton
              label={descending ? "Sort ascending" : "Sort descending"}
              onClick={() => toggleSort(sort)}
            >
              {descending ? <ArrowDown size={16} /> : <ArrowUp size={16} />}
            </IconButton>
          </div>
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
                    <span className="mobile-column-label">{label}</span>
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {suites.map((suite) => (
                <tr key={suite.id}>
                  <td>
                    <div className="suite-heading">
                      <Link className="suite-name" to={suitePath(suite.id)}>
                        <span className={`status-dot ${suite.failed ? "fail" : "pass"}`} />
                        <span className="suite-label">{suiteLabel(suite)}</span>
                      </Link>
                      {suite.errors > 0 && <EngineErrorCount count={suite.errors} />}
                    </div>
                  </td>
                  <td className="mono" data-label="Total tests">
                    <span>{suite.total}</span>
                  </td>
                  <td className="mono" data-label="Passed">
                    <span
                      className={`result-count${suite.total > 0 && !suite.failed ? " pass" : ""}`}
                    >
                      {suite.passed}
                    </span>
                  </td>
                  <td className="mono" data-label="Failed">
                    <span className={`result-count${suite.failed ? " fail" : ""}`}>
                      {suite.failed}
                    </span>
                  </td>
                  <td data-label="Pass rate">
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
