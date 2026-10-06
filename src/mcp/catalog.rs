use anyhow::Result;
use serde_json::{Map, Value, json};

use crate::{
    config::{WorkspaceConfig, replacements::ReplacementProfilesFile, viewports::ViewportDef},
    types::{TestAction, TestScenario, TestSuite},
};

pub(super) fn build() -> Result<Value> {
    let mut schemas = Map::new();
    for (kind, schema) in [
        ("workspace", schemars::schema_for!(WorkspaceConfig)),
        ("suite", schemars::schema_for!(TestSuite)),
        ("scenario", schemars::schema_for!(TestScenario)),
        ("action", schemars::schema_for!(TestAction)),
        ("viewports", schemars::schema_for!(Vec<ViewportDef>)),
        (
            "replacement-profiles",
            schemars::schema_for!(ReplacementProfilesFile),
        ),
    ] {
        schemas.insert(kind.into(), serde_json::to_value(schema)?);
    }
    let guides: Map<String, Value> = [
        (
            "overview",
            "Start here: authoring and safe workflows",
            include_str!("guides/overview.md"),
        ),
        (
            "suites",
            "Suite files, discovery, and identity",
            include_str!("guides/suites.md"),
        ),
        (
            "scenarios",
            "Scenarios, reuse, state, and capture selection",
            include_str!("guides/scenarios.md"),
        ),
        (
            "actions",
            "Browser action syntax and execution",
            include_str!("guides/actions.md"),
        ),
        (
            "workspace",
            "Workspace configuration and precedence",
            include_str!("guides/workspace.md"),
        ),
        (
            "files",
            "Predefined files, scripts, and generated artifacts",
            include_str!("guides/files.md"),
        ),
    ]
    .into_iter()
    .map(|(topic, title, content)| (topic.into(), json!({"title": title, "content": content})))
    .collect();

    let mut examples = Map::new();
    for (name, kind, path, description, value) in [
        (
            "minimal-suite",
            "suite",
            "visual_tests/home.tests.yaml",
            "A complete minimal suite. Replace the URL with an authorized test page.",
            json!({"scenarios": [{"id": "home", "label": "Home", "url": "https://example.com"}]}),
        ),
        (
            "responsive-checkout",
            "suite",
            "visual_tests/checkout.tests.yaml",
            "A complete suite using the phone and desktop labels from the viewports example. Replace selectors with verified page selectors.",
            json!({"displayName": "Checkout", "viewportNames": ["phone", "desktop"], "scenarios": [{"id": "checkout", "label": "Checkout form", "url": "https://example.com/checkout", "actions": [{"click": "#mobile-menu", "viewportNames": "phone"}, {"input": "#email", "value": "qa@example.com"}, {"check": "#terms"}, {"click": "#submit"}, {"wait": "#confirmation"}, {"save": "confirmation", "selector": "#confirmation"}]}]}),
        ),
        (
            "reuse-actions",
            "suite",
            "visual_tests/cart.tests.yaml",
            "needs replays dependency actions inside each consuming scenario; it is not shared browser state or scheduling.",
            json!({"scenarios": [{"id": "dismiss-consent", "url": "https://example.com/cart", "actions": [{"click": "#accept-cookies"}]}, {"id": "cart", "url": "https://example.com/cart", "needs": "dismiss-consent", "actions": [{"click": "#show-cart"}]}]}),
        ),
        (
            "scenario",
            "scenario",
            "scenarios[] inside a suite",
            "A scenario object to insert into a suite's scenarios list.",
            json!({"id": "product", "label": "Product details", "url": "https://example.com/products/sample", "readySelector": "main", "selector": "#product"}),
        ),
        (
            "workspace",
            "workspace",
            "regressify.yaml",
            "Workspace defaults. Viewport labels are defined separately in common/_viewports.yaml.",
            json!({"misMatchThreshold": 0.1, "pixelMatchThreshold": 5, "viewportNames": ["phone", "desktop"], "hideSelectors": [".timestamp"], "settleTimeout": 5000}),
        ),
        (
            "viewports",
            "viewports",
            "common/_viewports.yaml",
            "A top-level list, not an object with a viewports key. Sizes are CSS pixels.",
            json!([{"label": "phone", "width": 375, "height": 667}, {"label": "tablet", "width": 768, "height": 1024}, {"label": "desktop", "width": 1366, "height": 768}]),
        ),
        (
            "replacement-profiles",
            "replacement-profiles",
            "common/_replacement-profiles.yaml",
            "Reference captures use raw URLs. Test captures apply the selected profile, workspace rules, then suite rules.",
            json!({"profiles": {"default": [], "staging": [{"ref": "https://example.com", "test": "https://staging.example.com"}]}}),
        ),
        (
            "cookies",
            "cookies",
            "common/_cookies.yaml",
            "Synthetic consent cookie only. Keep real authentication cookies private; an empty list is also valid.",
            json!([{"name": "consent", "value": "accepted", "url": "https://example.com", "secure": true, "httpOnly": false, "sameSite": "Lax"}]),
        ),
        (
            "basic-auth",
            "workspace",
            "regressify.yaml",
            "Merge this field into the existing workspace configuration. Resolve these placeholders locally; never upload the environment values.",
            json!({"basicAuth": {"origin": "https://example.com", "username": "${REGRESSIFY_USER}", "password": "${REGRESSIFY_PASSWORD}"}}),
        ),
    ] {
        examples.insert(name.into(), json!({"kind": kind, "path": path, "description": description, "format": "yaml", "content": serde_yaml::to_string(&value)?}));
    }

    let actions = [
        ("check", json!({"check": "#terms"})),
        ("click", json!({"click": "#submit"})),
        ("focus", json!({"focus": "#email"})),
        ("goto", json!({"goto": "https://example.com/account"})),
        ("hide", json!({"hide": ".timestamp"})),
        ("hover", json!({"hover": "#navigation"})),
        (
            "input",
            json!({"input": "#email", "value": "qa@example.com"}),
        ),
        ("persist", json!({"persist": "signed-in"})),
        ("press", json!({"press": "#search", "key": "Enter"})),
        ("remove", json!({"remove": ".advertisement"})),
        (
            "save",
            json!({"save": "confirmation", "selector": "#confirmation"}),
        ),
        ("select", json!({"select": "#country", "value": "SE"})),
        ("scroll", json!({"scroll": "#footer"})),
        ("switchTab", json!({"switchTab": "new", "url": "**/quote"})),
        ("uncheck", json!({"uncheck": "#newsletter"})),
        ("wait", json!({"wait": 10000, "url": "**/dashboard"})),
    ];
    let mut action_names = Vec::new();
    for (name, value) in actions {
        action_names.push(name);
        examples.insert(format!("action-{name}"), json!({"kind": "action", "path": "actions[] inside a scenario", "description": "One action object, not a complete suite. Replace URLs and selectors with verified project values.", "format": "yaml", "content": serde_yaml::to_string(&value)?}));
    }
    for (name, kind, path, description, content) in [
        (
            "css-override",
            "css",
            "common/_override.css",
            "Hide only an explicitly designated nondeterministic region while preserving layout.",
            "[data-visual-volatile] { visibility: hidden !important; }\n",
        ),
        (
            "ready-script",
            "javascript",
            "common/_on-ready.js",
            "Browser-side JavaScript executed once per document before capture, not a Node hook.",
            "document.documentElement.dataset.regressifyReady = \"true\";\n",
        ),
    ] {
        examples.insert(name.into(), json!({"kind": kind, "path": path, "description": description, "format": kind, "content": content}));
    }
    examples.insert("minimal-suite-json".into(), json!({"kind": "suite", "path": "visual_tests/home.tests.json", "description": "JSON alternative to the minimal YAML suite. Do not create both files for the same suite.", "format": "json", "content": serde_json::to_string_pretty(&json!({"scenarios": [{"id": "home", "url": "https://example.com"}]}))?}));

    Ok(json!({
        "formatVersion": 1,
        "regressifyVersion": env!("CARGO_PKG_VERSION"),
        "instructions": "Read get_authoring_guide with topic overview before authoring Regressify files. Use topic guides, get_schema for exact fields, and get_example for synthetic templates. This server supplies versioned public guidance only; it cannot read or edit your workspace, run browsers, or approve baselines. Never send cookies, credentials, saved state, or private source. Inspect the user's local files with the client's own authorized tools and preserve unrelated edits.",
        "schemaKinds": schemas.keys().collect::<Vec<_>>(),
        "schemas": schemas,
        "guides": guides,
        "actions": action_names,
        "examples": examples
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_local_references(value: &Value, root: &Value) {
        match value {
            Value::Object(object) => {
                if let Some(reference) = object.get("$ref") {
                    let pointer = reference
                        .as_str()
                        .unwrap()
                        .strip_prefix('#')
                        .expect("Schemas must not depend on external files");
                    assert!(root.pointer(pointer).is_some(), "{pointer}");
                }
                for child in object.values() {
                    assert_local_references(child, root);
                }
            }
            Value::Array(items) => {
                for child in items {
                    assert_local_references(child, root);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn schemas_are_valid_and_all_references_are_local() {
        let catalog = build().unwrap();
        for schema in catalog["schemas"].as_object().unwrap().values() {
            jsonschema::validator_for(schema).unwrap();
            assert_local_references(schema, schema);
        }
    }

    #[test]
    fn examples_match_schemas_and_real_parsers() {
        let catalog = build().unwrap();
        for (name, example) in catalog["examples"].as_object().unwrap() {
            let kind = example["kind"].as_str().unwrap();
            if matches!(kind, "css" | "javascript") {
                continue;
            }
            let content = example["content"].as_str().unwrap();
            let value: Value = if example["format"] == "json" {
                serde_json::from_str(content).unwrap()
            } else {
                serde_yaml::from_str(content).unwrap()
            };
            if let Some(schema) = catalog["schemas"].get(kind) {
                let validator = jsonschema::validator_for(schema).unwrap();
                assert!(
                    validator.is_valid(&value),
                    "{name}: {:?}",
                    validator.iter_errors(&value).collect::<Vec<_>>()
                );
            }
            match kind {
                "workspace" => {
                    serde_json::from_value::<WorkspaceConfig>(value).unwrap();
                }
                "suite" => {
                    serde_json::from_value::<TestSuite>(value).unwrap();
                }
                "scenario" => {
                    serde_json::from_value::<TestScenario>(value).unwrap();
                }
                "action" => {
                    serde_json::from_value::<TestAction>(value).unwrap();
                }
                "viewports" => {
                    let viewports: Vec<ViewportDef> = serde_json::from_value(value).unwrap();
                    crate::config::viewports::validate_viewports(
                        &viewports,
                        std::path::Path::new("example"),
                    )
                    .unwrap();
                }
                "replacement-profiles" => {
                    let profiles: ReplacementProfilesFile = serde_json::from_value(value).unwrap();
                    for rules in profiles.profiles.values() {
                        crate::config::replacements::validate_replacements(rules, name).unwrap();
                    }
                }
                "cookies" => {
                    crate::test::capture::expand_cookie_entries(value.as_array().unwrap(), name)
                        .unwrap();
                }
                _ => panic!("Missing example validator for {kind}"),
            }
        }
    }

    #[test]
    fn every_action_has_a_typed_example() {
        let catalog = build().unwrap();
        let root = &catalog["schemas"]["action"];
        let variants = root["anyOf"].as_array().unwrap();
        let actions = catalog["actions"].as_array().unwrap();
        assert_eq!(variants.len(), actions.len());
        for action in actions {
            let name = action.as_str().unwrap();
            assert!(catalog["examples"].get(format!("action-{name}")).is_some());
            assert_eq!(
                variants
                    .iter()
                    .filter(|variant| {
                        let schema = variant
                            .get("$ref")
                            .and_then(Value::as_str)
                            .and_then(|reference| reference.strip_prefix('#'))
                            .and_then(|pointer| root.pointer(pointer))
                            .unwrap_or(variant);
                        schema["properties"].get(name).is_some()
                    })
                    .count(),
                1,
                "{name}"
            );
        }
    }

    #[test]
    fn guides_are_standalone_and_cover_runtime_exceptions() {
        let catalog = build().unwrap();
        for guide in catalog["guides"].as_object().unwrap().values() {
            let body = guide["content"].as_str().unwrap();
            assert!(body.starts_with("# "));
            for forbidden in ["../../../", "(@/", "{%", "AGENTS.md", "src/test/"] {
                assert!(!body.contains(forbidden), "{forbidden}");
            }
        }
        let workspace = catalog["guides"]["workspace"]["content"].as_str().unwrap();
        assert!(workspace.contains("profile, then workspace, then suite"));
        assert!(workspace.contains("same origin"));
        let scenarios = catalog["guides"]["scenarios"]["content"].as_str().unwrap();
        assert!(scenarios.contains("not scheduling"));
        assert!(scenarios.contains("last applicable"));
        let files = catalog["guides"]["files"]["content"].as_str().unwrap();
        for path in [
            "_viewports.yaml",
            "_cookies.yaml",
            "_replacement-profiles.yaml",
            "_override.css",
            "_on-ready.js",
        ] {
            assert!(files.contains(path), "{path}");
        }
    }

    #[test]
    fn schemas_reject_unknown_fields_and_ambiguous_actions() {
        let catalog = build().unwrap();
        for (kind, invalid) in [
            ("workspace", json!({"misMatchTreshold": 1})),
            ("suite", json!({"scenarios": [], "unknown": true})),
            (
                "scenario",
                json!({"url": "https://example.com", "selector": "main", "selectors": "viewport"}),
            ),
            ("action", json!({"click": "#submit", "hover": "#menu"})),
            (
                "viewports",
                json!([{"label": "phone", "width": 0, "height": 600}]),
            ),
        ] {
            assert!(
                !jsonschema::validator_for(&catalog["schemas"][kind])
                    .unwrap()
                    .is_valid(&invalid),
                "{kind}"
            );
        }
    }
}
