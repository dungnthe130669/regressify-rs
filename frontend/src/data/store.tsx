import { createContext, type ReactNode, useContext, useEffect, useState } from "react";
import { StatePanel } from "../components/ui";
import {
  artifactPath,
  fetchReport,
  type Manifest,
  manifestSchema,
  type SuiteReport,
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

function loadSuiteReport(key: string, id: string, total: number, reload: boolean) {
  const state = getSuiteState(key);
  if (state.data || (state.controller && !reload)) return;
  state.controller?.abort();
  const controller = new AbortController();
  state.controller = controller;
  state.error = undefined;
  notifySuiteListeners(state);
  fetchReport(`/${key}`, suiteReportSchema, controller.signal, reload)
    .then((data) => {
      if (controller.signal.aborted) return;
      if (data.id !== id || data.tests.length !== total)
        throw new Error("Suite data does not match this snapshot.");
      state.data = data;
      state.error = undefined;
      state.controller = undefined;
      notifySuiteListeners(state);
    })
    .catch((reason: unknown) => {
      if (controller.signal.aborted) return;
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
    const configured = document.querySelector<HTMLMetaElement>(
      'meta[name="regressify-manifest"]',
    )?.content;
    const path = configured;
    setError(undefined);
    if (!path || !artifactPath(path, "data")) {
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
  const total = summary?.total ?? 0;
  useEffect(() => {
    if (!key || !id) return;
    const state = getSuiteState(key);
    const update = () => setRevision((value) => value + 1);
    state.listeners.add(update);
    if (!state.data && !state.error && !state.controller) {
      loadSuiteReport(key, id, total, false);
    }
    return () => {
      state.listeners.delete(update);
    };
  }, [key, id, total]);
  const state = key ? reportCache.get(key) : undefined;
  return {
    summary,
    data: state?.data,
    error: state?.error,
    retry: () => {
      if (key && id) loadSuiteReport(key, id, total, true);
    },
  };
}
