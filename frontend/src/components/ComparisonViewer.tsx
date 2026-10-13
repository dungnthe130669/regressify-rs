import {
  Columns2,
  Contrast,
  Download,
  Focus,
  FoldHorizontal,
  Image,
  Layers2,
  Minus,
  Plus,
  Scan,
  SquareSplitHorizontal,
} from "lucide-react";
import { useEffect, useEffectEvent, useLayoutEffect, useRef, useState } from "react";
import {
  alignedRowToImageRow,
  type Capture,
  imageAlignmentGap,
  imageRowToAlignedRow,
} from "../data/report";
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
type Fit = "width" | "area" | "manual";
type View = {
  fit: Fit;
  scale: number;
  padding?: { left: number; top: number; right: number; bottom: number };
};
const fallback: Size = { width: 640, height: 400 };

const zoomLevels = [
  0.25,
  1 / 3,
  0.5,
  2 / 3,
  0.75,
  0.8,
  0.9,
  1,
  1.1,
  1.25,
  1.5,
  1.75,
  2,
  2.5,
  3,
  4,
  5,
  6,
  8,
];

export function nextZoom(current: number, increase: boolean): number {
  return increase
    ? (zoomLevels.find((value) => value > current + 0.001) ?? 8)
    : (zoomLevels.findLast((value) => value < current - 0.001) ?? Math.min(current, 0.25));
}

