use std::{path::Path, time::Duration};

use gpui_kit::component::Root;
use gpui_kit::test::{TestAppContextExt, TestWindowExt};
use gpui_kit::{AppContext, Entity, Focusable, TestAppContext, WindowHandle, px, size};

use super::Workbench;
use crate::desktop::session::Selection;

async fn settle_dialog(cx: &mut TestAppContext, handle: WindowHandle<Root>) {
    // GPUI's dialog animation uses wall time, not the test executor's clock.
    std::thread::sleep(Duration::from_millis(300));
    cx.update_window(handle.into(), |_, window, cx| window.render_frame(cx))
        .unwrap();
}

fn editor(cx: &mut TestAppContext, root: &Path) -> (WindowHandle<Root>, Entity<Workbench>) {
    cx.update(gpui_kit::init);
    let mut created = None;
    let handle = cx.open_window(size(px(1280.), px(820.)), |window, cx| {
        let view = cx.new(|cx| Workbench::new(window, cx));
        view.update(cx, |this, cx| {
            this.session.connect(root).unwrap();
            let open = this
                .session
                .workspace
                .as_ref()
                .unwrap()
                .create_suite("ui-test")
                .unwrap();
            this.session.install_document(open).unwrap();
            this.select(Selection::Scenario(0), window, cx);
        });
        created = Some(view.clone());
        Root::new(view, window, cx)
    });
    (handle, created.unwrap())
}

#[gpui_kit::test]
async fn typing_a_guided_field_and_clicking_save_updates_the_file(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        assert!(window.find("field-url").visible());
        window.click("field-url", cx);
        window.press("ctrl-a", cx);
        window.input("https://shop.example/checkout", cx);
        assert_eq!(
            window.find("field-url").value(),
            Some("https://shop.example/checkout")
        );
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("status")
            .is_some_and(|status| status.label() == Some("Unsaved changes"))
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(view.read(cx).session.dirty());
        window.click("save", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("status")
            .is_some_and(|status| status.label() == Some("Saved"))
    })
    .await;
    let source =
        std::fs::read_to_string(directory.path().join("visual_tests/ui-test.tests.yaml")).unwrap();
    let value: serde_json::Value = serde_yaml::from_str(&source).unwrap();
    assert_eq!(
        value["scenarios"][0]["url"],
        "https://shop.example/checkout"
    );
    cx.update(|cx| assert!(!view.read(cx).session.dirty()));
}

#[gpui_kit::test]
fn duplicate_and_undo_buttons_update_the_actual_editor(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("duplicate-item", cx);
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert!(window.find("field-url").bounds().size.width > px(100.));
        window.click("undo", cx);
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert!(!view.read(cx).session.dirty());
    })
    .unwrap();
}

#[gpui_kit::test]
async fn invalid_source_exposes_a_diagnostic_and_save_does_not_write(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    let path = directory.path().join("visual_tests/ui-test.tests.yaml");
    let original = std::fs::read_to_string(&path).unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.within("editor-tabs").click(1_usize, cx);
        let focus = view.read(cx).source.read(cx).focus_handle(cx);
        window.focus(&focus, cx);
        window.render_frame(cx);
        window.press("ctrl-a", cx);
        window.input("scenarios: not-a-list", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window.try_find("diagnostic").is_some()
    })
    .await;
    cx.update_window(handle.into(), |_, window, cx| {
        assert!(window.find("diagnostic").visible());
        window.click("save", cx);
        assert!(view.read(cx).session.dirty());
        assert!(!view.read(cx).busy);
    })
    .unwrap();
    assert_eq!(std::fs::read_to_string(path).unwrap(), original);
}

#[gpui_kit::test]
async fn unsaved_dialog_cancel_and_discard_preserve_their_contracts(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("duplicate-item", cx);
        view.update(cx, |this, cx| {
            this.guard(super::files::Pending::Reload, window, cx)
        });
        window.render_frame(cx);
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("guard-cancel", cx);
        assert!(view.read(cx).session.dirty());
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"]
                .as_array()
                .unwrap()
                .len(),
            2
        );
    })
    .unwrap();
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.guard(super::files::Pending::Reload, window, cx)
        });
        window.render_frame(cx);
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("guard-discard", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("status")
            .is_some_and(|status| status.label() == Some("Document loaded"))
    })
    .await;
    cx.update(|cx| {
        assert!(!view.read(cx).session.dirty());
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    });
}

#[gpui_kit::test]
async fn new_suite_dialog_creates_a_real_file(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        view.update(cx, |this, cx| {
            this.guard(
                super::files::Pending::Name(super::files::Operation::New),
                window,
                cx,
            )
        });
        window.render_frame(cx);
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("file-name", cx);
        window.press("ctrl-a", cx);
        window.input("checkout.tests.json", cx);
        assert_eq!(
            window.find("file-name").value(),
            Some("checkout.tests.json")
        );
        window.click("file-confirm", cx);
    })
    .unwrap();
    cx.wait_for(handle.into(), Duration::from_secs(5), |window, _| {
        window
            .try_find("status")
            .is_some_and(|status| status.label() == Some("File saved"))
    })
    .await;
    let source =
        std::fs::read_to_string(directory.path().join("visual_tests/checkout.tests.json")).unwrap();
    let suite: crate::types::TestSuite = serde_json::from_str(&source).unwrap();
    assert_eq!(suite.scenarios.len(), 1);
    cx.update(|cx| assert_eq!(view.read(cx).session.entries.len(), 2));
}

#[gpui_kit::test]
async fn item_removal_requires_confirmation_and_is_undoable(cx: &mut TestAppContext) {
    let directory = tempfile::tempdir().unwrap();
    let (handle, view) = editor(cx, directory.path());
    cx.update_window(handle.into(), |_, window, cx| {
        window.render_frame(cx);
        window.click("remove-item", cx);
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    })
    .unwrap();
    settle_dialog(cx, handle).await;
    cx.update_window(handle.into(), |_, window, cx| {
        window.click("remove-confirm", cx);
        assert!(
            view.read(cx).session.value().unwrap()["scenarios"]
                .as_array()
                .unwrap()
                .is_empty(),
            "{}",
            view.read(cx).status
        );
        window.click("undo", cx);
        assert_eq!(
            view.read(cx).session.value().unwrap()["scenarios"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    })
    .unwrap();
}
