use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

use anyhow::{Result, anyhow, bail};
use chromiumoxide::{Browser, Command, Page, cdp::browser_protocol::dom::NodeId, js::Evaluation};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tokio::sync::Mutex;

use crate::types::Frame;

use super::cdp::Client;

#[derive(Clone)]
struct Realm {
    session: String,
    frame: String,
}

#[derive(Clone)]
pub(crate) struct Runtime {
    client: Client,
    main_session: String,
    cache: Arc<Mutex<ContextCache>>,
}

struct ContextCache {
    frames: BTreeMap<(String, String), (String, i64)>,
    installed: BTreeSet<(String, i64)>,
    sessions: BTreeMap<String, String>,
    events: tokio::sync::broadcast::Receiver<super::cdp::Event>,
}

impl ContextCache {
    fn new(events: tokio::sync::broadcast::Receiver<super::cdp::Event>) -> Self {
        Self {
            frames: BTreeMap::new(),
            installed: BTreeSet::new(),
            sessions: BTreeMap::new(),
            events,
        }
    }

    fn drain(&mut self) {
        use tokio::sync::broadcast::error::TryRecvError;
        loop {
            match self.events.try_recv() {
                Ok(event)
                    if matches!(
                        event.method.as_str(),
                        "Page.frameNavigated"
                            | "Page.frameDetached"
                            | "Runtime.executionContextsCleared"
                            | "Runtime.executionContextDestroyed"
                            | "Target.detachedFromTarget"
                    ) =>
                {
                    self.frames.clear();
                    self.installed.clear();
                    if event.method == "Target.detachedFromTarget" {
                        self.sessions.clear();
                    }
                }
                Ok(_) => {}
                Err(TryRecvError::Lagged(_)) => {
                    self.frames.clear();
                    self.installed.clear();
                    self.sessions.clear();
                }
                Err(TryRecvError::Closed | TryRecvError::Empty) => break,
            }
        }
    }
}

#[derive(Clone)]
pub struct Target {
    runtime: Runtime,
    realm: Realm,
    parents: Vec<(Realm, String)>,
}

pub struct Evaluated(Value);

impl Evaluated {
    pub fn into_value<T: DeserializeOwned>(self) -> Result<T> {
        Ok(serde_json::from_value(self.0)?)
    }
}

pub struct Response<T> {
    pub result: T,
}

pub struct Element {
    pub node_id: NodeId,
}

pub(crate) struct NavigationWatch {
    events: tokio::sync::broadcast::Receiver<super::cdp::Event>,
    main_frame: String,
}

impl Runtime {
    pub async fn new(browser: &Browser, page: &Page) -> Result<Self> {
        let client = Client::connect(browser).await?;
        let events = client.subscribe();
        let main_session = client.attach(page.target_id().as_ref()).await?;
        client
            .call(Some(&main_session), "Page.enable", json!({}))
            .await?;
        client
            .call(Some(&main_session), "Runtime.enable", json!({}))
            .await?;
        Ok(Self {
            client,
            main_session,
            cache: Arc::new(Mutex::new(ContextCache::new(events))),
        })
    }

    async fn root(&self) -> Result<Realm> {
        let tree = self
            .client
            .call(Some(&self.main_session), "Page.getFrameTree", json!({}))
            .await?;
        let frame = tree["frameTree"]["frame"]["id"]
            .as_str()
            .ok_or_else(|| anyhow!("Main frame is unavailable"))?;
        Ok(Realm {
            session: self.main_session.clone(),
            frame: frame.into(),
        })
    }

    async fn context(&self, realm: &Realm) -> Result<(String, i64)> {
        let key = (realm.session.clone(), realm.frame.clone());
        {
            let mut cache = self.cache.lock().await;
            cache.drain();
            if let Some(context) = cache.frames.get(&key) {
                return Ok(context.clone());
            }
        }
        let params = json!({"frameId": realm.frame, "worldName": "__regressify_runtime"});
        let mut session = realm.session.clone();
        let result = match self
            .client
            .call(Some(&session), "Page.createIsolatedWorld", params.clone())
            .await
        {
            Ok(result) => result,
            Err(_) => {
                session = if let Some(session) =
                    self.cache.lock().await.sessions.get(&realm.frame).cloned()
                {
                    session
                } else {
                    let session = self.client.attach(&realm.frame).await?;
                    self.client
                        .call(Some(&session), "Page.enable", json!({}))
                        .await?;
                    self.client
                        .call(Some(&session), "Runtime.enable", json!({}))
                        .await?;
                    self.cache
                        .lock()
                        .await
                        .sessions
                        .insert(realm.frame.clone(), session.clone());
                    session
                };
                self.client
                    .call(Some(&session), "Page.createIsolatedWorld", params)
                    .await?
            }
        };
        let context = result["executionContextId"]
            .as_i64()
            .ok_or_else(|| anyhow!("Frame execution context is unavailable"))?;
        let result = (session, context);
        let mut cache = self.cache.lock().await;
        cache.drain();
        cache.frames.insert(key, result.clone());
        Ok(result)
    }

