//! Run settings stay available independently of which command was last executed.

use super::*;
use gpui_kit::component::{
    form::{Field as FormField, Form},
    group_box::{GroupBox, GroupBoxVariants},
};

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
            .gap_3()
            .p_3()
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
            let mut fields = Form::new().columns(2);
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
                            .disabled(disabled)
                            .into_any_element(),
                        Control::Textarea(input) => div()
                            .id(id)
                            .child(Textarea::new(input).disabled(disabled))
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
                let mut form_field =
                    FormField::new().when(boolean, |field| field.label_indent(false));
                if !boolean {
                    form_field = form_field.label(label).required(field.required);
                }
                if !field.description.is_empty() {
                    form_field = form_field.description(field.description.clone());
                }
                form_field = form_field
                    .child(control)
                    .children(self.errors.get(&field.key).map(|error| {
                        div()
                            .text_xs()
                            .text_color(cx.theme().danger)
                            .child(error.clone())
                    }));
                fields = fields.child(form_field.when(
                    matches!(field.key.as_str(), "regressifyDir" | "proxy"),
                    |field| field.col_span(2),
                ));
            }
            if count > 0 {
                form = form.child(
                    GroupBox::new()
                        .normal()
                        .id(format!("run-options-group-{index}"))
                        .title(name)
                        .child(fields),
                );
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
                    .flex()
                    .w_full()
                    .min_h(px(160.))
                    .child(
                        gpui_kit::component::empty::Empty::new()
                            .text_color(cx.theme().muted_foreground)
                            .child("No matching run settings"),
                    ),
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
                            .cleanable(true),
                    ),
            )
            .child(
                div()
                    .id("run-options-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scrollbar()
                    .child(form),
            )
            .into_any_element()
    }
}
