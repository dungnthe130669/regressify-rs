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
const reportCache = new Map<string, SuiteReport>();

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
  const [result, setResult] = useState<{ key: string; data?: SuiteReport; error?: string }>();
  const [attempt, setAttempt] = useState(0);
  const key = summary?.report;
  useEffect(() => {
    if (!key || !id || reportCache.has(key)) return;
    const controller = new AbortController();
    setResult({ key });
    fetchReport(`/${key}`, suiteReportSchema, controller.signal, attempt > 0)
      .then((data) => {
        if (controller.signal.aborted) return;
        if (data.id !== id || data.tests.length !== summary?.total)
          throw new Error("Suite data does not match this snapshot.");
        reportCache.set(key, data);
        setResult({ key, data });
      })
      .catch((reason: unknown) => {
        if (!controller.signal.aborted)
          setResult({
            key,
            error: reason instanceof Error ? reason.message : "Unable to load this suite.",
          });
      });
    return () => controller.abort();
  }, [key, id, summary?.total, attempt]);
  return {
    summary,
    data: key ? reportCache.get(key) || (result?.key === key ? result.data : undefined) : undefined,
    error: result?.key === key ? result?.error : undefined,
    retry: () => setAttempt(attempt + 1),
  };
}
