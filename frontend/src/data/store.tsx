import { createContext, type ReactNode, useContext, useEffect, useState } from "react";
import { StatePanel } from "../components/ui";
import {
  artifactPath,
  fetchReport,
  type Manifest,
  manifestSchema,
  type SuiteReport,
  type SuiteSummary,
  suiteReportSchema,
} from "./report";

const ManifestContext = createContext<Manifest | null>(null);
type SuiteState = {
  data?: SuiteReport;
  error?: string;
  controller?: AbortController;
  listeners: Set<() => void>;
};
const reportCache = new Map<string, SuiteState>();

function getSuiteState(key: string): SuiteState {
  let state = reportCache.get(key);
  if (!state) {
    state = { listeners: new Set() };
    reportCache.set(key, state);
  }
  return state;
}

function notifySuiteListeners(state: SuiteState) {
  for (const listener of state.listeners) listener();
}

function loadSuiteReport(summary: SuiteSummary, reload: boolean) {
  const state = getSuiteState(summary.report);
  if (state.data || (state.controller && !reload)) return;
  state.controller?.abort();
  const controller = new AbortController();
  state.controller = controller;
  state.error = undefined;
  notifySuiteListeners(state);
  fetchReport(`/${summary.report}`, suiteReportSchema, controller.signal, reload)
    .then((data) => {
      if (controller.signal.aborted || state.controller !== controller) return;
      const passed = data.tests.filter((test) => test.status === "pass").length;
      const errors = data.tests.filter((test) => test.error !== null).length;
      if (
        data.id !== summary.id ||
        data.tests.length !== summary.total ||
        passed !== summary.passed ||
        data.tests.length - passed !== summary.failed ||
        errors > summary.errors
      )
        throw new Error("Suite data does not match this snapshot.");
      state.data = data;
      state.error = undefined;
      state.controller = undefined;
      notifySuiteListeners(state);
    })
    .catch((reason: unknown) => {
      if (controller.signal.aborted || state.controller !== controller) return;
      state.error = reason instanceof Error ? reason.message : "Unable to load this suite.";
      state.controller = undefined;
      notifySuiteListeners(state);
    });
}

export function ReportProvider({ children }: { children: ReactNode }) {
  const [manifest, setManifest] = useState<Manifest>();
  const [error, setError] = useState<string>();
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    const controller = new AbortController();
    const path = document.querySelector<HTMLMetaElement>(
      'meta[name="regressify-manifest"]',
    )?.content;
    setError(undefined);
    if (!path || artifactPath(path, "data") !== "_snapshot/data/manifest.json") {
      setError("The snapshot entry page is missing its report manifest.");
      return;
    }
    fetchReport(`/${path}`, manifestSchema, controller.signal, attempt > 0)
      .then((data) => {
        if (!controller.signal.aborted) setManifest(data);
      })
      .catch((reason: unknown) => {
        if (!controller.signal.aborted)
          setError(reason instanceof Error ? reason.message : "Unable to load this snapshot.");
      });
    return () => controller.abort();
  }, [attempt]);
  if (error)
    return (
      <StatePanel
        title="Snapshot unavailable"
        detail={error}
        retry={() => setAttempt(attempt + 1)}
      />
    );
  if (!manifest) return <StatePanel title="Loading snapshot" loading />;
  return <ManifestContext.Provider value={manifest}>{children}</ManifestContext.Provider>;
}

export function useManifest(): Manifest {
  const manifest = useContext(ManifestContext);
  if (!manifest) throw new Error("Report data is not available.");
  return manifest;
}

export function useSuite(id: string | undefined) {
  const summary = useManifest().suites.find((suite) => suite.id === id);
  const [, setRevision] = useState(0);
  const key = summary?.report;
  useEffect(() => {
    if (!summary || !key) return;
    const state = getSuiteState(key);
    const update = () => setRevision((value) => value + 1);
    state.listeners.add(update);
    if (!state.data && !state.error && !state.controller) {
      loadSuiteReport(summary, false);
    }
    return () => {
      state.listeners.delete(update);
    };
  }, [summary, key]);
  const state = key ? reportCache.get(key) : undefined;
  return {
    summary,
    data: state?.data,
    error: state?.error,
    retry: () => {
      if (summary) loadSuiteReport(summary, true);
    },
  };
}
