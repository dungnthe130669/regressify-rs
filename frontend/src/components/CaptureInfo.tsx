import {
  autoUpdate,
  FloatingPortal,
  flip,
  offset,
  safePolygon,
  shift,
  useDismiss,
  useFloating,
  useFocus,
  useHover,
  useInteractions,
  useRole,
} from "@floating-ui/react";
import { ExternalLink, Info, X } from "lucide-react";
import { useId, useState } from "react";
import { type Capture, safePageUrl } from "../data/report";
import { IconButton } from "./ui";

export function CaptureInfo({
  capture,
  suiteId,
  suiteName,
  runId,
  dimensions,
}: {
  capture: Capture;
  suiteId: string;
  suiteName: string;
  runId: string;
  dimensions: { label: string; width: number; height: number }[];
}) {
  const [open, setOpen] = useState(false);
  const [pinned, setPinned] = useState(false);
  const titleId = useId();
  const { refs, floatingStyles, context } = useFloating({
    open,
    onOpenChange(next, _event, reason) {
      if (pinned && (reason === "hover" || reason === "safe-polygon" || reason === "focus")) return;
      setOpen(next);
      if (!next) setPinned(false);
    },
    placement: "bottom-end",
    strategy: "fixed",
    middleware: [offset(8), flip({ padding: 12 }), shift({ padding: 12, crossAxis: true })],
    whileElementsMounted: autoUpdate,
  });
  const hover = useHover(context, {
    move: false,
    handleClose: safePolygon({ blockPointerEvents: false }),
  });
  const focus = useFocus(context, { visibleOnly: false });
  const dismiss = useDismiss(context);
  const role = useRole(context, { role: "dialog" });
  const { getReferenceProps, getFloatingProps } = useInteractions([hover, focus, dismiss, role]);
  const close = () => {
    setOpen(false);
    setPinned(false);
  };
  return (
    <>
      <button
        type="button"
        className="icon-button"
        ref={refs.setReference}
        aria-label="Capture information"
        {...getReferenceProps({
          onClick: () => {
            setPinned(!pinned);
            setOpen(!pinned);
          },
        })}
      >
        <Info size={18} />
      </button>
      {open && (
        <FloatingPortal>
          <dialog
            open
            ref={refs.setFloating}
            style={floatingStyles}
            className="info-popover"
            aria-labelledby={titleId}
            {...getFloatingProps()}
          >
            <div className="info-heading">
              <h2 id={titleId}>Capture information</h2>
              <IconButton label="Close capture information" onClick={close}>
                <X size={16} />
              </IconButton>
            </div>
            <dl className="capture-metadata">
              <div>
                <dt>Difference</dt>
                <dd className={`mono ${capture.status === "fail" ? "text-fail" : "text-pass"}`}>
                  {capture.mismatch === null ? "Not compared" : `${capture.mismatch.toFixed(2)}%`}
                </dd>
              </div>
              <div>
                <dt>Threshold</dt>
                <dd className="mono">{capture.threshold}%</dd>
              </div>
              <div>
                <dt>Suite</dt>
                <dd title={suiteId}>{suiteName}</dd>
              </div>
              <div>
                <dt>Scenario</dt>
                <dd>{capture.scenario || "Unspecified"}</dd>
              </div>
              <div>
                <dt>Capture</dt>
                <dd className="mono">{capture.id}</dd>
              </div>
              <div>
                <dt>Run</dt>
                <dd className="mono">{runId || "Unknown"}</dd>
              </div>
              <div>
                <dt>Selector</dt>
                <dd className="mono">{capture.selector || "Document"}</dd>
              </div>
              {dimensions.map((size) => (
                <div key={size.label}>
                  <dt>{size.label} size</dt>
                  <dd className="mono">
                    {size.width} x {size.height}
                  </dd>
                </div>
              ))}
              {capture.dimensions && !capture.dimensions.same && (
                <div className="metadata-wide">
                  <dt>Dimension difference</dt>
                  <dd className="mono">
                    {capture.dimensions.width > 0 ? "+" : ""}
                    {capture.dimensions.width}px width, {capture.dimensions.height > 0 ? "+" : ""}
                    {capture.dimensions.height}px height
                  </dd>
                </div>
              )}
              {(
                [
                  ["Reference page", capture.referenceUrl],
                  ["Test page", capture.testUrl],
                ] as const
              ).map(
                ([label, url]) =>
                  safePageUrl(url) && (
                    <div className="metadata-wide" key={label}>
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
            {(capture.error || capture.comparisonError) && (
              <div className="capture-failure" role="alert">
                <strong>{capture.error ? "Engine error" : "Comparison failed"}</strong>
                <pre>{capture.error || capture.comparisonError}</pre>
              </div>
            )}
          </dialog>
        </FloatingPortal>
      )}
    </>
  );
}
