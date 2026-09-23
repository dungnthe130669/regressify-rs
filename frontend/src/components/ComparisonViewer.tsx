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
import {
  type ReactZoomPanPinchRef,
  TransformComponent,
  TransformWrapper,
} from "react-zoom-pan-pinch";
import type { Capture } from "../data/report";
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
}: {
  capture: Capture;
  mode: string | null;
  onMode: (mode: string) => void;
}) {
  const getSize = useImageSizes(capture);
  const [blend, setBlend] = useState(50);
  const [scale, setScale] = useState(1);
  const world = useRef<HTMLDivElement>(null);
  const viewer = useRef<HTMLElement>(null);
  const controls = useRef<ReactZoomPanPinchRef | null>(null);
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
    const observer = new ResizeObserver(() => {
      const current = controls.current;
      const width = current?.instance.wrapperComponent?.clientWidth;
      if (current && width) void current.setTransform(0, 0, Math.min(1, width / size.width), 0);
    });
    observer.observe(element);
    return () => observer.disconnect();
  }, [size.width]);
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
      </div>
      <TransformWrapper
        key={`${capture.id}:${mode}:${size.width}:${size.height}`}
        minScale={0.001}
        maxScale={8}
        limitToBounds={false}
        smooth={false}
        disablePadding
        keyboard={{ disabled: false }}
        wheel={{ step: scale * 0.1 }}
        onInit={(current) => {
          controls.current = current;
          const width = current.instance.wrapperComponent?.clientWidth || size.width;
          void current.setTransform(0, 0, Math.min(1, width / size.width), 0);
        }}
        onTransform={(_context, state) => setScale(state.scale)}
      >
        {({ zoomIn, zoomOut, centerView, setTransform, instance }) => (
          <>
            <div className="canvas-labels">
              {mode === "side" ? (
                lanes.map((lane) => (
                  <span key={lane.label}>
                    {lane.label}{" "}
                    <small className="mono">
                      {lane.size.width} x {lane.size.height}
                    </small>
                  </span>
                ))
              ) : mode === "wipe" || mode === "overlay" ? (
                <>
                  <span>
                    Reference{" "}
                    <small className="mono">
                      {referenceSize.width} x {referenceSize.height}
                    </small>
                  </span>
                  <span>
                    Test{" "}
                    <small className="mono">
                      {testSize.width} x {testSize.height}
                    </small>
                  </span>
                </>
              ) : (
                <span>
                  {modes.find(([value]) => value === mode)?.[1]}{" "}
                  <small className="mono">
                    {size.width} x {size.height}
                  </small>
                </span>
              )}
            </div>
            <TransformComponent
              wrapperClass="zoom-canvas"
              contentClass="zoom-content"
              wrapperProps={{
                role: "region",
                "aria-label": "Zoomable comparison images",
                onPointerMove: (event) => {
                  if (mode !== "wipe" || event.pointerType === "touch" || event.buttons !== 0)
                    return;
                  const bounds = world.current?.getBoundingClientRect();
                  if (bounds && bounds.width > 0) {
                    setBlend(
                      Math.round(
                        Math.max(
                          0,
                          Math.min(100, ((event.clientX - bounds.left) / bounds.width) * 100),
                        ),
                      ),
                    );
                  }
                },
              }}
            >
              <div
                ref={world}
                className={`image-world mode-${mode}`}
                style={{ width: size.width, height: size.height }}
              >
                {mode === "side" ? (
                  lanes.map((lane) => (
                    <div
                      className="comparison-lane"
                      key={lane.label}
                      style={{ width: laneWidth, height: size.height }}
                    >
                      <div style={lane.size}>
                        <ReportImage
                          path={lane.path}
                          alt={`${lane.label}: ${capture.label}`}
                          eager
                        />
                      </div>
                    </div>
                  ))
                ) : mode === "wipe" || mode === "overlay" ? (
                  <>
                    <div className="image-layer" style={referenceSize}>
                      <ReportImage
                        path={capture.reference}
                        alt={`Reference: ${capture.label}`}
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
                        <ReportImage path={capture.test} alt={`Test: ${capture.label}`} eager />
                      </div>
                    </div>
                    {mode === "wipe" && (
                      <div className="wipe-divider" style={{ left: `${blend}%` }} />
                    )}
                  </>
                ) : (
                  <ReportImage path={single} alt={`${mode}: ${capture.label}`} eager />
                )}
              </div>
            </TransformComponent>
            <div className="canvas-toolbar">
              <div className="zoom-controls">
                <IconButton
                  label="Zoom out"
                  onClick={() => {
                    void zoomOut();
                  }}
                >
                  <Minus size={16} />
                </IconButton>
                <output className="mono zoom-value" aria-label="Zoom level">
                  {Math.round(scale * 1000) / 10}%
                </output>
                <IconButton
                  label="Zoom in"
                  onClick={() => {
                    void zoomIn();
                  }}
                >
                  <Plus size={16} />
                </IconButton>
                <span className="control-divider" />
                <IconButton
                  label="Fit images"
                  onClick={() => {
                    const width = instance.wrapperComponent?.clientWidth || size.width;
                    void setTransform(0, 0, Math.min(1, width / size.width), 150);
                  }}
                >
                  <Scan size={17} />
                </IconButton>
                <IconButton
                  label="Actual size"
                  onClick={() => {
                    void centerView(1, 150);
                  }}
                >
                  <Focus size={17} />
                </IconButton>
              </div>
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
          </>
        )}
      </TransformWrapper>
    </section>
  );
}
