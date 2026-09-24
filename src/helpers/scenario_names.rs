use std::collections::{HashMap, HashSet};

use percent_encoding::percent_decode_str;
use reqwest::Url;

pub(crate) struct ScenarioName<'a> {
    pub id: &'a str,
    pub label: Option<&'a str>,
    pub url: Option<&'a str>,
}

impl<'a> ScenarioName<'a> {
    pub fn with_capture_label(mut self, label: &'a str, key: &str) -> Self {
        if self.label.is_none_or(|label| label.trim().is_empty())
            && !generated_capture_label(label, key)
            && Some(label) != self.url
        {
            self.label = Some(label);
        }
        self
    }

    pub fn captured(
        id: &'a str,
        display_name: Option<&'a str>,
        label: Option<&'a str>,
        reference_url: Option<&'a str>,
        test_url: Option<&'a str>,
    ) -> Self {
        Self {
            id,
            // Recorded display names distinguish explicit URL labels from old URL fallbacks.
            label: display_name
                .filter(|name| !name.trim().is_empty())
                .or_else(|| {
                    label.filter(|label| {
                        !label.trim().is_empty()
                            && Some(*label) != reference_url
                            && Some(*label) != test_url
                    })
                }),
            url: reference_url
                .filter(|url| !url.trim().is_empty())
                .or(test_url),
        }
    }
}

struct NameParts {
    explicit: Option<String>,
    origin: Option<String>,
    path: Vec<String>,
    detail: String,
    depth: usize,
}

impl NameParts {
    fn new(scenario: &ScenarioName<'_>) -> Self {
        let url = scenario
            .url
            .and_then(|url| Url::parse(url).ok())
            .filter(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some());
        let route = url.as_ref().and_then(|url| {
            let fragment = url.fragment()?.strip_prefix('!').unwrap_or(url.fragment()?);
            fragment
                .starts_with('/')
                .then(|| Url::parse(&format!("https://scenario.invalid{fragment}")).ok())?
        });
        let page = route.as_ref().or(url.as_ref());
        let mut path = page
            .and_then(Url::path_segments)
            .into_iter()
            .flatten()
            .filter(|segment| !segment.is_empty())
            .map(|segment| percent_decode_str(segment).decode_utf8_lossy().into_owned())
            .collect::<Vec<_>>();
        if path.last().is_some_and(|segment| {
            matches!(
                segment.to_ascii_lowercase().as_str(),
                "index.html" | "index.htm" | "index.php" | "default.aspx"
            )
        }) {
            path.pop();
        }
        // Bound work on untrusted report URLs, including unusually deep paths.
        if path.len() > 32 {
            path.drain(..path.len() - 32);
        }
        let mut path = path
            .into_iter()
            .map(|segment| humanize(&segment))
            .collect::<Vec<_>>();
        path.retain(|segment| !segment.is_empty());
        if path.is_empty() {
            path.push(
                if url.is_some() {
                    "Home"
                } else {
                    "Unnamed scenario"
                }
                .into(),
            );
        }
        let query = page.map(query_detail).unwrap_or_default();
        let mut detail = if query.is_empty() {
            String::new()
        } else {
            format!("?{query}")
        };
        if route.is_none()
            && let Some(url) = url.as_ref()
            && let Some(fragment) = url.fragment().filter(|fragment| !fragment.is_empty())
        {
            let fragment = percent_decode_str(fragment).decode_utf8_lossy();
            let fragment = if fragment.contains('=') {
                let mut parameters = url.clone();
                parameters.set_query(Some(&fragment));
                query_detail(&parameters)
            } else {
                clean_text(&fragment)
            };
            if !fragment.is_empty() {
                detail.push('#');
                detail.push_str(&fragment);
            }
        }
        Self {
            explicit: scenario
                .label
                .filter(|label| !label.trim().is_empty())
                .map(str::to_owned),
            origin: url.map(|url| url.origin().ascii_serialization()),
            path,
            detail: bounded(&detail),
            depth: 1,
        }
    }

    fn title(&self, include_origin: bool) -> String {
        if let Some(label) = &self.explicit {
            return label.clone();
        }
        let path = self.path[self.path.len() - self.depth..].join(" / ");
        match &self.origin {
            Some(origin) if include_origin => format!("{origin} / {path}"),
            _ => path,
        }
    }
}

