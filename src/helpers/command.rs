#![allow(dead_code)]

use anyhow::{Ok, Result, anyhow, bail};
use colored::Colorize;
use std::{
    ffi::OsStr,
    process::{Command, Stdio},
};

use crate::common;

use super::OnceLockBoolExtensions;

pub trait CommandExtensions {
    /// Conditionally add an argument to the command.
    fn arg_if<S: AsRef<OsStr>>(&mut self, condition: bool, arg: S) -> &mut Self;

    /// Conditionally add arguments to the command.
    fn args_if<I, S>(&mut self, condition: bool, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>;

    /// Conditionally add an argument to the command if the option is Some.
    fn arg_if_some<A, S: AsRef<OsStr>>(
        &mut self,
        option: Option<A>,
        f: impl FnOnce(A) -> S,
    ) -> &mut Self;

    /// Conditionally add arguments to the command.
    fn args_if_some<A, I, S>(&mut self, option: Option<A>, f: impl FnOnce(A) -> I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>;

    /// Print the command to be executed.
    fn println(&mut self) -> Result<&mut Self>;

    /// Get the command as a string.
    fn as_str(&self) -> Result<String>;

    /// Execute the command, optionally requiring success.
    fn exec(&mut self, require_success: bool) -> Result<()>;

    /// Execute the command and consider specified status codes as success.
    fn exec_success_or_status(&mut self, status: Vec<i32>) -> Result<()>;

    /// Execute the command and get the output as a string.
    fn exec_get_string(&mut self, require_success: bool, trim_end: bool) -> Result<String>;

    /// Execute the command and get the output as lines.
    fn exec_get_lines(&mut self, require_success: bool, trim_end: bool) -> Result<Vec<String>>;
}

impl CommandExtensions for Command {
    fn arg_if<S: AsRef<OsStr>>(&mut self, condition: bool, arg: S) -> &mut Self {
        if condition {
            self.arg(arg);
        }

        self
    }

    fn args_if<I, S>(&mut self, condition: bool, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        if condition {
            self.args(args);
        }

        self
    }

    fn arg_if_some<A, S: AsRef<OsStr>>(
        &mut self,
        option: Option<A>,
        f: impl FnOnce(A) -> S,
    ) -> &mut Self {
        option.map(|o| self.arg(f(o)));

        self
    }

    fn args_if_some<A, I, S>(&mut self, option: Option<A>, f: impl FnOnce(A) -> I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        option.map(|o| self.args(f(o)));

        self
    }

    fn println(&mut self) -> Result<&mut Self> {
        #[cfg(feature = "local")]
        if !common::NO_PRINT.get_bool() {
            eprintln!(
                "{}{}",
                "# ".yellow().italic(),
                format_command(self)?.yellow().italic()
            );
        }

        Ok(self)
    }

    fn as_str(&self) -> Result<String> {
        format_command(self)
    }

    fn exec(&mut self, require_success: bool) -> Result<()> {
        let output = self
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?
            .wait_with_output()?;

        let is_success = output.status.success();
        let status_code = output.status.code().unwrap_or(-1);

        if require_success && !is_success {
            bail!(
                "Failed {} command with status {}",
                self.get_program().to_string_lossy(),
                status_code
            );
        } else {
            Ok(())
        }
    }

    fn exec_success_or_status(&mut self, status: Vec<i32>) -> Result<()> {
        let output = self
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()?
            .wait_with_output()?;

        let is_success = output.status.success();
        let status_code = output.status.code().unwrap_or(-1);

        if !is_success && !status.contains(&status_code) {
            bail!(
                "Failed {} command with status {}",
                self.get_program().to_string_lossy(),
                status_code
            );
        } else {
            Ok(())
        }
    }

    fn exec_get_string(&mut self, require_success: bool, trim_end: bool) -> Result<String> {
        let output = self
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| anyhow!("Failed to spawn process {:?}: {}", self.get_program(), e))?
            .wait_with_output()
            .map_err(|e| anyhow!("Failed to wait for process {:?}: {}", self.get_program(), e))?;

        let is_success = output.status.success();
        let status_code = output.status.code().unwrap_or(-1);
        let output = String::from_utf8(output.stdout)
            .map_err(|e| anyhow!("Failed to read output from {:?}: {}", self.get_program(), e))?;

        if require_success && !is_success {
            let output = output.trim_end();
            if !output.is_empty() {
                eprintln!("{}", output);
            }

            bail!(
                "Failed {} command with status {}",
                self.get_program().to_string_lossy(),
                status_code
            );
        }

        let output = if trim_end {
            output.trim_end().to_string()
        } else {
            output
        };

        Ok(output)
    }

    fn exec_get_lines(&mut self, require_success: bool, trim_end: bool) -> Result<Vec<String>> {
        let output = self
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?
            .wait_with_output()?;

        let is_success = output.status.success();
        let status_code = output.status.code().unwrap_or(-1);
        let output = String::from_utf8(output.stdout)?;

        if require_success && !is_success {
            let output = output.trim_end();
            if !output.is_empty() {
                eprintln!("{}", output);
            }
            bail!(
                "Failed {} command with status {}",
                self.get_program().to_string_lossy(),
                status_code
            );
        }

        let output = if trim_end { output.trim_end() } else { &output };

        let lines = output.lines().map(|line| line.to_string()).collect();
        Ok(lines)
    }
}

fn format_command(command: &Command) -> Result<String> {
    let mut output = String::new();
    let program = command
        .get_program()
        .to_str()
        .ok_or_else(|| anyhow!("Invalid program"))?;

    escape(&mut output, program);

    for arg in command.get_args() {
        output.push(' ');
        let arg_str = arg.to_str().ok_or_else(|| anyhow!("Invalid argument"))?;
        escape(&mut output, arg_str);
    }

    Ok(output)
}

fn escape(output: &mut String, arg: &str) {
    if arg.is_empty() {
        output.push_str("''");
        return;
    }

    let safe = arg.chars().all(|c| {
        c.is_ascii_alphanumeric()
            || c == '+'
            || c == '-'
            || c == ','
            || c == '_'
            || c == '.'
            || c == ':'
            || c == '='
            || c == '/'
            || c == '@'
    });

    if safe {
        output.push_str(arg);
        return;
    }

    output.push('\'');
    for c in arg.chars() {
        if c == '\'' {
            output.push_str("'\\''");
        } else {
            output.push(c);
        }
    }
    output.push('\'');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_command_basic() {
        let mut cmd = Command::new("echo");
        cmd.arg("hello").arg("world");
        assert_eq!(format_command(&cmd).unwrap(), "echo hello world");
    }

    #[test]
    fn test_format_command_with_spaces() {
        let mut cmd = Command::new("echo");
        cmd.arg("hello world");
        assert_eq!(format_command(&cmd).unwrap(), "echo 'hello world'");
    }

    #[test]
    fn test_format_command_with_special_chars() {
        let mut cmd = Command::new("echo");
        cmd.arg("foo;bar");
        // Semicolon is unsafe, so it must be quoted
        assert_eq!(format_command(&cmd).unwrap(), "echo 'foo;bar'");
    }

    #[test]
    fn test_format_command_with_quotes() {
        let mut cmd = Command::new("echo");
        cmd.arg("foo'bar");
        assert_eq!(format_command(&cmd).unwrap(), "echo 'foo'\\''bar'");
    }

    #[test]
    fn test_format_command_with_double_quotes() {
        let mut cmd = Command::new("echo");
        cmd.arg("foo\"bar");
        assert_eq!(format_command(&cmd).unwrap(), "echo 'foo\"bar'");
    }

    #[test]
    fn test_format_command_mixed_quotes() {
        let mut cmd = Command::new("echo");
        cmd.arg("foo'bar\"baz");
        assert_eq!(format_command(&cmd).unwrap(), "echo 'foo'\\''bar\"baz'");
    }

    #[test]
    fn test_format_command_dollar() {
        let mut cmd = Command::new("echo");
        cmd.arg("$HOME");
        assert_eq!(format_command(&cmd).unwrap(), "echo '$HOME'");
    }

    #[test]
    fn test_format_command_dollar_and_quote() {
        let mut cmd = Command::new("echo");
        cmd.arg("$HOME'");
        // Should be '$HOME'\'''
        assert_eq!(format_command(&cmd).unwrap(), "echo '$HOME'\\'''");
    }

    #[test]
    fn test_format_command_empty() {
        let mut cmd = Command::new("echo");
        cmd.arg("");
        assert_eq!(format_command(&cmd).unwrap(), "echo ''");
    }
}
