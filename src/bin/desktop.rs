#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use gpui_kit::component::{ActiveTheme, Root};
use gpui_kit::{prelude::*, *};

struct Workbench;

impl Render for Workbench {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .p_6()
            .gap_4()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(div().text_xl().child("Regressify"))
            .child("Workspaces")
    }
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(|cx| {
            gpui_kit::init(cx);
            let bounds = Bounds::centered(None, size(px(1280.), px(820.)), cx);
            cx.spawn(async move |cx| {
                let options = WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitlebarOptions {
                        title: Some("Regressify".into()),
                        ..Default::default()
                    }),
                    ..Default::default()
                };
                if let Err(error) = cx.open_window(options, |window, cx| {
                    let workbench = cx.new(|_| Workbench);
                    cx.new(|cx| Root::new(workbench, window, cx))
                }) {
                    eprintln!("Unable to open Regressify Desktop: {error:#}");
                }
            })
            .detach();
        });
}