    async fn evaluate(&self, realm: &Realm, expression: &str, by_value: bool) -> Result<Value> {
        let (session, context) = self.context(realm).await?;
        let key = (session.clone(), context);
        let installed = self.cache.lock().await.installed.contains(&key);
        if !installed {
            let bootstrap = format!(
                r#"(() => {{
                const module = {{ exports: {{}} }};
                const exports = module.exports;
                {}
                const engine = new (module.exports.InjectedScript())(globalThis, {{
                    isUnderTest: false, sdkLanguage: 'javascript', testIdAttributeName: 'data-testid',
                    stableRafCount: 2, browserName: 'chromium', customEngines: []
                }});
                const deepElement = (element) => {{
                    let previous;
                    while (element && element !== previous && element.shadowRoot) {{
                        previous = element;
                        element = element.shadowRoot.activeElement || element;
                    }}
                    return element;
                }};
                globalThis.__regressifyDocument = new Proxy(document, {{ get(target, key) {{
                    if (key === 'querySelector') return selector => engine.querySelector(engine.parseSelector(selector), target, false);
                    if (key === 'querySelectorAll') return selector => engine.querySelectorAll(engine.parseSelector(selector), target);
                    if (key === 'activeElement') return deepElement(target.activeElement);
                    if (key === 'elementFromPoint') return (horizontal, vertical) => {{
                        let element = target.elementFromPoint(horizontal, vertical);
                        let previous;
                        while (element && element !== previous && element.shadowRoot) {{
                            previous = element;
                            element = element.shadowRoot.elementFromPoint(horizontal, vertical) || element;
                        }}
                        return element;
                    }};
                    const value = Reflect.get(target, key, target);
                    return typeof value === 'function' ? value.bind(target) : value;
                }} }});
            }})()"#,
                include_str!("vendor/injected.js")
            );
            let result = self
                .client
                .call(
                    Some(&session),
                    "Runtime.evaluate",
                    json!({
                        "contextId": context, "expression": bootstrap, "returnByValue": true
                    }),
                )
                .await?;
            check_evaluation(&result)?;
            self.cache.lock().await.installed.insert(key);
        }
        let expression = format!(
            "((document) => ({}))(globalThis.__regressifyDocument)",
            expression
        );
        let result = self
            .client
            .call(
                Some(&session),
                "Runtime.evaluate",
                json!({
                    "contextId": context, "expression": expression, "returnByValue": by_value,
                    "awaitPromise": true, "userGesture": true
                }),
            )
            .await?;
        check_evaluation(&result)?;
        Ok(result["result"].clone())
    }

    pub async fn target(&self, frame: &Option<Frame>, timeout: Duration) -> Result<Target> {
        tokio::time::timeout(timeout, self.resolve_target(frame, timeout))
            .await
            .map_err(|_| anyhow!("Frame resolution exceeded {}ms", timeout.as_millis()))?
    }

    async fn resolve_target(&self, frame: &Option<Frame>, timeout: Duration) -> Result<Target> {
        let selectors = match frame {
            Some(Frame::Single(selector)) => vec![selector.clone()],
            Some(Frame::Multiple(selectors)) if selectors.is_empty() => {
                bail!("Frame selector chain cannot be empty")
            }
            Some(Frame::Multiple(selectors)) => selectors.clone(),
            None => Vec::new(),
        };
        let started = std::time::Instant::now();
        loop {
            let mut realm = self.root().await?;
            let mut parents = Vec::new();
            let mut pending = None;
            for selector in &selectors {
                let expression = format!(
                    "document.querySelector({})",
                    serde_json::to_string(selector)?
                );
                let element = self.evaluate(&realm, &expression, false).await?;
                let Some(object) = element["objectId"].as_str() else {
                    pending = Some(selector);
                    break;
                };
                let (session, _) = self.context(&realm).await?;
                let described = self
                    .client
                    .call(
                        Some(&session),
                        "DOM.describeNode",
                        json!({"objectId": object}),
                    )
                    .await?;
                self.client
                    .call(
                        Some(&session),
                        "Runtime.releaseObject",
                        json!({"objectId": object}),
                    )
                    .await?;
                let node = &described["node"];
                anyhow::ensure!(
                    node["nodeName"] == "IFRAME" || node["nodeName"] == "FRAME",
                    "Frame selector '{}' must identify an iframe",
                    selector
                );
                let child = node["frameId"]
                    .as_str()
                    .or_else(|| node["contentDocument"]["frameId"].as_str());
                let Some(child) = child else {
                    pending = Some(selector);
                    break;
                };
                parents.push((realm.clone(), selector.clone()));
                realm = Realm {
                    session,
                    frame: child.to_string(),
                };
            }
            if let Some(selector) = pending {
                anyhow::ensure!(
                    started.elapsed() < timeout,
                    "Timed out waiting for frame '{}'",
                    selector
                );
                tokio::time::sleep(Duration::from_millis(100)).await;
                continue;
            }
            self.context(&realm).await?;
            return Ok(Target {
                runtime: self.clone(),
                realm,
                parents,
            });
        }
    }
}

