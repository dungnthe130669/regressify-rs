import {
  ChevronRight,
  Monitor,
  Moon,
  PanelLeftClose,
  PanelLeftOpen,
  ScanLine,
  Sun,
  X,
} from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { BrowserRouter, Link, Navigate, Route, Routes, useLocation, useMatch } from "react-router";
import { IconButton, StatePanel } from "./components/ui";
import { suitePath } from "./data/report";
import { ReportProvider, useManifest } from "./data/store";
import ImagePage from "./pages/ImagePage";
import { OverviewPage } from "./pages/OverviewPage";
import { SuitePage } from "./pages/SuitePage";

function ThemeControl() {
  const [theme, setTheme] = useState(() => {
    try {
      const saved = localStorage.getItem("regressify-theme");
      return saved === "light" || saved === "dark" ? saved : "system";
    } catch {
      return "system";
    }
  });
  useEffect(() => {
    const media = matchMedia("(prefers-color-scheme: dark)");
    const apply = () => {
      document.documentElement.dataset.theme =
        theme === "system" ? (media.matches ? "dark" : "light") : theme;
    };
    apply();
    try {
      localStorage.setItem("regressify-theme", theme);
    } catch {
      /* Storage is optional in private/file contexts. */
    }
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [theme]);
  return (
    <fieldset className="segmented theme-control" aria-label="Color theme">
      {(
        [
          ["light", Sun],
          ["dark", Moon],
          ["system", Monitor],
        ] as const
      ).map(([value, Icon]) => (
        <IconButton
          key={value}
          label={`${value[0]?.toUpperCase()}${value.slice(1)} theme`}
          aria-pressed={theme === value}
          onClick={() => setTheme(value)}
        >
          <Icon size={16} />
        </IconButton>
      ))}
    </fieldset>
  );
}

function SuiteNavigation({ close }: { close: () => void }) {
  const manifest = useManifest();
  const match = useMatch("/suites/:suiteId/*");
  const [query, setQuery] = useState("");
  return (
    <>
      <div className="nav-heading">
        <span>Test suites</span>
        <span className="mono muted">{manifest.suites.length}</span>
      </div>
      <input
        className="nav-search"
        aria-label="Find a suite"
        placeholder="Find a suite..."
        type="search"
        value={query}
        onChange={(event) => setQuery(event.target.value)}
      />
      <nav aria-label="Test suites" className="suite-links">
        {manifest.suites
          .filter((suite) => suite.id.toLowerCase().includes(query.toLowerCase()))
          .map((suite) => (
            <Link
              key={suite.id}
              to={suitePath(suite.id)}
              onClick={close}
              aria-current={match?.params.suiteId === suite.id ? "page" : undefined}
            >
              <span className={`status-dot ${suite.failed ? "fail" : "pass"}`} />
              <span className="suite-link-name">{suite.id}</span>
              <span className="mono muted">{suite.total}</span>
            </Link>
          ))}
        {!manifest.suites.some((suite) => suite.id.toLowerCase().includes(query.toLowerCase())) && (
          <p className="muted nav-empty">No matching suites</p>
        )}
      </nav>
    </>
  );
}

function Shell() {
  const manifest = useManifest();
  const location = useLocation();
  const match = useMatch("/suites/:suiteId/*");
  const imageMatch = useMatch("/suites/:suiteId/images/:imageId");
  const [panelPreference, setPanelPreference] = useState<boolean | null>(null);
  const panelOpen = panelPreference ?? !imageMatch;
  const dialog = useRef<HTMLDialogElement>(null);
  const main = useRef<HTMLElement>(null);
  useEffect(() => {
    document.title = manifest.title;
  }, [manifest.title]);
  useEffect(() => {
    if (location.pathname) main.current?.focus({ preventScroll: true });
    window.scrollTo(0, 0);
  }, [location.pathname]);
  const togglePanel = () => {
    if (matchMedia("(max-width: 959px)").matches) dialog.current?.showModal();
    else setPanelPreference(!panelOpen);
  };
  return (
    <>
      <a href="#main" className="skip-link">
        Skip to results
      </a>
      <header className="app-header">
        <Link to="/" className="brand" aria-label="Regressify overview">
          <ScanLine size={23} strokeWidth={2.2} />
          <span>Regressify</span>
          <span className="brand-divider" />
          <span className="brand-context">Snapshot</span>
        </Link>
        <div className="header-actions">
          <ThemeControl />
        </div>
      </header>
      <div className={`app-layout ${match && panelOpen ? "with-panel" : ""}`}>
        {match && panelOpen && (
          <aside className="suite-panel" id="suite-panel">
            <SuiteNavigation close={() => {}} />
            <div className="panel-footer">
              <ScanLine size={14} /> {manifest.suites.length} suites in this snapshot
            </div>
          </aside>
        )}
        <div className="workspace">
          <section className="breadcrumb-bar" aria-label="Snapshot navigation">
            {match && (
              <IconButton
                label="Toggle suite index"
                aria-expanded={panelOpen}
                aria-controls={panelOpen ? "suite-panel" : undefined}
                onClick={togglePanel}
              >
                {panelOpen ? <PanelLeftClose size={18} /> : <PanelLeftOpen size={18} />}
              </IconButton>
            )}
            <nav aria-label="Breadcrumb">
              <Link to="/">Overview</Link>
              {match && (
                <>
                  <ChevronRight size={14} />
                  <Link to={suitePath(match.params.suiteId || "")}>{match.params.suiteId}</Link>
                  {location.pathname.includes("/images/") && (
                    <>
                      <ChevronRight size={14} />
                      <span>Comparison</span>
                    </>
                  )}
                </>
              )}
            </nav>
            <span className="snapshot-date mono">
              {new Date(manifest.generatedAt).toLocaleString(undefined, {
                dateStyle: "medium",
                timeStyle: "short",
              })}
            </span>
          </section>
          <main id="main" ref={main} tabIndex={-1}>
            <Routes>
              <Route path="/" element={<OverviewPage />} />
              <Route path="/index.html" element={<Navigate to="/" replace />} />
              <Route path="/suites/:suiteId" element={<SuitePage />} />
              <Route path="/suites/:suiteId/images/:imageId" element={<ImagePage />} />
              <Route
                path="*"
                element={
                  <StatePanel
                    title="Page not found"
                    detail="This URL is not part of this snapshot."
                    back
                  />
                }
              />
            </Routes>
          </main>
          {!imageMatch && (
            <footer className="app-footer">
              <span>Regressify</span>
              <span>Visual regression report</span>
            </footer>
          )}
        </div>
      </div>
      <dialog
        ref={dialog}
        aria-label="Suite index"
        className="mobile-navigation"
        onClick={(event) => {
          if (event.target === dialog.current) dialog.current?.close();
        }}
        onKeyDown={(event) => {
          if (event.key === "Escape") dialog.current?.close();
        }}
      >
        <div className="mobile-nav-content">
          <div className="mobile-nav-close">
            <IconButton label="Close suite index" onClick={() => dialog.current?.close()}>
              <X size={20} />
            </IconButton>
          </div>
          <SuiteNavigation close={() => dialog.current?.close()} />
        </div>
      </dialog>
    </>
  );
}

export default function App() {
  return (
    <BrowserRouter>
      <ReportProvider>
        <Shell />
      </ReportProvider>
    </BrowserRouter>
  );
}
