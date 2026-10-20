use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result, anyhow};
use tiny_http::{Header, Request, Response, Server, StatusCode};

use crate::types::BasicAuth;

use super::{
    authentication::Authentication,
    browsers::{LaunchOptions, prepare_browser},
};

pub(crate) async fn launch_browser(
    log_prefix: &str,
    options: LaunchOptions,
) -> Result<(chromiumoxide::Browser, tokio::task::JoinHandle<()>, PathBuf)> {
    let executable = prepare_browser(Some(false)).await?;
    super::browsers::launch_browser(&Some(executable), log_prefix, options)
        .await
        .map_err(|error| anyhow!("{error}"))
}

pub(crate) struct Site {
    pub origin: String,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Site {
    pub fn new(handler: impl Fn(Request) + Send + 'static) -> Result<Self> {
        let server = Server::http("127.0.0.1:0").map_err(|error| anyhow!("{error}"))?;
        let origin = format!("http://{}", server.server_addr());
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let thread = std::thread::spawn(move || {
            while !stopped.load(Ordering::Relaxed) {
                match server.recv_timeout(Duration::from_millis(100)) {
                    Ok(Some(request)) => handler(request),
                    Ok(None) => {}
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            origin,
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for Site {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn expanded_action_page(request: Request) {
    let _ = request.respond(Response::from_string(r#"<!doctype html><html><head>
<style>
body { margin:8px; font:14px sans-serif; }
button,input { width:95px; height:32px; margin:3px; }
.row { display:flex; gap:15px; margin:12px 0; }
#pointer,#native,#drop,#files { width:100px; height:65px; background:lightblue; }
#scroller { width:160px; height:60px; overflow:auto; }
#scroller > div { width:600px; height:600px; }
#touch { width:200px; height:90px; background:lightgreen; touch-action:none; }
#covered-wrap { position:relative; width:140px; height:40px; }
#covered { width:130px; margin:0; }
#cover { position:absolute; width:60px; height:40px; left:40px; top:0; background:gray; }
</style></head><body>
<div class="row"><button id="click">Click</button><button id="context">Context</button><a id="aux" href='#aux-link'>Aux link</a><button id="odd'id">Quoted</button>
<div id="covered-wrap"><button id="covered">Partial cover</button><div id="cover"></div></div></div>
<div><input id="first"><input id="second"><input id="typed"><input id="range" type="range" min="0" max="100" value="10"><input id="color" type="color" value='#000000'><input id="otp"><span id="copy">Alpha Beta Gamma</span></div>
<div class="row"><div id="pointer">Pointer source</div><div id="native" draggable="true">Native source</div><div id="drop">Drop target</div><div id="files">Files</div></div>
<div class="row"><div id="scroller"><div>Scrollable</div></div><div id="touch">Touch</div></div>
<button id="change">Change</button><span id="status">Loading</span><span id="busy">Busy</span><span id="gone">Remove</span>
<button id="delayed" disabled>Delayed</button><button id="disabled" disabled>Disabled</button><input id="value">
<div><button id="confirm">Confirm</button><button id="prompt">Prompt</button><button id="alert">Alert</button><button id="extra">Two dialogs</button><button id="noop">No dialog</button><button id="unload">Arm unload</button></div>
<script>
const body = document.body;
body.dataset.initialTouch = String(navigator.maxTouchPoints);
body.dataset.loads = String(Number(sessionStorage.loads || 0) + 1);
sessionStorage.loads = body.dataset.loads;
const get = id => document.getElementById(id);
get('click').addEventListener('dblclick', e => {
  body.dataset.double = String(e.isTrusted && e.detail === 2 && e.ctrlKey && e.shiftKey);
});
get('click').addEventListener('click', e => { body.dataset.modifiers = String(e.ctrlKey || e.shiftKey || e.altKey || e.metaKey); });
get('click').addEventListener('mousemove', e => {
  if (e.altKey && e.offsetX >= 5 && e.offsetX <= 20) body.dataset.hoverMod = String(e.isTrusted);
});
get('second').addEventListener('blur', () => { body.dataset.blurred = 'true'; });
get('range').addEventListener('change', () => { body.dataset.rangeChanged = get('range').value; });
get('color').addEventListener('change', () => { body.dataset.colorChanged = get('color').value; });
get('otp').addEventListener('paste', e => {
  e.preventDefault();
  body.dataset.customPaste = e.clipboardData.getData('text/plain');
});
document.addEventListener('selectionchange', () => {
  const selected = String(window.getSelection() || '');
  if (selected === 'Beta') body.dataset.selectedDom = 'Beta';
});
get('context').addEventListener('contextmenu', e => { e.preventDefault(); body.dataset.context = String(e.isTrusted && e.button === 2); });
get('aux').addEventListener('auxclick', e => { e.preventDefault(); body.dataset.aux = String(e.isTrusted && e.button === 1); });
get('covered').onclick = e => body.dataset.position = String(e.isTrusted && e.offsetX < 30);
get("odd'id").onclick = e => body.dataset.quoted = String(e.isTrusted);
const typing = [];
get('typed').onkeydown = e => { typing.push({time:e.timeStamp, trusted:e.isTrusted}); body.dataset.typing = JSON.stringify(typing); };
let pointer = false;
get('pointer').onpointerdown = e => { pointer = true; body.dataset.pointerStart = String(e.isTrusted); };
document.addEventListener('pointermove', e => { if (pointer && e.buttons === 1) body.dataset.pointerMoved = String(e.isTrusted); });
document.addEventListener('pointerup', e => { if (pointer) { body.dataset.pointerDrop = String(e.target === get('drop') && e.isTrusted); pointer = false; } });
get('native').ondragstart = e => { e.dataTransfer.setData('text/plain', 'native-payload'); body.dataset.dragStart = String(e.isTrusted); };
get('drop').ondragover = e => e.preventDefault();
get('drop').ondrop = e => { e.preventDefault(); body.dataset.nativeDrop = String(e.isTrusted && e.dataTransfer.getData('text/plain') === 'native-payload'); };
get('files').ondragover = e => e.preventDefault();
get('files').ondrop = async e => {
  e.preventDefault();
  const files = [...e.dataTransfer.files];
  body.dataset.fileDrop = String(e.isTrusted && files.length === 2 && await files[0].text() === 'first-file' && await files[1].text() === 'second-file');
};
get('scroller').onscroll = () => { get('scroller').dataset.moved = String(get('scroller').scrollTop > 0 && get('scroller').scrollLeft > 0); };
get('scroller').addEventListener('wheel', e => { body.dataset.wheelTrusted = String(e.isTrusted); });
let touchStart;
let pinchStartSpan = 0;
get('touch').addEventListener('touchstart', e => {
  touchStart = {time:e.timeStamp, x:e.touches[0].clientX};
  if (e.touches.length === 2) {
    pinchStartSpan = Math.abs(e.touches[1].clientX - e.touches[0].clientX);
  }
  body.dataset.touchStarted = 'true';
});
get('touch').addEventListener('touchmove', e => {
  if (e.touches.length === 2 && pinchStartSpan > 0) {
    const span = Math.abs(e.touches[1].clientX - e.touches[0].clientX);
    if (e.isTrusted && span > pinchStartSpan * 1.5) body.dataset.pinched = 'true';
  } else {
    body.dataset.swipe = String(e.isTrusted && Math.abs(e.touches[0].clientX - touchStart.x) >= 50);
  }
});
get('touch').addEventListener('touchend', e => {
  body.dataset.touchTrusted = String(e.isTrusted);
  body.dataset.touchDuration = String(e.timeStamp - touchStart.time);
});
get('touch').addEventListener('touchcancel', () => body.dataset.touchCancelled = 'true');
get('change').onclick = () => setTimeout(() => {
  get('status').textContent = '  Ready   now ';
  get('busy').style.display = 'none'; get('gone').remove();
  get('delayed').disabled = false; get('value').value = 'ready';
}, 150);
get('confirm').onclick = () => body.dataset.confirm = String(confirm('Continue?'));
get('prompt').onclick = () => body.dataset.prompt = String(prompt('Name?'));
get('alert').onclick = () => { alert('Notice'); body.dataset.alert = 'handled'; };
get('extra').onclick = () => { confirm('First'); confirm('Second'); };
get('unload').onclick = () => window.onbeforeunload = () => '';
</script></body></html>"#)
        .with_header(Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap()));
}

#[tokio::test]
#[ignore = "requires managed Chromium; isolated child owns fixture paths"]
async fn expanded_actions_dispatch_trusted_events_and_preserve_frames() -> Result<()> {
    use super::{
        runtime::Runtime,
        storage_state::StorageState,
        test_action::{ActionExecutionContext, exec_test_action},
    };
    use crate::{
        config::ResolvedConfig,
        types::{Frame, TestAction, TestScenario, TestSuite},
    };
    use serde_json::{Value, json};
    let root = match std::env::var_os("REGRESSIFY_EXPANDED_ACTION_FIXTURE") {
        Some(root) => PathBuf::from(root),
        None => {
            let workspace = tempfile::tempdir()?;
            let output = std::process::Command::new(std::env::current_exe()?)
                .args(["--exact", "test::browser_tests::expanded_actions_dispatch_trusted_events_and_preserve_frames", "--ignored", "--nocapture"])
                .env("REGRESSIFY_EXPANDED_ACTION_FIXTURE", workspace.path()).output()?;
            anyhow::ensure!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            return Ok(());
        }
    };
    crate::common::ROOT_DIR
        .set(root.clone())
        .map_err(|_| anyhow!("Fixture root already initialized"))?;
    std::fs::create_dir_all(root.join("visual_tests").join("uploads"))?;
    std::fs::write(
        root.join("visual_tests").join("uploads").join("first.txt"),
        "first-file",
    )?;
    std::fs::write(
        root.join("visual_tests").join("uploads").join("second.txt"),
        "second-file",
    )?;
    let files = ["first.txt", "second.txt"].map(|name| {
        PathBuf::from("uploads")
            .join(name)
            .to_string_lossy()
            .into_owned()
    });
    let outer = Site::new(expanded_action_page)?;
    let inner = Site::new(expanded_action_page)?;
    let (mut browser, handler, directory) = launch_browser(
        "expanded-actions",
        LaunchOptions {
            has_touch: true,
            viewport: Some((1100, 900)),
            ..Default::default()
        },
    )
    .await?;
    let result: Result<()> = async {
        let page = browser.new_page("about:blank").await?;
        let runtime = Runtime::new(&browser, &page).await?;
        let storage = Arc::new(StorageState::install_with_client(runtime.client(), &page, None, &outer.origin).await?);
        let touch_config = ResolvedConfig { element_timeout_ms: 4000, ..Default::default() };
        let touch_tabs = super::tabs::Tabs::install(&page, storage.clone(), &touch_config, None, true).await?;
        page.goto(outer.origin.as_str()).await?;
        let child = inner.origin.replace("127.0.0.1", "localhost");
        super::runtime::evaluate_main_document(&page, format!("(() => {{ const frame = document.createElement('iframe'); frame.id = 'frame'; frame.width = 1000; frame.height = 600; frame.src = {}; document.body.append(frame); }})()", serde_json::to_string(&child)?)).await?;
        let child_target = runtime.target(&Some(Frame::Single("#frame".into())), Duration::from_secs(4)).await?;
        super::actionability::wait_for_selector(&child_target, "body[data-initial-touch]", &None, &super::actionability::ActionabilityConfig { element_timeout_ms: 4000, ..Default::default() }, crate::diagnostics::record::Stage::Navigation).await?;
        child_target.evaluate(format!("(() => {{ const frame = document.createElement('iframe'); frame.id = 'nested'; frame.width = 950; frame.height = 580; frame.src = {}; document.body.append(frame); }})()", serde_json::to_string(&outer.origin)?)).await?;
        let suite = TestSuite::default();
        let scenario = TestScenario { url: outer.origin.clone(), ..Default::default() };
        let config = ResolvedConfig { element_timeout_ms: 4000, actionability_timeout_ms: 2000, ..Default::default() };
        let context = || ActionExecutionContext { page: &page, runtime: &runtime, storage: &storage, test_suite: &suite, scenario: &scenario, config: &config, url_replacements: &[], viewport_label: None, log_prefix: "expanded" };
        for frame in [None, Some(Frame::Single("#frame".into())), Some(Frame::Multiple(vec!["#frame".into(), "#nested".into()]))] {
            touch_tabs.check().await?;
            for mut value in [
                json!({"click":"#click","clickCount":2,"modifiers":["Control","Shift"]}),
                json!({"click":"#context","button":"right"}),
                json!({"click":"#aux","button":"middle"}),
                json!({"click":"#covered","position":{"x":10,"y":10}}),
                json!({"click":"[id=\"odd'id\"]"}),
                json!({"input":"#typed","value":"abc","typingDelay":35}),
                json!({"focus":"#first"}),
                json!({"key":"Tab"}),
                json!({"key":"x"}),
                json!({"drag":"#pointer","to":"#drop","mode":"pointer","duration":120}),
                json!({"drag":"#native","to":"#drop","duration":180}),
                json!({"dropFiles":"#files","file":files}),
                json!({"wait":"body[data-file-drop=true]"}),
                json!({"wheel":"#scroller","deltaY":100,"deltaX":50}),
                json!({"wait":"#scroller[data-moved=true]"}),
                json!({"scroll":"#scroller","position":{"x":20,"y":40}}),
                json!({"scroll":"#scroller","offset":{"x":5,"y":10}}),
                json!({"hover":"#click","position":{"x":10,"y":10},"modifiers":["Alt"]}),
                json!({"input":"#range","value":"75"}),
                json!({"input":"#color","value":"#ff00aa"}),
                json!({"paste":"#otp","value":"654321"}),
                json!({"paste":"#first","value":"hello"}),
                json!({"paste":"#first","value":" world","append":true}),
                json!({"selectText":"#first","start":0,"end":5}),
                json!({"selectText":"#copy","text":"Beta"}),
                json!({"focus":"#second"}),
                json!({"blur":"#second"}),
                json!({"focus":"#first"}),
                json!({"blur":"active"}),
                json!({"tap":"#touch"}),
                json!({"longPress":"#touch","duration":600}),
            ] {
                value["frame"] = json!(frame);
                let action: TestAction = serde_json::from_value(value.clone())?;
                exec_test_action(context(), &action).await.with_context(|| format!("expanded action {}", value.as_object().unwrap().keys().filter(|key| *key != "frame").cloned().collect::<Vec<_>>().join(",")))?;
            }
            let target = runtime.target(&frame, Duration::from_secs(4)).await?;
            exec_test_action(context(), &serde_json::from_value(json!({"dialog":"accept","kind":"confirm"}))?).await?;
            exec_test_action(context(), &serde_json::from_value(json!({"click":"#confirm","frame":frame}))?).await?;
            let values: Value = target.evaluate("({...document.body.dataset, first:document.querySelector('#first').value, second:document.querySelector('#second').value, typed:document.querySelector('#typed').value, scrollLeft:document.querySelector('#scroller').scrollLeft, scrollTop:document.querySelector('#scroller').scrollTop})").await?.into_value()?;
            for name in ["double", "context", "aux", "position", "quoted", "pointerStart", "pointerMoved", "pointerDrop", "dragStart", "nativeDrop", "fileDrop", "wheelTrusted", "touchTrusted", "confirm", "hoverMod", "blurred"] {
                anyhow::ensure!(values[name] == "true", "Missing trusted interaction result ({}): {name} = {}", json!(frame), values[name]);
            }
            anyhow::ensure!(values["first"] == "hello world" && values["second"] == "x" && values["typed"] == "abc", "Focused keys, paste, or typing changed the wrong field");
            anyhow::ensure!(values["rangeChanged"] == "75" && values["colorChanged"] == "#ff00aa", "Native range/color input values were not updated");
            anyhow::ensure!(values["customPaste"] == "654321" && values["selectedDom"] == "Beta", "Clipboard paste or DOM text selection did not run");
            anyhow::ensure!(values["scrollLeft"] == 25 && values["scrollTop"] == 50, "Scroll container position/offset was not applied: {:?}", values);
            anyhow::ensure!(values["initialTouch"].as_str().unwrap().parse::<u64>()? > 0, "Touch was not enabled before document scripts");
            anyhow::ensure!(values["touchDuration"].as_str().unwrap().parse::<f64>()? >= 550.0, "Long press released before its duration");
            let typing: Vec<Value> = serde_json::from_str(values["typing"].as_str().unwrap())?;
            let characters = &typing[typing.len() - 3..];
            anyhow::ensure!(characters.iter().all(|key| key["trusted"] == true), "Typing used synthetic key events");
            anyhow::ensure!(characters.windows(2).all(|pair| pair[1]["time"].as_f64().unwrap() - pair[0]["time"].as_f64().unwrap() >= 30.0), "typingDelay did not separate character events");
            for value in [
                json!({"swipe":"#touch","position":{"x":30,"y":30},"offset":{"x":70,"y":0},"duration":160,"frame":frame}),
                json!({"pinch":"#touch","scale":2.0,"duration":160,"frame":frame}),
                json!({"click":"#change","frame":frame}),
                json!({"wait":"#status","text":"Ready now","frame":frame}),
                json!({"wait":"#status","containsText":"Ready","frame":frame}),
                json!({"wait":"#busy","state":"hidden","frame":frame}),
                json!({"wait":"#gone","state":"detached","frame":frame}),
                json!({"wait":"#delayed","state":"enabled","frame":frame}),
                json!({"wait":"#disabled","state":"disabled","frame":frame}),
                json!({"wait":"#value","value":"ready","frame":frame}),
                json!({"wait":"#busy","state":"attached","frame":frame}),
            ] {
                exec_test_action(context(), &serde_json::from_value(value)?).await?;
            }
            let swiped: String = target.evaluate("document.body.dataset.swipe").await?.into_value()?;
            anyhow::ensure!(swiped == "true", "Swipe did not move a trusted touch point");
            let pinched: String = target.evaluate("document.body.dataset.pinched").await?.into_value()?;
            anyhow::ensure!(pinched == "true", "Pinch did not move two trusted touch points");
            let cancelling: TestAction = serde_json::from_value(json!({"longPress":"#touch","duration":5000,"frame":frame}))?;
            anyhow::ensure!(tokio::time::timeout(Duration::from_millis(800), exec_test_action(context(), &cancelling)).await.is_err(), "Cancellation fixture did not interrupt the hold");
            exec_test_action(context(), &serde_json::from_value(json!({"wait":"body[data-touch-cancelled=true]","frame":frame}))?).await?;
            exec_test_action(context(), &serde_json::from_value(json!({"click":"#click","frame":frame}))?).await?;
            let released: String = target.evaluate("document.body.dataset.modifiers").await?.into_value()?;
            anyhow::ensure!(released == "false", "Gesture modifiers remained held");
        }
        let short = ResolvedConfig { element_timeout_ms: 500, actionability_timeout_ms: 300, ..Default::default() };
        for value in [
            json!({"click":"#covered","position":{"x":500,"y":10}}),
            json!({"tap":"#disabled"}),
            json!({"wheel":"#missing","deltaY":10}),
            json!({"swipe":"#touch","offset":{"x":10000,"y":0}}),
            json!({"wait":"#status","text":"never"}),
            json!({"wait":"#busy","state":"visible"}),
            json!({"blur":"#second"}),
            json!({"selectText":"#copy","text":"Nonexistent"}),
            json!({"input":"#range","value":"250"}),
        ] {
            let context = ActionExecutionContext { config: &short, ..context() };
            anyhow::ensure!(exec_test_action(context, &serde_json::from_value(value)?).await.is_err(), "Invalid/unsatisfied interaction succeeded");
        }
        Ok(())
    }.await;
    browser.close().await?;
    tokio::time::timeout(Duration::from_secs(10), browser.wait()).await??;
    handler.await?;
    drop(browser);
    let cleanup = std::fs::remove_dir_all(directory);
    result?;
    cleanup?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires managed Chromium; loopback history and real JavaScript dialogs"]
async fn expanded_actions_dialogs_and_history_are_strict() -> Result<()> {
    use super::{
        runtime::Runtime,
        storage_state::StorageState,
        test_action::{ActionExecutionContext, exec_test_action},
    };
    use crate::{
        config::ResolvedConfig,
        types::{TestAction, TestScenario, TestSuite},
    };
    use serde_json::{Value, json};
    let site = Site::new(expanded_action_page)?;
    let (mut browser, handler, directory) =
        launch_browser("expanded-dialogs", Default::default()).await?;
    let result: Result<()> = async {
        let page = browser.new_page("about:blank").await?;
        let runtime = Runtime::new(&browser, &page).await?;
        let storage = StorageState::install(&browser, &page, None, &site.origin).await?;
        let suite = TestSuite::default();
        let scenario = TestScenario {
            url: site.origin.clone(),
            ..Default::default()
        };
        let config = ResolvedConfig {
            element_timeout_ms: 2500,
            actionability_timeout_ms: 1500,
            ..Default::default()
        };
        let context = || ActionExecutionContext {
            page: &page,
            runtime: &runtime,
            storage: &storage,
            test_suite: &suite,
            scenario: &scenario,
            config: &config,
            url_replacements: &[],
            viewport_label: None,
            log_prefix: "dialogs",
        };
        let back: TestAction = serde_json::from_value(json!({"navigate":"back"}))?;
        anyhow::ensure!(
            exec_test_action(context(), &back).await.is_err(),
            "Missing history must fail"
        );
        page.goto(format!("{}/one", site.origin)).await?;
        for value in [
            json!({"dialog":"accept","kind":"confirm","message":"Continue?"}),
            json!({"click":"#confirm"}),
            json!({"dialog":"accept","kind":"prompt","promptText":"fixture response"}),
            json!({"click":"#prompt"}),
            json!({"dialog":"dismiss","kind":"alert","message":"Notice"}),
            json!({"click":"#alert"}),
        ] {
            exec_test_action(context(), &serde_json::from_value(value)?).await?;
        }
        let target = runtime.target(&None, Duration::from_secs(3)).await?;
        let values: Value = target
            .evaluate("({...document.body.dataset})")
            .await?
            .into_value()?;
        anyhow::ensure!(
            values["confirm"] == "true"
                && values["prompt"] == "fixture response"
                && values["alert"] == "handled",
            "Dialog outcomes were incorrect"
        );
        for (expectation, trigger) in [
            (
                json!({"dialog":"accept","kind":"prompt"}),
                json!({"click":"#confirm"}),
            ),
            (
                json!({"dialog":"accept","kind":"confirm","message":"Wrong message"}),
                json!({"click":"#confirm"}),
            ),
            (
                json!({"dialog":"accept","kind":"confirm"}),
                json!({"click":"#extra"}),
            ),
            (
                json!({"dialog":"accept","kind":"confirm"}),
                json!({"click":"#noop"}),
            ),
        ] {
            exec_test_action(context(), &serde_json::from_value(expectation)?).await?;
            anyhow::ensure!(
                tokio::time::timeout(
                    Duration::from_secs(6),
                    exec_test_action(context(), &serde_json::from_value(trigger)?)
                )
                .await?
                .is_err(),
                "Unmet/extra dialog expectation succeeded"
            );
        }
        for value in [
            json!({"goto":format!("{}/two", site.origin)}),
            json!({"navigate":"back"}),
            json!({"wait":0,"url":format!("{}/one", site.origin)}),
            json!({"navigate":"forward"}),
            json!({"wait":0,"url":format!("{}/two", site.origin)}),
            json!({"click":"#unload"}),
            json!({"dialog":"dismiss","kind":"beforeunload"}),
            json!({"goto":format!("{}/cancelled", site.origin)}),
            json!({"wait":0,"url":format!("{}/two", site.origin)}),
            json!({"dialog":"accept","kind":"beforeunload"}),
            json!({"navigate":"reload","ignoreCache":true}),
            json!({"wait":"#click","url":format!("{}/two", site.origin)}),
        ] {
            exec_test_action(context(), &serde_json::from_value(value)?).await?;
        }
        runtime.ensure_no_pending_dialog()?;
        Ok(())
    }
    .await;
    browser.close().await?;
    tokio::time::timeout(Duration::from_secs(10), browser.wait()).await??;
    handler.await?;
    drop(browser);
    let cleanup = std::fs::remove_dir_all(directory);
    result?;
    cleanup?;
    Ok(())
}

#[tokio::test]
#[ignore = "requires managed Chromium; run regressify install"]
async fn reference_and_test_inject_ready_scripts_without_node() -> Result<()> {
    use crate::{test::capture, types::ScreenshotType};
    use clap::Parser;
    use serde_json::{Value, json};

    if let Some(root) = std::env::var_os("REGRESSIFY_BROWSER_HOOK_FIXTURE") {
        let mode = std::env::var("REGRESSIFY_BROWSER_HOOK_MODE")?;
        let command = if mode == "test" { "test" } else { "ref" };
        let arguments: Vec<std::ffi::OsString> = vec![
            "regressify".into(),
            "--root".into(),
            root,
            "--run-id".into(),
            mode.into(),
            "--async-capture-limit".into(),
            "2".into(),
            "--async-capture-ramp-ms".into(),
            "0".into(),
            command.into(),
            "--fetch-browser".into(),
            "false".into(),
        ];
        return crate::application::execute(crate::cli::Cli::try_parse_from(arguments)?).await;
    }
    let site = Site::new(|request| {
        let _ = request.respond(Response::from_string(r#"<!doctype html>
<html><head><script>window.beforeObserved = window.beforeCalls === 1;</script>
<style>body { margin:0; min-height:100vh; background:rgb(200,30,30); }
#cookie-banner { position:fixed; inset:0; z-index:1000; background:white; }
button { margin:30px; } .volatile { position:fixed; inset:0; background:red; }</style></head>
<body><div class="volatile">Changing data</div><div id="cookie-banner">Cookie banner</div>
<button id="target" onclick="
const context = window.regressify;
const id = context.scenario.id;
const expectedShared = id === 'scenario-ready' ? 0 : 1;
const expectedReady = id === 'js-path' ? 0 : 1;
if (window.beforeObserved && (window.sharedCalls || 0) === expectedShared &&
    (window.readyCalls || 0) === expectedReady && !document.querySelector('#cookie-banner') &&
    getComputedStyle(document.querySelector('.volatile')).visibility === 'hidden' &&
    context.viewport.label === 'desktop' && typeof context.isReference === 'boolean' &&
    !('actions' in context.scenario) && !('basicAuth' in context.scenario) &&
    !('basicAuth' in context.config) && typeof process === 'undefined' && typeof require === 'undefined') {
    document.body.style.background = 'rgb(14,160,80)';
}">Continue</button></body></html>"#)
            .with_header(Header::from_bytes("Content-Type", "text/html").unwrap()));
    })?;
    let workspace = tempfile::tempdir()?;
    let root = workspace.path();
    std::fs::create_dir(root.join("visual_tests"))?;
    std::fs::create_dir(root.join("common"))?;
    std::fs::write(
        root.join("common/_viewports.yaml"),
        "- label: desktop\n  width: 640\n  height: 240\n",
    )?;
    std::fs::write(
        root.join("common/_override.css"),
        ".volatile { visibility: hidden !important; }",
    )?;
    std::fs::write(
        root.join("common/before.js"),
        "window.beforeCalls = (window.beforeCalls || 0) + 1;",
    )?;
    for (path, counter) in [("shared.js", "sharedCalls"), ("ready.js", "readyCalls")] {
        std::fs::write(
            root.join("common").join(path),
            format!(
                "(async () => {{ await new Promise(resolve => setTimeout(resolve, 25)); window.{counter} = (window.{counter} || 0) + 1; document.querySelector('#cookie-banner')?.remove(); }})()"
            ),
        )?;
    }
    std::fs::write(
        root.join("regressify.yaml"),
        "elementTimeout: 1500\npostInteractionWait: 0\n",
    )?;
    let mut scenarios = Vec::new();
    for id in ["js-path", "scenario-ready", "both"] {
        let mut scenario = json!({
            "id": id, "url": site.origin, "readySelector": "#target",
            "onBeforeScript": "common/before.js", "delay": 0, "noScrollTop": true,
            "selectors": ["viewport"], "actions": [{"click": "#target"}]
        });
        if id != "scenario-ready" {
            scenario["jsOnReadyPath"] = json!("common/shared.js");
        }
        if id != "js-path" {
            scenario["onReadyScript"] = json!("common/ready.js");
        }
        scenarios.push(scenario);
    }
    std::fs::write(
        root.join("visual_tests/hooks.tests.json"),
        serde_json::to_vec_pretty(&json!({"id": "hooks", "scenarios": scenarios}))?,
    )?;
    let run = |mode: &str| -> Result<()> {
        let output = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--ignored",
                "--exact",
                "test::browser_tests::reference_and_test_inject_ready_scripts_without_node",
                "--nocapture",
            ])
            .env("REGRESSIFY_BROWSER_HOOK_FIXTURE", root)
            .env("REGRESSIFY_BROWSER_HOOK_MODE", mode)
            .env("PATH", "")
            .env_remove("NODE_PATH")
            .env_remove("REPLACEMENT_PROFILE")
            .output()?;
        anyhow::ensure!(
            output.status.success(),
            "{mode}: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    };
    run("reference")?;
    run("test")?;
    let suite_dir = root.join(".regressify/hooks");
    let mut baselines = Vec::new();
    for id in ["js-path", "scenario-ready", "both"] {
        let key =
            capture::final_screenshot_order_key(id, Some("desktop"), &ScreenshotType::Viewport);
        let baseline = suite_dir
            .join("bitmaps_reference")
            .join(format!("{key}.png"));
        baselines.push((baseline.clone(), std::fs::read(&baseline)?));
        for mode in ["reference", "test"] {
            let path = suite_dir
                .join("bitmaps_test")
                .join(mode)
                .join(format!("{key}.png"));
            anyhow::ensure!(
                !capture::error_marker_path(&path).exists(),
                "{mode}/{id} failed"
            );
            let image = lodepng::decode32(std::fs::read(path)?)?;
            let pixel = image.buffer.first().unwrap();
            anyhow::ensure!(
                (pixel.r, pixel.g, pixel.b) == (14, 160, 80),
                "{mode}/{id}: banner dismissal or hook order failed"
            );
        }
    }
    std::fs::write(
        root.join("common/ready.js"),
        "throw new Error('ready-hook-fixture-error')",
    )?;
    run("failed-reference")?;
    let report: Value = serde_json::from_slice(&std::fs::read(
        suite_dir.join("bitmaps_test/failed-reference/report.json"),
    )?)?;
    let tests = report["tests"].as_array().unwrap();
    assert_eq!(
        tests.iter().filter(|test| test["status"] == "fail").count(),
        2
    );
    assert_eq!(
        tests.iter().filter(|test| test["status"] == "pass").count(),
        1
    );
    for (path, bytes) in baselines {
        assert_eq!(
            std::fs::read(path)?,
            bytes,
            "a failing hook changed a reference"
        );
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires managed Chromium; run regressify install"]
async fn responsive_hamburger_menu_mobile_only_click_and_save() -> Result<()> {
    use crate::{
        diagnostics::{
            journal::{ArchiveState, JournalReader},
            record::{Field, FieldValue, SafeIdentity},
        },
        test::capture,
        types::ScreenshotType,
    };
    use clap::Parser;
    use serde_json::{Value, json};
    use std::{collections::BTreeMap, sync::Mutex};

    if let Some(root) = std::env::var_os("REGRESSIFY_RESPONSIVE_VIEWPORT_FIXTURE") {
        let arguments: Vec<std::ffi::OsString> = vec![
            "regressify".into(),
            "--root".into(),
            root,
            "--run-id".into(),
            "responsive".into(),
            "--async-capture-limit".into(),
            "2".into(),
            "--async-capture-ramp-ms".into(),
            "0".into(),
            "test".into(),
            "--fetch-browser".into(),
            "false".into(),
            "--skip-compare".into(),
        ];
        return crate::application::execute(crate::cli::Cli::try_parse_from(arguments)?).await;
    }
    let observed = Arc::new(Mutex::new(BTreeMap::<String, Value>::new()));
    let received = observed.clone();
    let site = Site::new(move |request| {
        let url = reqwest::Url::parse(&format!("http://fixture{}", request.url())).unwrap();
        if url.path() == "/observed" {
            let values = url.query_pairs().into_owned().collect::<BTreeMap<_, _>>();
            received.lock().unwrap().insert(
                values["viewport"].clone(),
                serde_json::from_str(&values["state"]).unwrap(),
            );
            let _ = request.respond(Response::empty(204));
            return;
        }
        let _ = request.respond(Response::from_string(r#"<!doctype html>
<html><head><style>
body { margin: 0; min-height: 100vh; background: rgb(200, 30, 30); }
button { position: absolute; left: 24px; top: 24px; }
#products-menu { top: 64px; } #target { top: 104px; }
</style></head><body>
<button id="products-menu">Products</button><button id="target" hidden>Widget</button>
<script>
const mobile = innerWidth < 600;
const path = [];
const products = document.getElementById('products-menu');
const target = document.getElementById('target');
products.hidden = mobile;
if (mobile) {
    const menu = document.createElement('button');
    menu.id = 'menu-toggle';
    menu.textContent = 'Menu';
    menu.onclick = () => {
        path.push('menu');
        products.hidden = false;
        document.body.style.background = 'rgb(40, 90, 200)';
    };
    document.body.appendChild(menu);
}
products.onclick = () => { path.push('products'); target.hidden = false; };
target.onclick = async () => {
    path.push('target');
    const state = { path, menuExists: !!document.getElementById('menu-toggle') };
    await fetch('/observed?viewport=' + (mobile ? 'mobile' : 'desktop') + '&state=' + encodeURIComponent(JSON.stringify(state)));
    document.body.style.background = 'rgb(14, 160, 80)';
};
</script></body></html>"#).with_header(Header::from_bytes("Content-Type", "text/html").unwrap()));
    })?;
    let workspace = tempfile::tempdir()?;
    let root = workspace.path();
    std::fs::create_dir_all(root.join("visual_tests"))?;
    std::fs::create_dir_all(root.join("common"))?;
    std::fs::write(
        root.join("common/_viewports.yaml"),
        "- label: desktop\n  width: 960\n  height: 300\n- label: mobile\n  width: 390\n  height: 300\n",
    )?;
    std::fs::write(root.join("regressify.yaml"), "elementTimeout: 1500\n")?;
    let suite = json!({ "scenarios": [{
        "id": "products-submenu", "url": site.origin, "viewportNames": ["desktop", "mobile"],
        "delay": 0, "noScrollTop": true, "selectors": ["viewport"],
        "actions": [
            { "click": "#menu-toggle", "viewportNames": "mobile" },
            { "save": "menu-open", "viewportNames": ["mobile"] },
            { "click": "#products-menu" },
            { "click": "#target" }
        ]
    }] });
    std::fs::write(
        root.join("visual_tests/responsive.tests.json"),
        serde_json::to_vec_pretty(&suite)?,
    )?;
    let output = std::process::Command::new(std::env::current_exe()?)
        .args([
            "--ignored",
            "--exact",
            "test::browser_tests::responsive_hamburger_menu_mobile_only_click_and_save",
            "--nocapture",
        ])
        .env("REGRESSIFY_RESPONSIVE_VIEWPORT_FIXTURE", root)
        .env_remove("REPLACEMENT_PROFILE")
        .env_remove("REGRESSIFY_DISABLE_DIAGNOSTIC_ARCHIVE")
        .env_remove("REGRESSIFY_DIAGNOSTIC_DISK_BUDGET_BYTES")
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        observed.lock().unwrap().get("desktop"),
        Some(&json!({ "path": ["products", "target"], "menuExists": false }))
    );
    assert_eq!(
        observed.lock().unwrap().get("mobile"),
        Some(&json!({ "path": ["menu", "products", "target"], "menuExists": true }))
    );
    let output_dir = root.join(".regressify");
    let run = output_dir.join("responsive/bitmaps_test/responsive");
    let mut expected = Vec::new();
    for (label, width) in [("desktop", 960), ("mobile", 390)] {
        let checkpoint_key = capture::final_screenshot_order_key(
            "products-submenu",
            Some(label),
            &ScreenshotType::Step("menu-open".into()),
        );
        let checkpoint = run.join(format!("{checkpoint_key}.png"));
        if label == "mobile" {
            expected.push(checkpoint_key);
            let image = lodepng::decode32(std::fs::read(&checkpoint)?)?;
            let pixel = image.buffer.first().unwrap();
            assert_eq!((pixel.r, pixel.g, pixel.b), (40, 90, 200));
            assert!(!capture::error_marker_path(&checkpoint).exists());
        } else {
            assert!(!checkpoint.exists());
            assert!(!capture::error_marker_path(&checkpoint).exists());
        }
        let final_key = capture::final_screenshot_order_key(
            "products-submenu",
            Some(label),
            &ScreenshotType::Viewport,
        );
        let final_image = run.join(format!("{final_key}.png"));
        expected.push(final_key);
        let image = lodepng::decode32(std::fs::read(&final_image)?)?;
        assert_eq!((image.width, image.height), (width, 300));
        let pixel = image.buffer.first().unwrap();
        assert_eq!((pixel.r, pixel.g, pixel.b), (14, 160, 80), "{label}");
        assert!(!capture::error_marker_path(&final_image).exists());
        let log = capture::read_capture_log(&output_dir, &final_image).unwrap();
        assert_eq!(log.state, ArchiveState::Complete);
        let archive = log.archive.unwrap();
        let reader = JournalReader::open(&output_dir, &archive.operation_id)?;
        let menu_target = FieldValue::Identity(SafeIdentity::source("#menu-toggle"));
        let mut menu_dispatched = false;
        let mut execution_seen = false;
        reader.visit(|record| {
            if let Some(execution) = &record.execution {
                execution_seen = true;
                assert_eq!(execution.suite_id.as_str(), "responsive");
                assert_eq!(execution.scenario_id.as_str(), "products-submenu");
                assert_eq!(execution.viewport.name.as_str(), label);
            }
            for value in record.fields.values() {
                if let FieldValue::Identity(source) = value {
                    assert!(!source.as_str().starts_with("sha256:"));
                }
            }
            menu_dispatched |= record.fields.get(&Field::Target) == Some(&menu_target);
        })?;
        assert!(
            execution_seen,
            "capture records must retain readable execution names"
        );
        assert_eq!(
            menu_dispatched,
            label == "mobile",
            "{label}: unexpected menu dispatch"
        );
    }
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(run.join(capture::REPORT_ORDER_FILE))?)?;
    assert_eq!(manifest["captures"], json!(expected));
    assert_eq!(
        crate::compare::report_gen::validate_capture_run_inventory(&run)?.len(),
        3
    );
    Ok(())
}

fn auth_site(
    expected: &'static str,
    credentials_seen: Arc<AtomicBool>,
    redirect: Option<String>,
) -> Result<Site> {
    Site::new(move |request| {
        if request.url() == "/redirect"
            && let Some(location) = &redirect
        {
            let response = Response::from_string("")
                .with_status_code(StatusCode(302))
                .with_header(Header::from_bytes("Location", location.as_str()).unwrap());
            let _ = request.respond(response);
            return;
        }
        let authorization = request
            .headers()
            .iter()
            .find(|header| header.field.equiv("Authorization"));
        if authorization.is_some() {
            credentials_seen.store(true, Ordering::Relaxed);
        }
        let response = if authorization.is_some_and(|header| header.value.as_str() == expected) {
            Response::from_string("<body>authenticated</body>")
        } else {
            Response::from_string("<body>unauthorized</body>")
                .with_status_code(StatusCode(401))
                .with_header(
                    Header::from_bytes("WWW-Authenticate", "Basic realm=\"fixture\"").unwrap(),
                )
        };
        let _ =
            request.respond(response.with_header(
                Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
            ));
    })
}

#[tokio::test]
#[ignore = "requires managed Chromium; run regressify install"]
async fn basic_auth_is_origin_scoped_and_cancels_bad_credentials() -> Result<()> {
    let allowed = Arc::new(AtomicBool::new(false));
    let other_allowed = Arc::new(AtomicBool::new(false));
    let forbidden = Arc::new(AtomicBool::new(false));
    let third = auth_site("Basic dXNlcjpwYXNz", forbidden.clone(), None)?;
    let first = auth_site(
        "Basic dXNlcjpwYXNz",
        allowed.clone(),
        Some(third.origin.clone()),
    )?;
    let second = auth_site("Basic b3RoZXI6cGFzcw==", other_allowed.clone(), None)?;
    let (mut browser, handler, directory) =
        launch_browser("fixture", LaunchOptions::default()).await?;
    let result: Result<()> = async {
        let page = browser.new_page("about:blank").await?;
        let entries = [
            BasicAuth {
                origin: first.origin.clone(),
                username: "user".into(),
                password: "pass".into(),
            },
            BasicAuth {
                origin: second.origin.clone(),
                username: "other".into(),
                password: "pass".into(),
            },
        ];
        let authentication = Authentication::install(&browser, &page, &entries)
            .await?
            .unwrap();
        for site in [&first, &second] {
            page.goto(site.origin.as_str()).await?;
            let text: String = page
                .evaluate("document.body.textContent")
                .await?
                .into_value()?;
            anyhow::ensure!(
                text == "authenticated",
                "unexpected authentication response: {text}"
            );
        }
        let _ =
            tokio::time::timeout(Duration::from_secs(10), page.goto(third.origin.as_str())).await?;
        let _ = tokio::time::timeout(
            Duration::from_secs(10),
            page.goto(format!("{}/redirect", first.origin)),
        )
        .await?;
        anyhow::ensure!(
            !forbidden.load(Ordering::Relaxed),
            "credentials leaked to an unconfigured origin"
        );
        anyhow::ensure!(
            allowed.load(Ordering::Relaxed),
            "first origin did not authenticate"
        );
        anyhow::ensure!(
            other_allowed.load(Ordering::Relaxed),
            "second origin did not authenticate"
        );
        authentication.check()?;
        let wrong_page = browser.new_page("about:blank").await?;
        let _wrong_authentication = Authentication::install(
            &browser,
            &wrong_page,
            &[BasicAuth {
                origin: third.origin.clone(),
                username: "wrong".into(),
                password: "wrong".into(),
            }],
        )
        .await?;
        let _ = tokio::time::timeout(
            Duration::from_secs(10),
            wrong_page.goto(third.origin.as_str()),
        )
        .await?;
        Ok(())
    }
    .await;
    browser.close().await?;
    handler.await?;
    let _ = std::fs::remove_dir_all(directory);
    result
}

fn interaction_page(request: Request) {
    let content = match request.url() {
        "/done" => "<body>done<script src='/slow.js'></script></body>",
        "/slow.js" => {
            std::thread::sleep(Duration::from_millis(150));
            "document.body.dataset.loaded = 'yes';"
        }
        _ => {
            r#"<input id="typed" maxlength="3"><select id="choice"><option value="old">Old</option></select>
            <a id="next" href="/done">Next</a><script>
            const typed = document.querySelector('#typed');
            const keys = [];
            typed.addEventListener('keydown', event => { keys.push(event.key); typed.dataset.keys = JSON.stringify(keys); typed.dataset.trusted = String(event.isTrusted); });
            setTimeout(() => document.querySelector('#choice').add(new Option('Ready', 'ready')), 600);
            </script>"#
        }
    };
    let mime = if request.url() == "/slow.js" {
        "application/javascript"
    } else {
        "text/html"
    };
    let _ = request.respond(
        Response::from_string(content)
            .with_header(Header::from_bytes("Content-Type", mime).unwrap()),
    );
}

#[tokio::test]
#[ignore = "requires managed Chromium; run regressify install"]
async fn navigation_default_allows_slow_document_responses() -> Result<()> {
    let site = Site::new(|request| {
        match request.url() {
            "/slow" => std::thread::sleep(Duration::from_secs(31)),
            "/timeout" => std::thread::sleep(Duration::from_millis(250)),
            _ => {}
        }
        let _ = request.respond(
            Response::from_string("<body>ready</body>")
                .with_header(Header::from_bytes("Content-Type", "text/html").unwrap()),
        );
    })?;
    let (mut browser, handler, directory) =
        launch_browser("navigation-timeout", LaunchOptions::default()).await?;
    let result: Result<()> = async {
        let page = browser.new_page("about:blank").await?;
        let runtime = super::runtime::Runtime::new(&browser, &page).await?;
        let target = runtime.target(&None, Duration::from_secs(5)).await?;
        target
            .navigate(&format!("{}/slow", site.origin), &Default::default())
            .await?;
        let loaded: bool = target
            .evaluate("document.readyState === 'complete'")
            .await?
            .into_value()?;
        anyhow::ensure!(loaded, "navigation returned before the document loaded");

        let error = target
            .navigate(
                &format!("{}/timeout", site.origin),
                &crate::types::GotoParameters {
                    timeout: Some(25),
                    ..Default::default()
                },
            )
            .await
            .expect_err("explicit navigation timeout must remain bounded");
        let message = error.to_string();
        anyhow::ensure!(
            message == "Navigation exceeded 25ms"
                || message == "Timed out waiting for CDP Page.navigate",
            "unexpected navigation timeout error: {message}"
        );
        Ok(())
    }
    .await;
    browser.close().await?;
    handler.await?;
    let _ = std::fs::remove_dir_all(directory);
    result
}

#[tokio::test]
#[ignore = "requires managed Chromium; run regressify install"]
async fn navigation_options_wait_for_load_and_network_idle() -> Result<()> {
    let site = Site::new(interaction_page)?;
    let (mut browser, handler, directory) =
        launch_browser("navigation-options", LaunchOptions::default()).await?;
    let result: Result<()> = async {
        let page = browser.new_page("about:blank").await?;
        let runtime = super::runtime::Runtime::new(&browser, &page).await?;
        let target = runtime.target(&None, Duration::from_secs(5)).await?;
        for wait in [
            crate::types::NavigationWait::Load,
            crate::types::NavigationWait::NetworkIdle,
        ] {
            target
                .navigate(
                    &format!("{}/done", site.origin),
                    &crate::types::GotoParameters {
                        wait_until: Some(wait),
                        timeout: Some(5000),
                        referer: Some(format!("{}/source", site.origin)),
                    },
                )
                .await?;
            let loaded: bool = target
                .evaluate("document.body.dataset.loaded === 'yes'")
                .await?
                .into_value()?;
            anyhow::ensure!(
                loaded,
                "navigation returned before external script completed"
            );
        }
        Ok(())
    }
    .await;
    browser.close().await?;
    handler.await?;
    let _ = std::fs::remove_dir_all(directory);
    result
}

#[tokio::test]
#[ignore = "requires managed Chromium; run regressify install"]
async fn action_keyboard_selection_and_navigation_work_in_frames() -> Result<()> {
    use super::{
        runtime::Runtime,
        storage_state::StorageState,
        test_action::{ActionExecutionContext, exec_test_action},
    };
    use crate::{
        config::ResolvedConfig,
        types::{Frame, TestAction, TestScenario, TestSuite},
    };
    let inner = Site::new(interaction_page)?;
    let outer = Site::new(interaction_page)?;
    let (mut browser, handler, directory) =
        launch_browser("interactions", LaunchOptions::default()).await?;
    let result: Result<()> = async {
        let page = browser.new_page("about:blank").await?;
        let runtime = Runtime::new(&browser, &page).await?;
        let storage = StorageState::install(&browser, &page, None, &outer.origin).await?;
        page.goto(outer.origin.as_str()).await?;
        page.evaluate(format!("(() => {{ const child = document.createElement('iframe'); child.src = {}; document.body.append(child); }})()", serde_json::to_string(&inner.origin.replace("127.0.0.1", "localhost"))?)).await?;
        let suite = TestSuite::default();
        let scenario = TestScenario { url: outer.origin.clone(), ..Default::default() };
        let config = ResolvedConfig { element_timeout_ms: 4000, actionability_timeout_ms: 2000, ..Default::default() };
        let context = || ActionExecutionContext { page: &page, runtime: &runtime, storage: &storage, test_suite: &suite, scenario: &scenario, config: &config, url_replacements: &[], viewport_label: None, log_prefix: "fixture" };
        for frame in [None, Some(Frame::Single("iframe".into()))] {
            for action in [
                serde_json::json!({"select":"#choice", "value":"ready", "frame":frame}),
                serde_json::json!({"input":"#typed", "value":"Ab+c", "append":true, "frame":frame}),
            ] {
                let action: TestAction = serde_json::from_value(action)?;
                exec_test_action(context(), &action).await?;
            }
            let target = runtime.target(&frame, Duration::from_secs(4)).await?;
            let expected_url = if frame.is_some() {
                format!("{}/", inner.origin.replace("127.0.0.1", "localhost"))
            } else {
                format!("{}/", outer.origin)
            };
            anyhow::ensure!(target.url().await?.as_deref() == Some(expected_url.as_str()), "URL read did not use the selected frame");
            let action: TestAction = serde_json::from_value(serde_json::json!({"wait":"#typed", "url":expected_url, "frame":frame}))?;
            exec_test_action(context(), &action).await?;
            let values: serde_json::Value = target.evaluate("(() => { const input = document.querySelector('#typed'); return {value:input.value, keys:JSON.parse(input.dataset.keys), trusted:input.dataset.trusted, choice:document.querySelector('#choice').value}; })()").await?.into_value()?;
            anyhow::ensure!(values["value"] == "Ab+", "maxlength was bypassed: {values}");
            anyhow::ensure!(values["keys"] == serde_json::json!(["A", "b", "+", "c"]), "typing did not dispatch per-character key events: {values}");
            anyhow::ensure!(values["trusted"] == "true" && values["choice"] == "ready", "trusted typing or delayed options failed");
            for key in ["Shift", "ControlOrMeta+A", "Backspace", "+"] {
                let action: TestAction = serde_json::from_value(serde_json::json!({"press":"#typed", "key":key, "frame":frame}))?;
                exec_test_action(context(), &action).await?;
            }
            let value: String = target.evaluate("document.querySelector('#typed').value").await?.into_value()?;
            anyhow::ensure!(value == "+", "keyboard shortcut or literal plus failed: {value}");
        }
        let action: TestAction = serde_json::from_value(serde_json::json!({"click":"#next"}))?;
        exec_test_action(context(), &action).await?;
        let loaded: String = page.evaluate("document.body.dataset.loaded").await?.into_value()?;
        anyhow::ensure!(loaded == "yes", "click completed before the navigation loaded");
        Ok(())
    }.await;
    browser.close().await?;
    handler.await?;
    let _ = std::fs::remove_dir_all(directory);
    result
}

#[tokio::test]
#[ignore = "requires managed Chromium; run regressify install"]
async fn url_waits_follow_delayed_redirects_and_same_document_navigation() -> Result<()> {
    use super::{
        runtime::Runtime,
        storage_state::StorageState,
        test_action::{ActionExecutionContext, exec_test_action},
    };
    use crate::{
        config::ResolvedConfig,
        types::{TestAction, TestScenario, TestSuite},
    };

    let destination = Site::new(|request| {
        let content = if request.url() == "/callback" {
            "<body><script>setTimeout(() => location.replace('/done?flow=login#signed-in'), 150);</script></body>"
        } else {
            "<body><div id='signed-in'>Signed in</div></body>"
        };
        let _ =
            request.respond(Response::from_string(content).with_header(
                Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
            ));
    })?;
    let destination_origin = destination.origin.replace("127.0.0.1", "localhost");
    let callback_url = format!("{destination_origin}/callback");
    let source = Site::new(move |request| {
        let response = if request.url() == "/authorize" {
            Response::from_string("")
                .with_status_code(StatusCode(302))
                .with_header(Header::from_bytes("Location", callback_url.as_str()).unwrap())
        } else {
            Response::from_string(
                "<body><button id='login' onclick=\"setTimeout(() => location.assign('/authorize'), 250)\">Log in</button></body>",
            )
        };
        let _ =
            request.respond(response.with_header(
                Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap(),
            ));
    })?;
    let (mut browser, handler, directory) =
        launch_browser("url-waits", LaunchOptions::default()).await?;
    let result: Result<()> = async {
        let page = browser.new_page("about:blank").await?;
        let runtime = Runtime::new(&browser, &page).await?;
        let storage = StorageState::install(&browser, &page, None, &source.origin).await?;
        page.goto(source.origin.as_str()).await?;
        let suite = TestSuite::default();
        let scenario = TestScenario {
            url: source.origin.clone(),
            ..Default::default()
        };
        let config = ResolvedConfig {
            element_timeout_ms: 6000,
            actionability_timeout_ms: 2000,
            ..Default::default()
        };
        let context = || ActionExecutionContext {
            page: &page,
            runtime: &runtime,
            storage: &storage,
            test_suite: &suite,
            scenario: &scenario,
            config: &config,
            url_replacements: &[],
            viewport_label: None,
            log_prefix: "fixture",
        };
        for action in [
            serde_json::json!({"click": "#login"}),
            serde_json::json!({
                "wait": "#signed-in",
                "url": format!("{destination_origin}/done?flow=login#signed-in")
            }),
        ] {
            let action: TestAction = serde_json::from_value(action)?;
            exec_test_action(context(), &action).await?;
        }
        let target = runtime.target(&None, Duration::from_secs(5)).await?;
        target
            .evaluate("history.pushState({}, '', '?flow=spa#ready')")
            .await?;
        let action: TestAction = serde_json::from_value(serde_json::json!({
            "wait": "#signed-in",
            "url": format!("{destination_origin}/done?flow=spa#ready")
        }))?;
        exec_test_action(context(), &action).await?;

        let timeout_config = ResolvedConfig {
            element_timeout_ms: 300,
            ..Default::default()
        };
        let action: TestAction = serde_json::from_value(serde_json::json!({
            "wait": 0,
            "url": format!("{destination_origin}/never")
        }))?;
        let error = tokio::time::timeout(
            Duration::from_secs(3),
            exec_test_action(
                ActionExecutionContext {
                    config: &timeout_config,
                    ..context()
                },
                &action,
            ),
        )
        .await?
        .err()
        .ok_or_else(|| anyhow!("URL wait unexpectedly matched a different destination"))?;
        anyhow::ensure!(
            error.to_string().contains("Timeout waiting for URL"),
            "unexpected URL wait error: {error}"
        );
        Ok(())
    }
    .await;
    browser.close().await?;
    handler.await?;
    let _ = std::fs::remove_dir_all(directory);
    result
}