export function imageFitScale(image: Size, viewport: Size, fit: Fit): number {
  return Math.max(
    0.001,
    Math.min(1, viewport.width / image.width, fit === "area" ? viewport.height / image.height : 1),
  );
}

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
  const [zoom, setZoom] = useState<{
    capture: Capture;
    views: Partial<Record<Mode, View>>;
  } | null>(null);
  const [crosshair, setCrosshair] = useState<{
    capture: Capture;
    scale: number;
    x: number;
    y: number;
  } | null>(null);
  const [viewport, setViewport] = useState<Size>({ width: 0, height: 0 });
  const [panning, setPanning] = useState(false);
  const world = useRef<HTMLDivElement>(null);
  const viewer = useRef<HTMLElement>(null);
  const canvas = useRef<HTMLElement>(null);
  const stage = useRef<HTMLDivElement>(null);
  const toolbar = useRef<HTMLDivElement>(null);
  const gestureScope = useRef({ capture, mode: requestedMode });
  const anchor = useRef<{
    x: number;
    y: number;
    imageX: number;
    imageY: number;
    element: HTMLElement;
  } | null>(null);
  const pan = useRef<{
    pointer: number;
    x: number;
    y: number;
    left: number;
    top: number;
    moved: boolean;
  } | null>(null);
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
  const diffSize = getSize(capture.diff);
  const lanes = [
    {
      label: "Reference",
      path: capture.reference,
      size: referenceSize,
      gap: imageAlignmentGap(capture.alignment?.reference, referenceSize.height, diffSize?.height),
    },
    {
      label: "Test",
      path: capture.test,
      size: testSize,
      gap: imageAlignmentGap(capture.alignment?.test, testSize.height, diffSize?.height),
    },
    ...(capture.diff
      ? [{ label: "Diff", path: capture.diff, size: diffSize || fallback, gap: null }]
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
    const scroller = element.closest<HTMLElement>(".review-scroll");
    const controls = toolbar.current;
    const observer = new ResizeObserver(() =>
      setViewport({
        width: element.clientWidth,
        height: Math.max(
          1,
          (scroller?.clientHeight ?? innerHeight) - (controls?.clientHeight ?? 0) - 24,
        ),
      }),
    );
    observer.observe(element);
    if (scroller) observer.observe(scroller);
    if (controls) observer.observe(controls);
    return () => observer.disconnect();
  }, []);
  const view = zoom?.capture === capture ? zoom.views[mode] : undefined;
  const fit = view?.fit ?? "width";
  const scale =
    fit === "manual" && view
      ? view.scale
      : imageFitScale(
          size,
          { width: viewport.width || size.width, height: viewport.height || size.height },
          fit,
        );
  const canvasWidth =
    mode === "side" ? Math.max(size.width * scale, viewport.width) : size.width * scale;
  const renderedLaneWidth = (canvasWidth - 24 * scale * (lanes.length - 1)) / lanes.length;
  const padding = view?.padding;
  const scrollRoot = () => viewer.current?.closest<HTMLElement>(".review-scroll");
  const cancelPan = () => {
    const pointer = pan.current?.pointer;
    pan.current = null;
    setPanning(false);
    if (pointer !== undefined && canvas.current?.hasPointerCapture(pointer)) {
      canvas.current.releasePointerCapture(pointer);
    }
  };
  const setView = (next: View) => {
    cancelPan();
    setCrosshair(null);
    setZoom((current) => ({
      capture,
      views: { ...(current?.capture === capture ? current.views : {}), [mode]: next },
    }));
  };
  const setScale = (value: number, point?: { x: number; y: number }) => {
    const next = Math.max(0.001, Math.min(8, value));
    if (!Number.isFinite(next) || (fit === "manual" && next === scale)) return;
    const bounds = canvas.current?.getBoundingClientRect();
    const surface = stage.current?.getBoundingClientRect();
    const scroll = scrollRoot();
    let padding: View["padding"];
    anchor.current = null;
    if (bounds && surface && scroll && point) {
      const laneIndex =
        mode === "side"
          ? Math.max(
              0,
              Math.min(
                lanes.length - 1,
                Math.floor((point.x - bounds.left) / (renderedLaneWidth + 24 * scale)),
              ),
            )
          : null;
      const source =
        laneIndex === null
          ? canvas.current
          : canvas.current?.querySelectorAll<HTMLElement>(".comparison-image")[laneIndex];
      if (source) {
        const image = source.getBoundingClientRect();
        const imageX = (point.x - image.left) / scale;
        const imageY = (point.y - image.top) / scale;
        const nextWidth =
          mode === "side" ? Math.max(size.width * next, viewport.width) : size.width * next;
        const nextLaneWidth = (nextWidth - 24 * next * (lanes.length - 1)) / lanes.length;
        const sourceOffset =
          laneIndex === null
            ? 0
            : laneIndex * (nextLaneWidth + 24 * next) +
              (nextLaneWidth - (lanes[laneIndex]?.size.width ?? laneWidth) * next) / 2;
        const originX = surface.left + scroll.scrollLeft;
        const originY = surface.top + scroll.scrollTop;
        const left = point.x - originX - imageX * next - sourceOffset;
        const top = point.y - originY - imageY * next;
        const scrollBounds = scroll.getBoundingClientRect();
        // Padding preserves the pixel anchor when native scroll offsets would clamp.
        padding = {
          left: Math.max(0, left),
          top: Math.max(0, top),
          right: Math.max(0, scrollBounds.left + scroll.clientWidth - originX - left - nextWidth),
          bottom: Math.max(
            0,
            scrollBounds.top + scroll.clientHeight - originY - top - size.height * next,
          ),
        };
        anchor.current = { ...point, imageX, imageY, element: source };
      }
    }
    setView({ fit: "manual", scale: next, padding });
  };
  const setFit = (next: "width" | "area") => {
    anchor.current = null;
    setView({ fit: next, scale: 1 });
    const scroll = scrollRoot();
    const element = viewer.current;
    if (scroll && element) {
      scroll.scrollTo({
        left: 0,
        top:
          scroll.scrollTop +
          element.getBoundingClientRect().top -
          scroll.getBoundingClientRect().top,
      });
    }
  };
  useLayoutEffect(() => {
    const pending = anchor.current;
    anchor.current = null;
    const bounds = pending?.element.isConnected ? pending.element.getBoundingClientRect() : null;
    const scroll = viewer.current?.closest<HTMLElement>(".review-scroll");
    if (pending && bounds && scroll) {
      scroll.scrollLeft += bounds.left + pending.imageX * scale - pending.x;
      scroll.scrollTop += bounds.top + pending.imageY * scale - pending.y;
    }
  }, [scale]);
  const wheelZoom = useEffectEvent((event: WheelEvent) => {
    if ((!event.ctrlKey && !event.metaKey) || !Number.isFinite(event.deltaY) || event.deltaY === 0)
      return;
    event.preventDefault();
    event.stopPropagation();
    setScale(nextZoom(scale, event.deltaY < 0), { x: event.clientX, y: event.clientY });
  });
  const clearGesture = useEffectEvent(() => {
    cancelPan();
    setCrosshair(null);
    anchor.current = null;
  });
  useEffect(() => {
    const element = viewer.current?.closest<HTMLElement>(".review-scroll");
    if (!element) return;
    element.addEventListener("wheel", wheelZoom, { passive: false });
    return () => element.removeEventListener("wheel", wheelZoom);
  }, []);
  useEffect(() => {
    if (gestureScope.current.capture !== capture || gestureScope.current.mode !== mode) {
      clearGesture();
      gestureScope.current = { capture, mode };
    }
  }, [capture, mode]);
  useEffect(() => {
    const clear = () => clearGesture();
    window.addEventListener("blur", clear);
    return () => {
      window.removeEventListener("blur", clear);
      clear();
    };
  }, []);
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
      <div ref={toolbar} className="comparison-modes">
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
          <IconButton
            label="Zoom out"
            disabled={scale <= 0.25}
            onClick={() => setScale(nextZoom(scale, false))}
          >
            <Minus size={16} />
          </IconButton>
          <output className="mono zoom-value" aria-label="Zoom level">
            {Math.round(scale * 1000) / 10}%
          </output>
          <IconButton
            label="Zoom in"
            disabled={scale >= 8}
            onClick={() => setScale(nextZoom(scale, true))}
          >
            <Plus size={16} />
          </IconButton>
          <IconButton
            label="Fit images"
            title="Fit width (W)"
            aria-pressed={fit === "width"}
            onClick={() => setFit("width")}
          >
            <FoldHorizontal size={17} />
          </IconButton>
          <IconButton
            label="Fit to window"
            title="Fit to window (0)"
            aria-pressed={fit === "area"}
            onClick={() => setFit("area")}
          >
            <Scan size={17} />
          </IconButton>
          <IconButton
            label="Actual size"
            title="Actual size (1)"
            aria-pressed={fit === "manual" && scale === 1}
            onClick={() => setScale(1)}
          >
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
      <div
        ref={stage}
        className="comparison-stage"
        style={{
          width: padding ? canvasWidth + padding.left + padding.right : undefined,
          paddingLeft: padding?.left,
          paddingTop: padding?.top,
          paddingRight: padding?.right,
          paddingBottom: padding?.bottom,
        }}
      >
        <section
          ref={canvas}
          className={`zoom-canvas${panning ? " is-panning" : ""}`}
          aria-label="Zoomable comparison images"
          aria-keyshortcuts="+ - 0 1 W"
          // biome-ignore lint/a11y/noNoninteractiveTabindex: This image region owns scoped zoom and pan keys while retaining native scrolling.
          tabIndex={0}
          style={{ width: canvasWidth, height: size.height * scale }}
          onPointerDown={(event) => {
            setCrosshair(null);
            if (event.button !== 0 || event.pointerType === "touch") return;
            const scroll = scrollRoot();
            if (!scroll) return;
            event.preventDefault();
            event.currentTarget.focus({ preventScroll: true });
            pan.current = {
              pointer: event.pointerId,
              x: event.clientX,
              y: event.clientY,
              left: scroll.scrollLeft,
              top: scroll.scrollTop,
              moved: false,
            };
            event.currentTarget.setPointerCapture(event.pointerId);
          }}
          onPointerMove={(event) => {
            const drag = pan.current;
            if (drag && drag.pointer === event.pointerId) {
              if (!(event.buttons & 1)) {
                cancelPan();
                return;
              }
              const deltaX = event.clientX - drag.x;
              const deltaY = event.clientY - drag.y;
              if (!drag.moved && Math.max(Math.abs(deltaX), Math.abs(deltaY)) < 4) return;
              drag.moved = true;
              setPanning(true);
              setCrosshair(null);
              const scroll = scrollRoot();
              if (scroll) scroll.scrollTo({ left: drag.left - deltaX, top: drag.top - deltaY });
              return;
            }
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
          onPointerUp={cancelPan}
          onPointerCancel={cancelPan}
          onLostPointerCapture={cancelPan}
          onContextMenu={cancelPan}
          onDoubleClick={(event) => {
            if (event.button !== 0) return;
            event.preventDefault();
            if (fit === "manual" && scale === 1) setFit("area");
            else setScale(1, { x: event.clientX, y: event.clientY });
          }}
          onKeyDown={(event) => {
            if (
              event.target !== event.currentTarget ||
              event.ctrlKey ||
              event.metaKey ||
              event.altKey
            )
              return;
            if (event.key === "+" || event.key === "=") setScale(nextZoom(scale, true));
            else if (event.key === "-") setScale(nextZoom(scale, false));
            else if (event.key === "0") setFit("area");
            else if (event.key === "1") setScale(1);
            else if (event.key.toLowerCase() === "w") setFit("width");
            else if (event.key === "Escape") cancelPan();
            else if (["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"].includes(event.key)) {
              const step = event.shiftKey ? 160 : 40;
              scrollRoot()?.scrollBy({
                left: event.key === "ArrowLeft" ? -step : event.key === "ArrowRight" ? step : 0,
                top: event.key === "ArrowUp" ? -step : event.key === "ArrowDown" ? step : 0,
              });
            } else return;
            event.preventDefault();
            event.stopPropagation();
          }}
        >
          <div
            ref={world}
            className={`image-world mode-${mode}`}
            style={{
              width: canvasWidth / scale,
              height: size.height,
              transform: `scale(${scale})`,
              transformOrigin: "top left",
            }}
          >
            {mode === "side" ? (
              lanes.map((lane) => {
                const crosshairY = crosshair ? alignedRowToImageRow(crosshair.y, lane.gap) : null;
                return (
                  <div
                    className="comparison-lane"
                    key={lane.label}
                    style={{ width: renderedLaneWidth / scale, height: size.height }}
                  >
                    <div
                      className="comparison-image"
                      style={lane.size}
                      onPointerMove={(event) => {
                        const image = event.currentTarget.querySelector("img");
                        if (
                          capture.error !== null ||
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
                            ? {
                                capture,
                                scale,
                                x: imageX,
                                y: imageRowToAlignedRow(imageY, lane.gap),
                              }
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
                      {isLoaded(lane.path) &&
                        crosshair &&
                        crosshair.capture === capture &&
                        crosshair.scale === scale &&
                        crosshair.x < lane.size.width &&
                        crosshairY !== null &&
                        crosshairY < lane.size.height && (
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
                              style={{ top: crosshairY * scale }}
                            />
                            <span
                              className="crosshair-vertical"
                              style={{ left: crosshair.x * scale }}
                            />
                          </span>
                        )}
                    </div>
                  </div>
                );
              })
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
      </div>
    </section>
  );
}
