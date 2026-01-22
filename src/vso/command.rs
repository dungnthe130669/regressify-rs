#![allow(dead_code)]

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow};

use super::{
    check::is_azure_devops,
    source::TASK_INTERNAL,
    task_result::{self, SUCCEEDED, SUCCEEDED_WITH_ISSUES},
};

pub fn prepend_path(path: &Path) -> Result<()> {
    let path = path
        .to_str()
        .ok_or_else(|| anyhow!("Cannot convert path to string"))?;
    command("task.prependpath", None, path)
}

pub fn set_variable(name: &str, value: &str, is_output: bool) -> Result<()> {
    let mut properties: HashMap<&str, &str> = HashMap::new();
    properties.insert("variable", name);
    if is_output {
        properties.insert("isOutput", "true");
    }
    command("task.setvariable", Some(properties), value)
}

pub fn set_result(result: i32, message: &str, done: Option<bool>) -> Result<()> {
    let result_str = if result == SUCCEEDED {
        "Succeeded"
    } else if result == SUCCEEDED_WITH_ISSUES {
        "SucceededWithIssues"
    } else if result == task_result::FAILED {
        "Failed"
    } else if result == task_result::CANCELED {
        "Canceled"
    } else if result == task_result::SKIPPED {
        "Skipped"
    } else {
        "Unknown"
    };

    if cfg!(debug_assertions) {
        debug(&format!("Task result: '{}'", result_str))?;
    }

    if result == task_result::FAILED && !message.is_empty() {
        error(message, Some(TASK_INTERNAL), None)?;
    } else if result == SUCCEEDED_WITH_ISSUES && !message.is_empty() {
        warning(message, Some(TASK_INTERNAL), None)?;
    }

    let mut properties = vec![("result", result_str)];

    if done.unwrap_or(false) {
        properties.push(("done", "true"));
    }

    let properties = properties.into_iter().collect::<HashMap<&str, &str>>();

    command("task.complete", Some(properties), message)?;

    Ok(())
}

pub fn debug(message: &str) -> Result<()> {
    command("task.debug", None, message)
}

pub fn warning(message: &str, source: Option<&str>, audit_action: Option<&str>) -> Result<()> {
    let properties = vec![
        ("type", "warning"),
        ("source", source.unwrap_or(TASK_INTERNAL)),
        ("correlationId", ""),
        ("auditAction", audit_action.unwrap_or("")),
    ]
    .into_iter()
    .collect::<HashMap<&str, &str>>();

    command("task.issue", Some(properties), message)
}

pub fn error(message: &str, source: Option<&str>, audit_action: Option<&str>) -> Result<()> {
    let properties = vec![
        ("type", "error"),
        ("source", source.unwrap_or(TASK_INTERNAL)),
        ("correlationId", ""),
        ("auditAction", audit_action.unwrap_or("")),
    ]
    .into_iter()
    .collect::<HashMap<&str, &str>>();

    command("task.issue", Some(properties), message)
}

fn command(cmd: &str, properties: Option<HashMap<&str, &str>>, message: &str) -> Result<()> {
    let mut output = format!("##vso[{}", cmd);

    if let Some(properties) = properties
        && !properties.is_empty()
    {
        output.push(' ');

        for (key, value) in properties {
            if !value.trim().is_empty() {
                output.push_str(key);
                output.push('=');
                output.push_str(&escape(value));
                output.push(';');
            }
        }
    }

    output.push(']');
    output.push_str(&escapedata(message));

    if cfg!(debug_assertions) || is_azure_devops() {
        println!("{}", &output);
    }
    Ok(())
}

fn escape(s: &str) -> String {
    let mut output = String::with_capacity(s.len());

    for c in s.chars() {
        match c {
            '%' => output.push_str("%AZP25"),
            '\r' => output.push_str("%0D"),
            '\n' => output.push_str("%0A"),
            ']' => output.push_str("%5D"),
            ';' => output.push_str("%3B"),
            _ => output.push(c),
        }
    }

    output
}

fn escapedata(s: &str) -> String {
    let mut output = String::with_capacity(s.len());

    for c in s.chars() {
        match c {
            '%' => output.push_str("%AZP25"),
            '\r' => output.push_str("%0D"),
            '\n' => output.push_str("%0A"),
            _ => output.push(c),
        }
    }

    output
}