fn check_evaluation(result: &Value) -> Result<()> {
    if let Some(details) = result.get("exceptionDetails") {
        bail!(
            "Browser evaluation failed: {}",
            details["exception"]["description"]
                .as_str()
                .or_else(|| details["text"].as_str())
                .unwrap_or("unknown JavaScript error")
        );
    }
    Ok(())
}

impl Target {
    pub(crate) async fn navigation_watch(&self) -> Result<NavigationWatch> {
        Ok(NavigationWatch {
            events: self.runtime.client.subscribe(),
            main_frame: self.runtime.root().await?.frame,
        })
    }

    pub(crate) async fn finish_navigation(
        &self,
        mut watch: NavigationWatch,
        timeout: Duration,
    ) -> Result<()> {
        let mut deadline = tokio::time::Instant::now() + Duration::from_millis(75);
        let mut started = false;
        loop {
            let event = match tokio::time::timeout_at(deadline, watch.events.recv()).await {
                Ok(Ok(event)) => event,
                Ok(Err(_)) => bail!("Navigation event stream closed or overflowed"),
                Err(_) if started => {
                    bail!("Timed out waiting for navigation triggered by the action")
                }
                Err(_) => return Ok(()),
            };
            let frame = event.params["frameId"]
                .as_str()
                .or_else(|| event.params["frame"]["id"].as_str());
            if frame != Some(self.realm.frame.as_str()) && frame != Some(watch.main_frame.as_str())
            {
                continue;
            }
            match event.method.as_str() {
                "Page.frameStartedLoading" | "Page.frameStartedNavigating" if !started => {
                    started = true;
                    deadline = tokio::time::Instant::now() + timeout;
                }
                "Page.navigatedWithinDocument" => return Ok(()),
                "Page.frameNavigated" => {
                    if frame == Some(watch.main_frame.as_str()) {
                        return self
                            .runtime
                            .target(&None, timeout)
                            .await?
                            .wait_for_load(timeout)
                            .await;
                    }
                    return self.wait_for_load(timeout).await;
                }
                _ => {}
            }
        }
    }

    pub async fn evaluate(&self, script: impl Into<Evaluation>) -> Result<Evaluated> {
        let expression = match script.into() {
            Evaluation::Expression(expression) => expression.expression,
            Evaluation::Function(function) => format!("({})()", function.function_declaration),
        };
        let result = self
            .runtime
            .evaluate(&self.realm, &expression, true)
            .await?;
        Ok(Evaluated(
            result.get("value").cloned().unwrap_or(Value::Null),
        ))
    }

