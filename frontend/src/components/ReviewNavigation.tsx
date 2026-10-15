import {
  autoUpdate,
  FloatingFocusManager,
  FloatingPortal,
  flip,
  offset,
  shift,
  size,
  useClick,
  useDismiss,
  useFloating,
  useInteractions,
  useListNavigation,
  useRole,
} from "@floating-ui/react";
import { Check, ChevronDown } from "lucide-react";
import { useId, useRef, useState } from "react";
import { Link, useMatch } from "react-router";
import { type Capture, imagePath, suiteLabel, suitePath } from "../data/report";
import { useManifest, useSuite } from "../data/store";

function ScenarioNavigation({ suiteId, close }: { suiteId: string; close: () => void }) {
  const { data, error, retry } = useSuite(suiteId);
  const selected = useMatch("/suites/:suiteId/images/:imageId");
  const [query, setQuery] = useState("");
  const groups = new Map<string, Capture[]>();
  for (const capture of data?.tests || []) {
    const name = capture.scenario || capture.label || capture.id;
    const group = groups.get(name) || [];
    group.push(capture);
    groups.set(name, group);
  }
  const needle = query.trim().toLowerCase();
  const visible = [...groups].filter(
    ([name, captures]) =>
      !needle ||
      `${name} ${captures.map((capture) => capture.label).join(" ")}`
        .toLowerCase()
        .includes(needle),
  );
  const captureLink = (capture: Capture, label: string) => (
    <Link
      key={capture.id}
      to={imagePath(suiteId, capture.id)}
      onClick={close}
      aria-current={selected?.params.imageId === capture.id ? "page" : undefined}
    >
      <span className={`status-dot ${capture.status}`} />
      <span className="suite-link-name">{label}</span>
    </Link>
  );
  return (
    <section className="navigation-section">
      <div className="nav-heading">
        <h2>Test scenarios</h2>
        <span className="mono muted">{groups.size}</span>
      </div>
      <input
        className="nav-search"
        aria-label="Find a scenario"
        placeholder="Find a scenario..."
        type="search"
        value={query}
        onChange={(event) => setQuery(event.target.value)}
      />
      <nav aria-label="Test scenarios" className="suite-links scenario-links">
        {error ? (
          <div className="nav-empty">
            <p role="alert">{error}</p>
            <button type="button" className="button" onClick={retry}>
              Retry
            </button>
          </div>
        ) : !data ? (
          <p className="muted nav-empty" role="status">
            Loading scenarios...
          </p>
        ) : visible.length ? (
          visible.map(([name, captures]) => {
            const first = captures[0];
            if (!first) return null;
            return captures.length === 1 ? (
              captureLink(first, name)
            ) : (
              <details className="scenario-group" key={name} open>
                <summary>
                  <span
                    className={`status-dot ${captures.some((capture) => capture.status === "fail") ? "fail" : "pass"}`}
                  />
                  <span>{name}</span>
                  <span className="mono muted">{captures.length}</span>
                </summary>
                {captures.map((capture) => captureLink(capture, capture.label))}
              </details>
            );
          })
        ) : (
          <p className="muted nav-empty">
            {groups.size ? "No matching scenarios" : "No scenarios"}
          </p>
        )}
      </nav>
    </section>
  );
}

