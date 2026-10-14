import {
  AlertCircle,
  ArrowLeft,
  Check,
  ImageOff,
  LoaderCircle,
  RotateCcw,
  Search,
  X,
} from "lucide-react";
import { type ButtonHTMLAttributes, useEffect, useRef, useState } from "react";
import { Link, useSearchParams } from "react-router";
import type { StatusFilter } from "../data/report";

export function IconButton({
  label,
  children,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & { label: string }) {
  return (
    <button type="button" className="icon-button" aria-label={label} title={label} {...props}>
      {children}
    </button>
  );
}

export function StatusBadge({
  status,
  error = false,
}: {
  status: "pass" | "fail";
  error?: boolean;
}) {
  return (
    <span className={`status-badge ${error ? "error" : status}`}>
      {error ? (
        <AlertCircle size={13} />
      ) : status === "pass" ? (
        <Check size={13} />
      ) : (
        <X size={13} />
      )}
      {error ? "Error" : status === "pass" ? "Passed" : "Failed"}
    </span>
  );
}

export function StatePanel({
  title,
  detail,
  retry,
  loading,
  back,
}: {
  title: string;
  detail?: string;
  retry?: () => void;
  loading?: boolean;
  back?: boolean;
}) {
  return (
    <section className="state-panel" role={loading ? "status" : undefined} aria-live="polite">
      {loading ? <LoaderCircle size={28} className="spin" /> : <AlertCircle size={28} />}
      <h1>{title}</h1>
      {detail && <p>{detail}</p>}
      {retry && (
        <button className="button" type="button" onClick={retry}>
          <RotateCcw size={16} /> Retry
        </button>
      )}
      {back && (
        <Link className="button" to="/">
          <ArrowLeft size={16} /> Overview
        </Link>
      )}
    </section>
  );
}

const previewObservers = new Map<
  Element | null,
  { observer: IntersectionObserver; targets: Map<Element, (visible: boolean) => void> }
>();

function observePreview(element: HTMLImageElement, update: (visible: boolean) => void) {
  const root = null;
  let group = previewObservers.get(root);
  if (!group) {
    const targets = new Map<Element, (visible: boolean) => void>();
    const observer = new IntersectionObserver(
      (entries) => {
        for (const entry of entries) targets.get(entry.target)?.(entry.isIntersecting);
      },
      { root, rootMargin: "600px 0px" },
    );
    group = { observer, targets };
    previewObservers.set(root, group);
  }
  const { observer, targets } = group;
  targets.set(element, update);
  observer.observe(element);
  return () => {
    observer.unobserve(element);
    targets.delete(element);
    if (!targets.size) {
      observer.disconnect();
      previewObservers.delete(root);
    }
  };
}

export function ReportImage({
  path,
  alt,
  width,
  height,
  className,
  eager = false,
}: {
  path: string | null;
  alt: string;
  width: number;
  height: number;
  className?: string;
  eager?: boolean;
}) {
  const [failed, setFailed] = useState<string>();
  const [visible, setVisible] = useState(false);
  const image = useRef<HTMLImageElement>(null);
  useEffect(() => {
    if (eager || !path || failed === path || !image.current) return;
    return observePreview(image.current, setVisible);
  }, [eager, path, failed]);
  const active = eager || visible;
  return path && failed !== path ? (
    <img
      ref={image}
      className={className}
      src={active ? `/${path}` : undefined}
      alt={alt}
      width={width}
      height={height}
      style={active ? undefined : { visibility: "hidden" }}
      loading={eager ? "eager" : "lazy"}
      decoding="async"
      onError={() => setFailed(path)}
    />
  ) : (
    <div className="image-unavailable">
      <ImageOff size={24} />
      <span>{path ? "Image unavailable" : "No image"}</span>
    </div>
  );
}

export function useFilters() {
  const [params, setParams] = useSearchParams();
  const statusParam = params.get("status");
  const status: StatusFilter =
    statusParam === "pass" || statusParam === "fail" || statusParam === "error"
      ? statusParam
      : "all";
  const set = (key: string, value: string) => {
    const next = new URLSearchParams(params);
    if (value && value !== "all") next.set(key, value);
    else next.delete(key);
    setParams(next, { replace: true, preventScrollReset: true });
  };
  return { query: params.get("q") || "", status, params, set };
}

export function Filters({
  counts,
  suite = false,
}: {
  counts: { all: number; pass: number; fail: number; error: number };
  suite?: boolean;
}) {
  const { query, status, set } = useFilters();
  return (
    <div className="filter-controls">
      <label className="search-input">
        <Search size={16} />
        <input
          type="search"
          aria-label={suite ? "Search captures" : "Search suites"}
          placeholder={suite ? "Search captures..." : "Search suites..."}
          value={query}
          onChange={(event) => set("q", event.target.value)}
        />
      </label>
      <fieldset className="segmented status-filters" aria-label="Filter results">
        {(
          [
            ["all", "All"],
            ["fail", "Failed"],
            ["pass", "Passed"],
            ["error", "Errors"],
          ] as const
        ).map(([value, label]) => (
          <button
            type="button"
            key={value}
            aria-pressed={status === value}
            onClick={() => set("status", value)}
          >
            {label}
            <span className="mono">{counts[value]}</span>
          </button>
        ))}
      </fieldset>
    </div>
  );
}
