//! A bounded child-process run: deadline, per-stream output cap and descendant cleanup.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#qualification-process-isolation

use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Runs `command` to completion within `timeout`, draining stdout and stderr concurrently
/// up to `limit` bytes each; more is an error. The helper starts inside a job that
/// also holds its descendants: when the helper exits or outlives the deadline (then it is
/// terminated with a bounded wait), closing the job ends any descendant still holding the
/// output pipes, so the readers reach end of file. Collecting both outputs shares one
/// deadline.
pub(crate) fn run_bounded_output(
    command: &mut Command,
    timeout: Duration,
    label: &str,
    limit: u64,
) -> Result<Output, String> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    // Started inside the job, so every descendant it creates is in the job too.
    let job = super::Job::create(super::Descendants::Included)?;
    let mut child = job.spawn(command, &format!("cannot start {label}"))?;
    let deadline = Instant::now() + timeout;
    let stdout = drain(child.stdout.take(), label, "stdout", limit)?;
    let stderr = drain(child.stderr.take(), label, "stderr", limit)?;
    let status = loop {
        match child
            .try_wait()
            .map_err(|error| format!("cannot inspect {label}: {error}"))?
        {
            Some(status) => break status,
            None if Instant::now() < deadline => thread::sleep(Duration::from_millis(25)),
            None => {
                let process_id = child.id();
                let stopped = child.stop_within(super::STOP_WAIT);
                drop(job);
                stopped.map_err(|error| {
                    format!(
                        "{label} exceeded {timeout:?}; failed to reap PID {process_id}: {error}"
                    )
                })?;
                return Err(format!(
                    "{label} exceeded {timeout:?}; killed and reaped PID {process_id}"
                ));
            }
        }
    };
    // Ends any descendant still holding a pipe, so both readers reach end of file.
    drop(job);
    let collect_by = Instant::now()
        + deadline
            .saturating_duration_since(Instant::now())
            .max(Duration::from_secs(2));
    let collect = |pending: mpsc::Receiver<Result<Vec<u8>, String>>, stream: &str| {
        pending
            .recv_timeout(collect_by.saturating_duration_since(Instant::now()))
            .map_err(|_| format!("{label} {stream} stayed open after it exited"))?
    };
    Ok(Output {
        status,
        stdout: collect(stdout, "stdout")?,
        stderr: collect(stderr, "stderr")?,
    })
}

fn drain(
    pipe: Option<impl Read + Send + 'static>,
    label: &str,
    stream: &'static str,
    limit: u64,
) -> Result<mpsc::Receiver<Result<Vec<u8>, String>>, String> {
    let mut pipe = pipe.ok_or_else(|| format!("{label} {stream} pipe is unavailable"))?;
    let owner = label.to_owned();
    let (sender, receiver) = mpsc::channel();
    thread::Builder::new()
        .name(format!("helper-{stream}"))
        .spawn(move || {
            let mut bytes = Vec::new();
            let read = (&mut pipe)
                .take(limit + 1)
                .read_to_end(&mut bytes)
                .map_err(|error| format!("cannot read {owner} {stream}: {error}"))
                .and_then(|_| {
                    if bytes.len() as u64 > limit {
                        Err(format!("{owner} wrote more than {limit} bytes to {stream}"))
                    } else {
                        Ok(bytes)
                    }
                });
            // The receiver is gone only after its deadline; the late result is moot.
            let _ = sender.send(read);
        })
        .map_err(|error| format!("cannot start the {stream} reader for {label}: {error}"))?;
    Ok(receiver)
}