fn clean_text(text: &str) -> String {
    text.chars()
        .filter(|character| {
            !character.is_control()
                && !matches!(character, '\u{200b}'..='\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn bounded(text: &str) -> String {
    let mut characters = text.chars();
    let prefix = characters.by_ref().take(120).collect::<String>();
    if characters.next().is_some() {
        format!("{prefix}...")
    } else {
        prefix
    }
}

fn humanize(text: &str) -> String {
    let text = text
        .rsplit_once('.')
        .filter(|(_, extension)| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "html" | "htm" | "php" | "aspx"
            )
        })
        .map_or(text, |(stem, _)| stem);
    let text = bounded(&clean_text(&text.replace(['-', '_'], " ")));
    let mut characters = text.chars();
    characters
        .next()
        .map(|first| first.to_uppercase().collect::<String>() + characters.as_str())
        .unwrap_or_default()
}

pub(crate) fn generated_capture_label(label: &str, key: &str) -> bool {
    label.trim().is_empty()
        || label.contains("__")
        || label == key.replace('_', " ")
        || label == key.replace("__", " ")
        || label
            == key
                .replace('_', " ")
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
}

pub(crate) fn capture_details(
    key: &str,
    scenario_id: Option<&str>,
    selector: &str,
) -> (String, String) {
    let (viewport, target) = scenario_id
        .filter(|id| !id.is_empty())
        .and_then(|id| key.strip_prefix(id))
        .map(|suffix| {
            if let Some(target) = suffix.strip_prefix("__") {
                ("", target)
            } else {
                suffix
                    .strip_prefix('_')
                    .unwrap_or(suffix)
                    .split_once("__")
                    .unwrap_or(("", ""))
            }
        })
        .unwrap_or_else(|| {
            let (prefix, target) = key.split_once("__").unwrap_or((key, ""));
            let viewport = prefix
                .split_once('_')
                .filter(|(prefix, _)| {
                    prefix.len() >= 16 && prefix.bytes().all(|byte| byte.is_ascii_hexdigit())
                })
                .map_or("", |(_, viewport)| viewport);
            (viewport, target)
        });
    let capture = match target {
        "FINAL" => "Full page".into(),
        "VIEWPORT" => "Viewport capture".into(),
        target if target.starts_with('$') => format!("Saved: {}", &target[1..]),
        target if target.starts_with("SELECTOR-") => "Element capture".into(),
        _ => match selector {
            "document" => "Full page".into(),
            "viewport" => "Viewport capture".into(),
            selector if selector.starts_with("save:") => format!("Saved: {}", &selector[5..]),
            selector if selector.starts_with("SELECTOR-") => "Element capture".into(),
            selector => selector.to_string(),
        },
    };
    (
        viewport.to_string(),
        [humanize(viewport), capture]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" / "),
    )
}

fn safe_query_key(key: &str) -> bool {
    let key = key.to_ascii_lowercase().replace(['-', '_'], "");
    !key.starts_with("utm")
        && !matches!(
            key.as_str(),
            "key" | "code" | "sig" | "sas" | "gclid" | "fbclid" | "msclkid"
        )
        && ![
            "token",
            "password",
            "secret",
            "credential",
            "signature",
            "auth",
            "session",
            "apikey",
        ]
        .iter()
        .any(|sensitive| key.contains(sensitive))
}

fn query_detail(url: &Url) -> String {
    let mut pairs = url
        .query_pairs()
        .filter(|(key, _)| safe_query_key(key))
        .map(|(key, value)| format!("{}={}", clean_text(&key), clean_text(&value)))
        .collect::<Vec<_>>();
    pairs.sort();
    pairs.join("&")
}

fn counts(names: &[String]) -> HashMap<String, usize> {
    let mut counts = HashMap::new();
    for name in names {
        *counts.entry(name.to_lowercase()).or_default() += 1;
    }
    counts
}

