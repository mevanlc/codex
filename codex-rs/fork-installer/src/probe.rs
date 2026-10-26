//! Runs packaged executables to confirm they start on this machine.

use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::process::Output;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;

use crate::cancel::Cancel;

/// Reports the `--version` output of an executable.
pub(crate) trait Probe {
    fn version(&self, executable: &Path) -> Result<String>;
}

pub(crate) struct ProcessProbe {
    pub(crate) timeout: Duration,
    pub(crate) cancel: Cancel,
}

impl Probe for ProcessProbe {
    fn version(&self, executable: &Path) -> Result<String> {
        let mut command = Command::new(executable);
        command.arg("--version");
        let output = output_with_timeout(command, self.timeout, &self.cancel)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stderr = stderr.trim();
            anyhow::bail!(
                "{} --version failed: {}",
                executable.display(),
                if stderr.is_empty() {
                    output.status.to_string()
                } else {
                    stderr.to_string()
                }
            );
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

/// Run a command to completion, killing it on timeout or cancellation.
pub(crate) fn output_with_timeout(
    mut command: Command,
    timeout: Duration,
    cancel: &Cancel,
) -> Result<Output> {
    let program = command.get_program().to_string_lossy().into_owned();
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to run {program}"))?;
    let readers = [
        child
            .stdout
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
        child
            .stderr
            .take()
            .map(|pipe| Box::new(pipe) as Box<dyn Read + Send>),
    ]
    .map(|pipe| {
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut bytes);
            }
            bytes
        })
    });
    let deadline = Instant::now() + timeout;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        let cancelled = cancel.check();
        if cancelled.is_err() || Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            cancelled?;
            anyhow::bail!("{program} did not finish within {}s", timeout.as_secs());
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let [stdout, stderr] = readers.map(|reader| reader.join().unwrap_or_default());
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}
