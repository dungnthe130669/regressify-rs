//! Run settings stay available independently of which command was last executed.

use super::*;

fn group(key: &str) -> usize {
    match key {
        "siteEnv" | "regressifyDir" | "runId" => 0,
        "asyncCaptureLimit" | "asyncCaptureRampMs" | "fetchBrowser" | "debug" | "requiredLogin"
        | "replacementProfile" | "skipCompare" => 1,
        "misMatchThreshold" | "asyncCompareLimit" => 2,
        "title" | "heading" | "subheading" => 3,
        "speed" | "minSizeKb" => 4,
        _ => 5,
    }
}

fn label(field: &Field) -> String {
    match field.key.as_str() {
        "siteEnv" => "Environment",
        "regressifyDir" => "Output folder",
        "runId" => "Run name (optional)",
        "fetchBrowser" => "Browser download",
        "debug" => "Show browser",
        "skipCompare" => "Compare after test",
        "asyncCaptureLimit" => "Concurrent captures",
        "asyncCaptureRampMs" => "Capture ramp interval (ms)",
        "asyncCompareLimit" => "Concurrent comparisons",
        "speed" => "Optimization speed",
        "minSizeKb" => "Minimum image size (KB)",
        _ => return field.label.clone(),
    }
    .into()
}

impl RunPanel {
    pub(super) fn render_options(&self, cx: &mut Context<Self>) -> AnyElement {
        let disabled = self.running() || self.disabled;
        let query = self.settings_search.read(cx).value().trim().to_lowercase();
        let mut form = div()
            .flex()
            .flex_col()
            .gap_5()
            .p_5()
            .w_full()
            .max_w(px(980.));
        let mut total = 0;
        for (index, name) in [
            "Environment & output",
            "Capture",
            "Comparison",
            "Report",
            "Image optimization",
            "Diagnostics & network",
        ]
        .into_iter()
        .enumerate()
        {
            let mut fields = div().grid().grid_cols(2).gap_4();
            let mut count = 0;
            for binding in self
                .bindings
                .iter()
                .filter(|binding| group(&binding.field.key) == index)
            {
                let field = &binding.field;
                let label = label(field);
                if !query.is_empty()
                    && ![
                        label.as_str(),
                        field.key.as_str(),
                        field.description.as_str(),
                        name,
                    ]
                    .iter()
                    .any(|text| text.to_lowercase().contains(&query))
                {
                    continue;
                }
                count += 1;
                let id = SharedString::from(format!("run-field-{}", field.key));
                let control = if field.key == "fetchBrowser" {
                    let selected = match self.options[&field.key].as_bool() {
                        None => 0,
                        Some(true) => 1,
                        Some(false) => 2,
                    };
                    div()
                        .id("browser-download-control")
                        .test_support()
                        .child(
                            TabBar::new(id)
                                .segmented()
                                .small()
                                .selected_index(selected)
                                .on_click(cx.listener(move |this, index, _, cx| {
                                    if this.running() || this.disabled {
                                        return;
                                    }
                                    this.options["fetchBrowser"] = match index {
                                        1 => json!(true),
                                        2 => json!(false),
                                        _ => Value::Null,
                                    };
                                    this.confirmation = false;
                                    cx.notify();
                                }))
                                .child(Tab::new().label("Auto"))
                                .child(Tab::new().label("Always"))
                                .child(Tab::new().label("Never")),
                        )
                        .into_any_element()
                } else {
                    match &binding.control {
                        Control::Input(input) => Input::new(input)
                            .id(id)
                            .small()
                            .disabled(disabled)
                            .into_any_element(),
                        Control::Textarea(input) => div()
                            .id(id)
                            .child(Textarea::new(input).disabled(disabled).h(px(72.)))
                            .into_any_element(),
                        _ => {
                            let descriptor = field.clone();
                            let inverted = field.key == "skipCompare";
                            let checked =
                                self.options[&field.key].as_bool().unwrap_or(false) != inverted;
                            Checkbox::new(id)
                                .label(label.clone())
                                .checked(checked)
                                .disabled(disabled)
                                .on_change(cx.listener(move |this, checked, _, cx| {
                                    this.change(
                                        &descriptor,
                                        if *checked != inverted {
                                            "true"
                                        } else {
                                            "false"
                                        },
                                        cx,
                                    );
                                }))
                                .into_any_element()
                        }
                    }
                };
                let boolean = field.kind == FieldKind::Boolean && field.key != "fetchBrowser";
                fields = fields.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .min_w_0()
                        .when(
                            matches!(field.key.as_str(), "regressifyDir" | "proxy"),
                            |field| field.col_span(2),
                        )
                        .when(!boolean, |row| {
                            row.child(div().text_sm().font_weight(FontWeight::MEDIUM).child(label))
                        })
                        .child(control)
                        .children(self.errors.get(&field.key).map(|error| {
                            div()
                                .text_xs()
                                .text_color(cx.theme().danger)
                                .child(error.clone())
                        })),
                );
            }
            if count > 0 {
                form = form
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_3()
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(name))
                            .child(div().flex_1().h(px(1.)).bg(cx.theme().border)),
                    )
                    .child(fields);
            }
            total += count;
        }
        if total == 0 {
            form = form.child(
                div()
                    .id("run-settings-empty")
                    .role(Role::Status)
                    .aria_label("No matching run settings")
                    .test_support()
                    .text_color(cx.theme().muted_foreground)
                    .child("No matching run settings"),
            );
        }
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                div()
                    .px_5()
                    .py_3()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(
                        Input::new(&self.settings_search)
                            .id("run-settings-search")
                            .prefix(IconName::Search)
                            .cleanable(true)
                            .small(),
                    ),
            )
            .child(
                div()
                    .id("run-options-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(form),
            )
            .into_any_element()
    }
}