/// Presentation only: never use these names for IDs, capture keys, or filenames.
pub(crate) fn scenario_names(scenarios: &[ScenarioName<'_>]) -> Vec<String> {
    let mut identities = HashMap::new();
    let mut parts = Vec::new();
    let indices = scenarios
        .iter()
        .map(|scenario| {
            let key = if scenario.id.is_empty() {
                (scenario.id, scenario.url, scenario.label)
            } else {
                (scenario.id, None, None)
            };
            *identities.entry(key).or_insert_with(|| {
                let index = parts.len();
                parts.push(NameParts::new(scenario));
                index
            })
        })
        .collect::<Vec<_>>();
    let origins = parts
        .iter()
        .filter_map(|part| part.origin.as_deref())
        .collect::<HashSet<_>>();
    let include_origin = origins.len() > 1;
    let mut names;
    loop {
        names = parts
            .iter()
            .map(|part| part.title(include_origin))
            .collect::<Vec<_>>();
        let counts = counts(&names);
        let mut expanded = false;
        for (part, name) in parts.iter_mut().zip(&names) {
            if part.explicit.is_none()
                && counts[&name.to_lowercase()] > 1
                && part.depth < part.path.len()
            {
                part.depth += 1;
                expanded = true;
            }
        }
        if !expanded {
            break;
        }
    }
    let collisions = counts(&names);
    for (part, name) in parts.iter().zip(&mut names) {
        if part.explicit.is_none()
            && collisions[&name.to_lowercase()] > 1
            && !part.detail.is_empty()
        {
            *name = format!("{name} ({})", part.detail);
        }
    }
    for (part, name) in parts.iter().zip(&mut names) {
        if part.explicit.is_none() {
            *name = bounded(name);
        }
    }
    let collisions = counts(&names);
    let mut used = names
        .iter()
        .map(|name| name.to_lowercase())
        .collect::<HashSet<_>>();
    let mut ordinals = HashMap::new();
    for (part, name) in parts.iter().zip(&mut names) {
        if part.explicit.is_some() || collisions[&name.to_lowercase()] < 2 {
            continue;
        }
        let ordinal = ordinals.entry(name.to_lowercase()).or_insert(0);
        loop {
            *ordinal += 1;
            let candidate = format!("{name} ({ordinal})");
            if used.insert(candidate.to_lowercase()) {
                *name = candidate;
                break;
            }
        }
    }
    indices
        .into_iter()
        .map(|index| names[index].clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(urls: &[&str]) -> Vec<String> {
        scenario_names(
            &urls
                .iter()
                .map(|url| ScenarioName {
                    id: url,
                    label: None,
                    url: Some(url),
                })
                .collect::<Vec<_>>(),
        )
    }

    #[test]
    fn shortest_unique_suffix_and_readable_pages() {
        assert_eq!(
            names(&[
                "https://example.test/en/men/running-shoes",
                "https://example.test/en/women/running-shoes/",
                "https://example.test/en/help_center.html",
                "https://example.test/en/about/index.html",
                "https://example.test/",
                "https://example.test/caf%C3%A9%20menu",
            ]),
            [
                "Men / Running shoes",
                "Women / Running shoes",
                "Help center",
                "About",
                "Home",
                "Caf\u{e9} menu"
            ]
        );
    }

    #[test]
    fn mixed_origins_keep_scheme_and_port_but_not_credentials() {
        assert_eq!(
            names(&[
                "https://username:password@example.test/cart",
                "http://example.test/cart",
                "https://example.test:8443/cart",
            ]),
            [
                "https://example.test / Cart",
                "http://example.test / Cart",
                "https://example.test:8443 / Cart"
            ]
        );
    }

    #[test]
    fn queries_and_fragments_disambiguate_without_tracking_or_secrets() {
        assert_eq!(
            names(&[
                "https://example.test/search?q=red+shoes&token=secret-one&utm_source=source",
                "https://example.test/search?q=blue+shoes&token=secret-two",
                "https://example.test/guide#setup",
                "https://example.test/guide#usage",
                "https://example.test/#/account/settings",
            ]),
            [
                "Search (?q=red shoes)",
                "Search (?q=blue shoes)",
                "Guide (#setup)",
                "Guide (#usage)",
                "Settings"
            ]
        );
    }

    #[test]
    fn labels_and_logical_scenarios_are_preserved() {
        let inputs = [
            ScenarioName {
                id: "first",
                label: Some("My checkout"),
                url: Some("https://example.test/checkout"),
            },
            ScenarioName {
                id: "second",
                label: None,
                url: Some("https://example.test/checkout"),
            },
            ScenarioName {
                id: "second",
                label: None,
                url: Some("https://example.test/checkout"),
            },
            ScenarioName {
                id: "third",
                label: Some("  "),
                url: Some("https://example.test/checkout"),
            },
            ScenarioName {
                id: "fourth",
                label: Some("Checkout (1)"),
                url: None,
            },
        ];
        assert_eq!(
            scenario_names(&inputs),
            [
                "My checkout",
                "Checkout (2)",
                "Checkout (2)",
                "Checkout (3)",
                "Checkout (1)"
            ]
        );
    }

    #[test]
    fn recorded_names_preserve_explicit_url_labels_and_partial_suite_context() {
        let url = "https://example.test/men/shoes";
        assert_eq!(
            scenario_names(&[
                ScenarioName::captured("first", Some("Men / Shoes"), Some(url), Some(url), None),
                ScenarioName::captured("second", Some(url), Some(url), Some(url), None),
                ScenarioName::captured("third", None, Some("third"), Some(url), None),
            ]),
            ["Men / Shoes", url, "third"]
        );
        assert_eq!(
            scenario_names(&[ScenarioName::captured(
                "legacy",
                None,
                Some(url),
                Some(url),
                None
            )]),
            ["Shoes"]
        );
    }

    #[test]
    fn expands_as_many_parents_as_needed_and_handles_path_prefixes() {
        assert_eq!(
            names(&[
                "https://example.test/a/shared/page",
                "https://example.test/b/shared/page",
                "https://example.test/shared/page",
                "https://example.test/shared",
            ]),
            [
                "A / Shared / Page",
                "B / Shared / Page",
                "Shared / Page",
                "Shared"
            ]
        );
    }

    #[test]
    fn credential_fragments_and_tracking_only_variants_use_ordinals() {
        assert_eq!(
            names(&[
                "https://example.test/account#access_token=synthetic-one&state=ready",
                "https://example.test/account#access_token=synthetic-two&state=ready",
                "https://example.test/search?utm_campaign=one&api_key=synthetic-one",
                "https://example.test/search?utm_campaign=two&api_key=synthetic-two",
            ]),
            [
                "Account (#state=ready) (1)",
                "Account (#state=ready) (2)",
                "Search (1)",
                "Search (2)"
            ]
        );
    }

    #[test]
    fn long_and_control_character_names_remain_bounded_and_distinct() {
        let first = format!("https://example.test/{}-one", "long".repeat(100));
        let second = format!("https://example.test/{}-two", "long".repeat(100));
        let names = names(&[&first, &second, "https://example.test/%E2%80%AEa%0Ab"]);
        assert_ne!(names[0], names[1]);
        assert!(names.iter().all(|name| name.chars().count() < 140));
        assert_eq!(names[2], "Ab");
    }

    #[test]
    fn legacy_capture_labels_are_kept_but_generated_and_blank_labels_are_not() {
        let url = "https://example.test/help-center";
        let captured = || ScenarioName::captured("legacy", None, None, Some(url), None);
        assert_eq!(
            scenario_names(&[
                ScenarioName::captured("", None, None, Some(url), None),
                ScenarioName::captured("", None, Some("  "), Some(url), None),
            ]),
            ["Help center", "Help center"]
        );
        assert_eq!(
            scenario_names(&[captured().with_capture_label("Existing label", "legacy__FINAL")]),
            ["Existing label"]
        );
        for label in ["legacy FINAL", "legacy  FINAL", "legacy__FINAL", "  "] {
            assert_eq!(
                scenario_names(&[captured().with_capture_label(label, "legacy__FINAL")]),
                ["Help center"]
            );
        }
    }

    #[test]
    fn malformed_and_unsupported_urls_do_not_expose_opaque_ids() {
        assert_eq!(
            names(&["not a URL", "javascript:alert(1)", "file:///private/path"]),
            [
                "Unnamed scenario (1)",
                "Unnamed scenario (2)",
                "Unnamed scenario (3)"
            ]
        );
        assert!(scenario_names(&[]).is_empty());
    }
}