export function ReviewNavigation({ close }: { close: () => void }) {
  const manifest = useManifest();
  const match = useMatch("/suites/:suiteId/*");
  const [query, setQuery] = useState("");
  const [open, setOpen] = useState(false);
  const [activeIndex, setActiveIndex] = useState<number | null>(null);
  const searchRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<Array<HTMLAnchorElement | null>>([]);
  const titleId = useId();
  const currentSuite = manifest.suites.find((suite) => suite.id === match?.params.suiteId);
  const currentLabel = currentSuite
    ? suiteLabel(currentSuite)
    : match?.params.suiteId || "Select a suite";
  const needle = query.trim().toLowerCase();
  const suites = manifest.suites.filter(
    (suite) => !needle || `${suite.id} ${suiteLabel(suite)}`.toLowerCase().includes(needle),
  );
  const changeOpen = (next: boolean) => {
    setOpen(next);
    if (!next) {
      setQuery("");
      setActiveIndex(null);
    }
  };
  const { refs, floatingStyles, context } = useFloating({
    open,
    onOpenChange(next, event, reason) {
      if (reason === "escape-key") event?.preventDefault();
      changeOpen(next);
    },
    placement: "bottom-start",
    strategy: "fixed",
    middleware: [
      offset(6),
      flip({ padding: 12 }),
      shift({ padding: 12 }),
      size({
        padding: 12,
        apply({ rects, availableHeight, availableWidth, elements }) {
          Object.assign(elements.floating.style, {
            width: `${Math.min(Math.max(rects.reference.width, 280), availableWidth)}px`,
            maxHeight: `${Math.min(400, availableHeight)}px`,
          });
        },
      }),
    ],
    whileElementsMounted: autoUpdate,
  });
  const click = useClick(context);
  const dismiss = useDismiss(context, { capture: { escapeKey: true } });
  const role = useRole(context, { role: "dialog" });
  const navigation = useListNavigation(context, {
    listRef,
    activeIndex,
    onNavigate: setActiveIndex,
    loop: true,
    focusItemOnOpen: false,
  });
  const { getReferenceProps, getFloatingProps, getItemProps } = useInteractions([
    click,
    dismiss,
    role,
    navigation,
  ]);
  return (
    <>
      <section className="navigation-section">
        <div className="nav-heading">
          <h2>Test suite</h2>
          <span className="mono muted">{manifest.suites.length}</span>
        </div>
        <button
          type="button"
          className="suite-selector"
          ref={refs.setReference}
          aria-label={`Test suite: ${currentLabel}`}
          {...getReferenceProps()}
        >
          {currentSuite && (
            <span className={`status-dot ${currentSuite.failed ? "fail" : "pass"}`} />
          )}
          <span className="suite-selector-name">{currentLabel}</span>
          <ChevronDown size={16} aria-hidden="true" />
        </button>
        {open && (
          <FloatingPortal root={refs.domReference.current?.closest("dialog") || undefined}>
            <FloatingFocusManager context={context} modal={false} initialFocus={searchRef}>
              <dialog
                open
                ref={refs.setFloating}
                style={floatingStyles}
                className="suite-dropdown"
                {...getFloatingProps()}
                aria-labelledby={titleId}
                aria-orientation={undefined}
              >
                <div className="nav-heading">
                  <h2 id={titleId}>Test suites</h2>
                  <span className="mono muted">{manifest.suites.length}</span>
                </div>
                <input
                  ref={searchRef}
                  className="nav-search"
                  aria-label="Find a suite"
                  placeholder="Find a suite..."
                  type="search"
                  value={query}
                  onChange={(event) => {
                    setQuery(event.target.value);
                    setActiveIndex(null);
                  }}
                />
                <nav aria-label="Test suites" className="suite-links">
                  {suites.map((suite, index) => (
                    <Link
                      key={suite.id}
                      to={suitePath(suite.id)}
                      ref={(element) => {
                        listRef.current[index] = element;
                      }}
                      tabIndex={activeIndex === index ? 0 : -1}
                      aria-current={match?.params.suiteId === suite.id ? "page" : undefined}
                      {...getItemProps({
                        onClick: () => {
                          changeOpen(false);
                          close();
                        },
                      })}
                    >
                      <span className={`status-dot ${suite.failed ? "fail" : "pass"}`} />
                      <span className="suite-link-name">{suiteLabel(suite)}</span>
                      <span className="mono muted">{suite.total}</span>
                      <Check size={14} className="suite-dropdown-check" aria-hidden="true" />
                    </Link>
                  ))}
                  {!suites.length && (
                    <p className="muted nav-empty" role="status">
                      {manifest.suites.length ? "No matching suites" : "No suites"}
                    </p>
                  )}
                </nav>
              </dialog>
            </FloatingFocusManager>
          </FloatingPortal>
        )}
      </section>
      {match?.params.suiteId && (
        <ScenarioNavigation
          key={match.params.suiteId}
          suiteId={match.params.suiteId}
          close={close}
        />
      )}
    </>
  );
}