    pub async fn execute<CommandType: Command>(
        &self,
        command: CommandType,
    ) -> Result<Response<CommandType::Response>> {
        let method = command.identifier();
        let params = serde_json::to_value(&command)?;
        let session = if method.starts_with("Input.") {
            self.runtime.main_session.clone()
        } else {
            self.runtime.context(&self.realm).await?.0
        };
        let method: &'static str = match method {
            std::borrow::Cow::Borrowed(method) => method,
            std::borrow::Cow::Owned(_) => bail!("CDP command identifier must be static"),
        };
        let result = self
            .runtime
            .client
            .call(Some(&session), method, params)
            .await?;
        Ok(Response {
            result: serde_json::from_value(result)?,
        })
    }

    pub async fn find_element(&self, selector: &str) -> Result<Element> {
        let result = self
            .runtime
            .evaluate(
                &self.realm,
                &format!(
                    "document.querySelector({})",
                    serde_json::to_string(selector)?
                ),
                false,
            )
            .await?;
        let object = result["objectId"]
            .as_str()
            .ok_or_else(|| anyhow!("Element '{}' is missing", selector))?;
        let (session, _) = self.runtime.context(&self.realm).await?;
        self.runtime
            .client
            .call(Some(&session), "DOM.getDocument", json!({}))
            .await?;
        let node = self
            .runtime
            .client
            .call(
                Some(&session),
                "DOM.requestNode",
                json!({"objectId": object}),
            )
            .await?;
        self.runtime
            .client
            .call(
                Some(&session),
                "Runtime.releaseObject",
                json!({"objectId": object}),
            )
            .await?;
        Ok(Element {
            node_id: serde_json::from_value(node["nodeId"].clone())?,
        })
    }

    pub async fn choose_files(
        &self,
        files: Vec<String>,
        horizontal: f64,
        vertical: f64,
        config: &super::actionability::ActionabilityConfig,
    ) -> Result<()> {
        let (session, _) = self.runtime.context(&self.realm).await?;
        let mut events = self.runtime.client.subscribe();
        let sessions = if session == self.runtime.main_session {
            vec![session]
        } else {
            vec![self.runtime.main_session.clone(), session]
        };
        for session in &sessions {
            self.runtime
                .client
                .call(
                    Some(session),
                    "Page.setInterceptFileChooserDialog",
                    json!({"enabled": true}),
                )
                .await?;
        }
        let result = async {
            super::actionability::cdp_click(self, horizontal, vertical, config).await?;
            let event =
                tokio::time::timeout(Duration::from_millis(config.element_timeout_ms), async {
                    loop {
                        let event = events.recv().await.map_err(|_| {
                            anyhow!("File chooser event stream closed or overflowed")
                        })?;
                        if event.method == "Page.fileChooserOpened"
                            && event.params["frameId"] == self.realm.frame
                        {
                            return Ok::<_, anyhow::Error>(event);
                        }
                    }
                })
                .await
                .map_err(|_| {
                    anyhow!("Timed out waiting for the upload control to open a file chooser")
                })??;
            anyhow::ensure!(
                event.params.get("backendNodeId").is_some(),
                "File chooser did not identify its input element"
            );
            self.runtime
                .client
                .call(
                    event.session_id.as_deref(),
                    "DOM.setFileInputFiles",
                    json!({
                        "files": files, "backendNodeId": event.params["backendNodeId"]
                    }),
                )
                .await?;
            Ok(())
        }
        .await;
        for session in &sessions {
            let disabled = self
                .runtime
                .client
                .call(
                    Some(session),
                    "Page.setInterceptFileChooserDialog",
                    json!({"enabled": false}),
                )
                .await;
            if result.is_ok() {
                disabled?;
            }
        }
        result
    }

    pub async fn goto(&self, url: &str) -> Result<()> {
        let (session, _) = self.runtime.context(&self.realm).await?;
        let result = self
            .runtime
            .client
            .call(
                Some(&session),
                "Page.navigate",
                json!({"url": url, "frameId": self.realm.frame}),
            )
            .await?;
        if let Some(error) = result["errorText"].as_str() {
            bail!("Navigation failed: {}", error);
        }
        Ok(())
    }

    pub(crate) async fn navigate(
        &self,
        url: &str,
        options: &crate::types::GotoParameters,
        default_timeout: u64,
    ) -> Result<()> {
        use crate::types::NavigationWait;
        let timeout = Duration::from_millis(
            options
                .timeout
                .filter(|value| *value > 0)
                .unwrap_or(default_timeout),
        );
        tokio::time::timeout(timeout, async {
            let (session, _) = self.runtime.context(&self.realm).await?;
            let wait = options.wait_until.unwrap_or_default();
            let mut events = self.runtime.client.subscribe();
            if wait == NavigationWait::NetworkIdle {
                self.runtime
                    .client
                    .call(Some(&session), "Network.enable", json!({}))
                    .await?;
            }
            let mut parameters = json!({"url": url, "frameId": self.realm.frame});
            if let Some(referer) = &options.referer {
                parameters["referrer"] = json!(referer);
            }
            let response = self
                .runtime
                .client
                .call(Some(&session), "Page.navigate", parameters)
                .await?;
            if let Some(error) = response["errorText"].as_str() {
                bail!("Navigation failed: {}", error);
            }
            match wait {
                NavigationWait::Commit => {}
                NavigationWait::DomContentLoaded => self.wait_for_ready(timeout).await?,
                NavigationWait::Load => self.wait_for_load(timeout).await?,
                NavigationWait::NetworkIdle => {
                    let mut pending = std::collections::HashSet::new();
                    loop {
                        let received =
                            tokio::time::timeout(Duration::from_millis(500), events.recv()).await;
                        match received {
                            Err(_) if pending.is_empty() => break,
                            Err(_) => continue,
                            Ok(Err(_)) => bail!("Network event stream closed or overflowed"),
                            Ok(Ok(event))
                                if event.session_id.as_deref() == Some(session.as_str()) =>
                            {
                                if let Some(request) = event.params["requestId"].as_str() {
                                    match event.method.as_str() {
                                        "Network.requestWillBeSent" => {
                                            pending.insert(request.to_string());
                                        }
                                        "Network.loadingFinished" | "Network.loadingFailed" => {
                                            pending.remove(request);
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                    self.wait_for_ready(timeout).await?;
                }
            }
            Ok::<_, anyhow::Error>(())
        })
        .await
        .map_err(|_| anyhow!("Navigation exceeded {}ms", timeout.as_millis()))?
    }

    pub async fn url(&self) -> Result<Option<String>> {
        Ok(Some(self.evaluate("location.href").await?.into_value()?))
    }

    pub async fn wait_for_ready(&self, timeout: Duration) -> Result<()> {
        self.wait_for_document("document.readyState !== 'loading'", timeout)
            .await
    }

    pub(crate) async fn wait_for_load(&self, timeout: Duration) -> Result<()> {
        self.wait_for_document("document.readyState === 'complete'", timeout)
            .await
    }

    async fn wait_for_document(&self, expression: &str, timeout: Duration) -> Result<()> {
        let started = std::time::Instant::now();
        loop {
            match self.evaluate(expression).await {
                Ok(result) if result.0 == true => return Ok(()),
                _ if started.elapsed() >= timeout => {
                    bail!("Timed out waiting for frame navigation readiness")
                }
                _ => tokio::time::sleep(Duration::from_millis(50)).await,
            }
        }
    }

    pub async fn scroll_frames(&self) -> Result<()> {
        for (realm, selector) in &self.parents {
            self.runtime.evaluate(realm, &format!("(() => {{ const element = document.querySelector({}); if (!element) throw new Error('Frame disappeared'); element.scrollIntoView({{block:'center', inline:'center', behavior:'instant'}}); }})()", serde_json::to_string(selector)?), true).await?;
        }
        Ok(())
    }

    pub async fn frame_offset(&self) -> Result<(f64, f64)> {
        self.project_point(0.0, 0.0, false).await
    }

    pub async fn check_frame_coverage(&self, horizontal: f64, vertical: f64) -> Result<()> {
        self.project_point(horizontal, vertical, true)
            .await
            .map(|_| ())
    }

    pub fn has_frames(&self) -> bool {
        !self.parents.is_empty()
    }

    async fn project_point(
        &self,
        mut horizontal: f64,
        mut vertical: f64,
        check: bool,
    ) -> Result<(f64, f64)> {
        for (realm, selector) in self.parents.iter().rev() {
            let expression = format!(
                r#"(() => {{
                const frame = document.querySelector({selector});
                if (!frame) throw new Error('Frame disappeared');
                const rectangle = frame.getBoundingClientRect();
                for (let ancestor = frame; ancestor; ancestor = ancestor.parentElement || ancestor.getRootNode().host) {{
                    const style = getComputedStyle(ancestor);
                    if (ancestor.inert || style.visibility === 'hidden' || style.display === 'none' || +style.opacity === 0)
                        throw new Error('Frame is hidden or inert: ' + {selector});
                }}
                if (!rectangle.width || !rectangle.height) throw new Error('Frame has zero size');
                const horizontal = rectangle.left + frame.clientLeft + {horizontal};
                const vertical = rectangle.top + frame.clientTop + {vertical};
                if ({check}) {{
                    const covering = document.elementFromPoint(horizontal, vertical);
                    if (covering !== frame && !frame.contains(covering))
                        throw new Error('Frame ' + {selector} + ' is obscured by ' + (covering ? covering.tagName + '#' + covering.id : 'viewport clipping'));
                }}
                return {{horizontal, vertical}};
            }})()"#,
                selector = serde_json::to_string(selector)?
            );
            let result = self.runtime.evaluate(realm, &expression, true).await?;
            horizontal = result["value"]["horizontal"]
                .as_f64()
                .ok_or_else(|| anyhow!("Invalid frame horizontal offset"))?;
            vertical = result["value"]["vertical"]
                .as_f64()
                .ok_or_else(|| anyhow!("Invalid frame vertical offset"))?;
        }
        Ok((horizontal, vertical))
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn context_cache_invalidates_after_navigation_or_event_overflow() {
        let (sender, receiver) = tokio::sync::broadcast::channel(1);
        let mut cache = ContextCache::new(receiver);
        let key = ("session".into(), "frame".into());
        cache.frames.insert(key.clone(), ("session".into(), 7));
        cache.installed.insert(("session".into(), 7));
        sender
            .send(super::super::cdp::Event {
                method: "Page.frameNavigated".into(),
                session_id: Some("session".into()),
                params: json!({}),
            })
            .unwrap();
        cache.drain();
        assert!(cache.frames.is_empty() && cache.installed.is_empty());
        cache.frames.insert(key, ("session".into(), 8));
        for _ in 0..2 {
            sender
                .send(super::super::cdp::Event {
                    method: "Runtime.consoleAPICalled".into(),
                    session_id: None,
                    params: json!({}),
                })
                .unwrap();
        }
        cache.drain();
        assert!(cache.frames.is_empty());
    }
    use super::*;

    #[tokio::test]
    #[ignore = "requires a locally installed Chromium browser"]
    async fn selectors_resolve_text_xpath_shadow_dom_and_cross_origin_frames() -> Result<()> {
        use super::super::browser_tests::Site;
        use tiny_http::{Header, Response};
        let inner = Site::new(|request| {
            let _ = request.respond(Response::from_string("<button id='inside'>Frame button</button><input type=file id=upload><button id=chooser onclick=\"document.querySelector('#hidden-file').click()\">Upload</button><input type=file id=hidden-file hidden><script>document.querySelector('#inside').onclick=()=>document.body.dataset.clicked='yes';</script>")
                .with_header(Header::from_bytes("Content-Type", "text/html").unwrap()));
        })?;
        let inner_url = inner.origin.replace("127.0.0.1", "localhost");
        let outer = Site::new(move |request| {
            let html = format!(
                "<button id=outside>Outside</button><div id=host></div><iframe src='{inner_url}'></iframe><script>document.querySelector('#host').attachShadow({{mode:'open'}}).innerHTML='<button>Shadow button</button>';</script>"
            );
            let _ = request.respond(
                Response::from_string(html)
                    .with_header(Header::from_bytes("Content-Type", "text/html").unwrap()),
            );
        })?;
        let (mut browser, handler, directory) =
            super::super::browsers::launch_browser(&None, "selectors", Default::default())
                .await
                .map_err(|error| anyhow!("{error}"))?;
        let result: Result<()> = async {
            let page = browser.new_page(outer.origin.as_str()).await?;
            let runtime = Runtime::new(&browser, &page).await?;
            let root = runtime.target(&None, Duration::from_secs(5)).await?;
            for selector in ["text=Shadow button", "button:has-text('Shadow')", "xpath=//button[@id='outside']", "css=#host >> text=Shadow button"] {
                let found: bool = root.evaluate(format!("!!document.querySelector({})", serde_json::to_string(selector)?)).await?.into_value()?;
                anyhow::ensure!(found, "selector not resolved: {selector}");
            }
            let frame = runtime.target(&Some(Frame::Single("iframe".into())), Duration::from_secs(5)).await?;
            let found: String = frame.evaluate("document.querySelector('text=Frame button').id").await?.into_value()?;
            anyhow::ensure!(found == "inside", "cross-origin frame did not resolve");
            frame.scroll_frames().await?;
            frame.check_frame_coverage(30.0, 20.0).await?;
            let config = super::super::actionability::ActionabilityConfig { element_timeout_ms: 2000, actionability_timeout_ms: 1000, poll_interval_ms: 25 };
            let position = super::super::actionability::ensure_actionable(&frame, "text=Frame button", &None, &config).await?;
            let offset = frame.frame_offset().await?;
            super::super::actionability::cdp_click(&frame, offset.0 + position.center_x, offset.1 + position.center_y, &config).await?;
            let clicked: String = frame.evaluate("document.body.dataset.clicked").await?.into_value()?;
            anyhow::ensure!(clicked == "yes", "cross-origin CDP click missed its element");
            let upload = tempfile::NamedTempFile::new()?;
            let files = vec![upload.path().to_string_lossy().to_string()];
            let element = frame.find_element("#upload").await?;
            let command = chromiumoxide::cdp::browser_protocol::dom::SetFileInputFilesParams::builder()
                .node_id(element.node_id).files(files.clone()).build().map_err(|error| anyhow!(error))?;
            frame.execute(command).await?;
            let count: usize = frame.evaluate("document.querySelector('#upload').files.length").await?.into_value()?;
            anyhow::ensure!(count == 1, "cross-origin direct file upload failed");
            let position = super::super::actionability::ensure_actionable(&frame, "#chooser", &None, &config).await?;
            frame.choose_files(files, offset.0 + position.center_x, offset.1 + position.center_y, &config).await?;
            let count: usize = frame.evaluate("document.querySelector('#hidden-file').files.length").await?.into_value()?;
            anyhow::ensure!(count == 1, "cross-origin button file chooser failed");
            frame.evaluate(format!("(() => {{ const child = document.createElement('iframe'); child.id = 'nested'; child.src = {}; document.body.appendChild(child); }})()", serde_json::to_string(&inner.origin)?)).await?;
            let nested = runtime.target(&Some(Frame::Multiple(vec!["iframe".into(), "#nested".into()])), Duration::from_secs(5)).await?;
            let nested_position = super::super::actionability::ensure_actionable(&nested, "#inside", &None, &config).await?;
            let nested_offset = nested.frame_offset().await?;
            super::super::actionability::cdp_click(&nested, nested_offset.0 + nested_position.center_x, nested_offset.1 + nested_position.center_y, &config).await?;
            let nested_clicked: String = nested.evaluate("document.body.dataset.clicked").await?.into_value()?;
            anyhow::ensure!(nested_clicked == "yes", "nested frame click missed");
            root.evaluate("(() => { const element = document.createElement('div'); element.id = 'later'; element.textContent = 'later'; element.style.display = 'none'; document.body.append(element); setTimeout(() => element.style.display = 'block', 200); })()").await?;
            let started = std::time::Instant::now();
            super::super::actionability::wait_for_selector(&root, "#later", &None, &config).await?;
            anyhow::ensure!(started.elapsed() >= Duration::from_millis(100), "selector wait returned before visibility");
            root.evaluate("(() => { const overlay = document.createElement('div'); overlay.id = 'overlay'; overlay.style.cssText = 'position:fixed;inset:0;z-index:99999;background:white'; document.body.append(overlay); })()").await?;
            let error = super::super::actionability::ensure_actionable(&frame, "#inside", &None, &config).await.err()
                .ok_or_else(|| anyhow!("parent overlay was not detected"))?;
            anyhow::ensure!(error.to_string().contains("obscured"), "unexpected overlay failure: {error}");
            root.evaluate("document.querySelector('#overlay').remove()").await?;
            frame.goto(&format!("{}/next", inner.origin)).await?;
            frame.wait_for_ready(Duration::from_secs(5)).await?;
            anyhow::ensure!(frame.url().await?.is_some_and(|url| url.ends_with("/next")), "iframe navigation did not target the frame");
            anyhow::ensure!(page.url().await?.as_deref() == Some(format!("{}/", outer.origin).as_str()), "iframe navigation changed the main page");
            Ok(())
        }.await;
        browser.close().await?;
        handler.await?;
        let _ = std::fs::remove_dir_all(directory);
        result
    }
}
