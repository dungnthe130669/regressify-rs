import {
  Columns2,
  Contrast,
  Download,
  Focus,
  Image,
  Layers2,
  Minus,
  Plus,
  Scan,
  SquareSplitHorizontal,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import type { Capture } from "../data/report";
import { CaptureInfo } from "./CaptureInfo";
import { IconButton, ReportImage } from "./ui";

const modes = [
  ["side", "Side by side", Columns2],
  ["wipe", "Wipe", SquareSplitHorizontal],
  ["overlay", "Overlay", Layers2],
  ["diff", "Diff", Contrast],
  ["reference", "Reference", Image],
  ["test", "Test", Image],
] as const;
type Mode = (typeof modes)[number][0];
type Size = { width: number; height: number };
const fallback: Size = { width: 640, height: 400 };

function useImageSizes(capture: Capture) {
  const [sizes, setSizes] = useState<Record<string, Size>>({});
  useEffect(() => {
    const images = [capture.reference, capture.test, capture.diff]
      .filter((path): path is string => path !== null)
      .map((path) => {
        const image = new window.Image();
        image.onload = () =>
          setSizes((current) => ({
            ...current,
            [path]: { width: image.naturalWidth, height: image.naturalHeight },
          }));
        image.src = `/${path}`;
        return image;
      });
    return () => {
      for (const image of images) image.onload = null;
    };
  }, [capture.reference, capture.test, capture.diff]);
  return (path: string | null) => (path ? sizes[path] || fallback : fallback);
}

export function ComparisonViewer({
  capture,
  mode: requestedMode,
  onMode,
  suiteId,
  runId,
}: {
  capture: Capture;
  mode: string | null;
  onMode: (mode: string) => void;
  suiteId: string;
  runId: string;
}) {
  const getSize = useImageSizes(capture);
  const [blend, setBlend] = useState(50);
  const [zoom, setZoom] = useState<{ mode: Mode; scale: number } | null>(null);
  const [viewerWidth, setViewerWidth] = useState(0);
  const world = useRef<HTMLDivElement>(null);
  const viewer = useRef<HTMLElement>(null);
  const [compact, setCompact] = useState(() => matchMedia("(max-width: 600px)").matches);
  useEffect(() => {
    const media = matchMedia("(max-width: 600px)");
    const update = () => setCompact(media.matches);
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);
  const available = (value: string) =>
    value === "side" ||
    (value === "reference"
      ? !!capture.reference
      : value === "test"
        ? !!capture.test
        : value === "diff"
          ? !!capture.diff
          : !!capture.reference && !!capture.test);
  const mode: Mode =
    modes.find(([value]) => value === requestedMode && available(value))?.[0] ||
    (compact && capture.reference && capture.test ? "wipe" : "side");
  const referenceSize = getSize(capture.reference);
  const testSize = getSize(capture.test);
  const lanes = [
    { label: "Reference", path: capture.reference, size: referenceSize },
    { label: "Test", path: capture.test, size: testSize },
    ...(capture.diff ? [{ label: "Diff", path: capture.diff, size: getSize(capture.diff) }] : []),
  ];
  const laneWidth = Math.max(...lanes.map((lane) => lane.size.width));
  const single =
    mode === "reference" ? capture.reference : mode === "diff" ? capture.diff : capture.test;
  const size =
    mode === "side"
      ? {
          width: laneWidth * lanes.length + 24 * (lanes.length - 1),
          height: Math.max(...lanes.map((lane) => lane.size.height)),
        }
      : mode === "wipe" || mode === "overlay"
        ? {
            width: Math.max(referenceSize.width, testSize.width),
            height: Math.max(referenceSize.height, testSize.height),
          }
        : getSize(single);
  useEffect(() => {
    const element = viewer.current;
    if (!element) return;
    const observer = new ResizeObserver(() => setViewerWidth(element.clientWidth));
    observer.observe(element);
    return () => observer.disconnect();
  }, []);
  const fitScale = Math.max(0.001, Math.min(1, (viewerWidth || size.width) / size.width));
  const scale = zoom?.mode === mode ? zoom.scale : fitScale;
  const setScale = (value: number) => setZoom({ mode, scale: Math.max(0.001, Math.min(8, value)) });
  return (
    <section ref={viewer} className="comparison-viewer" aria-label="Image comparison">
      <div className="comparison-modes">
        <fieldset className="segmented mode-segments" aria-label="Comparison mode">
          {modes.map(([value, label, Icon]) => (
            <button
              key={value}
              type="button"
              aria-pressed={mode === value}
              disabled={!available(value)}
              onClick={() => onMode(value)}
            >
              <Icon size={15} />
              <span>{label}</span>
            </button>
          ))}
        </fieldset>
        <select
          className="mode-select"
          aria-label="Comparison mode"
          value={mode}
          onChange={(event) => onMode(event.target.value)}
        >
          {modes.map(([value, label]) => (
            <option key={value} value={value} disabled={!available(value)}>
              {label}
            </option>
          ))}
        </select>
        <div className="zoom-controls">
          <IconButton label="Zoom out" onClick={() => setScale(scale / 1.2)}>
            <Minus size={16} />
          </IconButton>
          <output className="mono zoom-value" aria-label="Zoom level">
            {Math.round(scale * 1000) / 10}%
          </output>
          <IconButton label="Zoom in" onClick={() => setScale(scale * 1.2)}>
            <Plus size={16} />
          </IconButton>
          <IconButton label="Fit images" onClick={() => setZoom(null)}>
            <Scan size={17} />
          </IconButton>
          <IconButton label="Actual size" onClick={() => setScale(1)}>
            <Focus size={17} />
          </IconButton>
        </div>
        <div className="image-downloads">
          {(
            [
              ["reference", capture.reference],
              ["test", capture.test],
              ["diff", capture.diff],
            ] as const
          ).map(
            ([label, path]) =>
              path && (
                <a
                  key={label}
                  className="icon-button"
                  href={`/${path}`}
                  download={`${capture.id}-${label}.png`}
                  title={`Download ${label}`}
                  aria-label={`Download ${label}`}
                >
                  <Download size={16} />
                  <span className="download-letter" aria-hidden="true">
                    {label[0]?.toUpperCase()}
                  </span>
                </a>
              ),
          )}
        </div>
        <CaptureInfo
          capture={capture}
          suiteId={suiteId}
          runId={runId}
          dimensions={lanes
            .filter((lane) => lane.path)
            .map((lane) => ({ label: lane.label, ...lane.size }))}
        />
        {(mode === "wipe" || mode === "overlay") && (
          <label className="blend-control">
            <span>{mode === "wipe" ? "Wipe" : "Opacity"}</span>
            <input
              type="range"
              min="0"
              max="100"
              value={blend}
              aria-label={mode === "wipe" ? "Wipe position" : "Test image opacity"}
              onChange={(event) => setBlend(Number(event.target.value))}
            />
            <output className="mono">{blend}%</output>
          </label>
        )}
      </div>
      <div className="canvas-labels">
        {mode === "side" ? (
          lanes.map((lane) => <span key={lane.label}>{lane.label}</span>)
        ) : mode === "wipe" || mode === "overlay" ? (
          <>
            <span>Reference</span>
            <span>Test</span>
          </>
        ) : (
          <span>{modes.find(([value]) => value === mode)?.[1]}</span>
        )}
      </div>
      <section
        className="zoom-canvas"
        aria-label="Zoomable comparison images"
        style={{ width: size.width * scale, height: size.height * scale }}
        onPointerMove={(event) => {
          if (mode !== "wipe" || event.pointerType === "touch" || event.buttons !== 0) return;
          const bounds = world.current?.getBoundingClientRect();
          if (bounds && bounds.width > 0) {
            setBlend(
              Math.round(
                Math.max(0, Math.min(100, ((event.clientX - bounds.left) / bounds.width) * 100)),
              ),
            );
          }
        }}
      >
        <div
          ref={world}
          className={`image-world mode-${mode}`}
          style={{
            width: size.width,
            height: size.height,
            transform: `scale(${scale})`,
            transformOrigin: "top left",
          }}
        >
          {mode === "side" ? (
            lanes.map((lane) => (
              <div
                className="comparison-lane"
                key={lane.label}
                style={{ width: laneWidth, height: size.height }}
              >
                <div style={lane.size}>
                  <ReportImage path={lane.path} alt={`${lane.label}: ${capture.label}`} eager />
                </div>
              </div>
            ))
          ) : mode === "wipe" || mode === "overlay" ? (
            <>
              <div className="image-layer" style={referenceSize}>
                <ReportImage path={capture.reference} alt={`Reference: ${capture.label}`} eager />
              </div>
              <div
                className="image-layer"
                style={{
                  ...size,
                  opacity: mode === "overlay" ? blend / 100 : 1,
                  clipPath: mode === "wipe" ? `inset(0 0 0 ${blend}%)` : undefined,
                }}
              >
                <div style={testSize}>
                  <ReportImage path={capture.test} alt={`Test: ${capture.label}`} eager />
                </div>
              </div>
              {mode === "wipe" && <div className="wipe-divider" style={{ left: `${blend}%` }} />}
            </>
          ) : (
            <ReportImage path={single} alt={`${mode}: ${capture.label}`} eager />
          )}
        </div>
      </section>
    </section>
  );
}
