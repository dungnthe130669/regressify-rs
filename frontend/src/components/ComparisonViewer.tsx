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
  return {
    getSize: (path: string | null) =>
      path
        ? sizes[path] ||
          (path === capture.reference
            ? capture.imageSizes?.reference
            : path === capture.test
              ? capture.imageSizes?.test
              : capture.imageSizes?.diff)
        : undefined,
    isLoaded: (path: string | null) => !!path && !!sizes[path],
  };
}

export function ComparisonViewer({
  capture,
  mode: requestedMode,
  onMode,
  suiteId,
  suiteName,
  runId,
}: {
  capture: Capture;
  mode: string | null;
  onMode: (mode: string) => void;
  suiteId: string;
  suiteName: string;
  runId: string;
}) {
  const { getSize, isLoaded } = useImageSizes(capture);
  const [blend, setBlend] = useState(50);
  const [zoom, setZoom] = useState<{ mode: Mode; scale: number } | null>(null);
  const [crosshair, setCrosshair] = useState<{
    capture: Capture;
    scale: number;
    x: number;
    y: number;
  } | null>(null);
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
  const referenceSize = getSize(capture.reference) || fallback;
  const testSize = getSize(capture.test) || fallback;
  const lanes = [
    { label: "Reference", path: capture.reference, size: referenceSize },
    { label: "Test", path: capture.test, size: testSize },
    ...(capture.diff
      ? [{ label: "Diff", path: capture.diff, size: getSize(capture.diff) || fallback }]
      : []),
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
        : getSize(single) || fallback;
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
  useEffect(() => {
    const clear = () => setCrosshair(null);
    setCrosshair((current) =>
      mode === "side" && current?.capture === capture && current.scale === scale ? current : null,
    );
    window.addEventListener("scroll", clear, true);
    window.addEventListener("blur", clear);
    return () => {
      window.removeEventListener("scroll", clear, true);
      window.removeEventListener("blur", clear);
    };
  }, [capture, mode, scale]);
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
          suiteName={suiteName}
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
                <div
                  className="comparison-image"
                  style={lane.size}
                  onPointerMove={(event) => {
                    const image = event.currentTarget.querySelector("img");
                    if (
                      !capture.diff ||
                      event.pointerType === "touch" ||
                      event.buttons !== 0 ||
                      !image?.naturalWidth ||
                      !image.naturalHeight
                    ) {
                      setCrosshair(null);
                      return;
                    }
                    const bounds = image.getBoundingClientRect();
                    const imageX =
                      ((event.clientX - bounds.left) / bounds.width) * image.naturalWidth;
                    const imageY =
                      ((event.clientY - bounds.top) / bounds.height) * image.naturalHeight;
                    setCrosshair(
                      imageX >= 0 &&
                        imageX < image.naturalWidth &&
                        imageY >= 0 &&
                        imageY < image.naturalHeight
                        ? { capture, scale, x: imageX, y: imageY }
                        : null,
                    );
                  }}
                  onPointerLeave={() => setCrosshair(null)}
                  onPointerCancel={() => setCrosshair(null)}
                  onPointerDown={() => setCrosshair(null)}
                >
                  <ReportImage
                    path={lane.path}
                    alt={`${lane.label}: ${capture.label}`}
                    width={lane.size.width}
                    height={lane.size.height}
                    eager
                  />
                  {capture.diff &&
                    isLoaded(lane.path) &&
                    crosshair &&
                    crosshair.capture === capture &&
                    crosshair.scale === scale &&
                    crosshair.x < lane.size.width &&
                    crosshair.y < lane.size.height && (
                      <span
                        className="comparison-crosshair"
                        aria-hidden="true"
                        style={{
                          width: lane.size.width * scale,
                          height: lane.size.height * scale,
                          transform: `scale(${1 / scale})`,
                        }}
                      >
                        <span
                          className="crosshair-horizontal"
                          style={{ top: crosshair.y * scale }}
                        />
                        <span
                          className="crosshair-vertical"
                          style={{ left: crosshair.x * scale }}
                        />
                      </span>
                    )}
                </div>
              </div>
            ))
          ) : mode === "wipe" || mode === "overlay" ? (
            <>
              <div className="image-layer" style={referenceSize}>
                <ReportImage
                  path={capture.reference}
                  alt={`Reference: ${capture.label}`}
                  width={referenceSize.width}
                  height={referenceSize.height}
                  eager
                />
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
                  <ReportImage
                    path={capture.test}
                    alt={`Test: ${capture.label}`}
                    width={testSize.width}
                    height={testSize.height}
                    eager
                  />
                </div>
              </div>
              {mode === "wipe" && <div className="wipe-divider" style={{ left: `${blend}%` }} />}
            </>
          ) : (
            <ReportImage
              path={single}
              alt={`${mode}: ${capture.label}`}
              width={size.width}
              height={size.height}
              eager
            />
          )}
        </div>
      </section>
    </section>
  );
}
