use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use tokio::time::timeout;

#[derive(Debug)]
pub(crate) enum CommandError {
    Timeout,
    Io {
        operation: &'static str,
        source: std::io::Error,
    },
}

pub(crate) async fn run_command(
    command: &mut Command,
    input: &[u8],
    deadline: Duration,
    max_stdout: usize,
    max_stderr: usize,
) -> Result<CommandOutput, CommandError> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);
    timeout(deadline, async {
        let mut child = command.spawn().map_err(|source| CommandError::Io {
            operation: "spawn",
            source,
        })?;
        #[cfg(unix)]
        let _group = ProcessGroup(child.id());
        let mut stdin = child.stdin.take().ok_or_else(|| missing_pipe("stdin"))?;
        let stdout = child.stdout.take().ok_or_else(|| missing_pipe("stdout"))?;
        let stderr = child.stderr.take().ok_or_else(|| missing_pipe("stderr"))?;
        let write = async {
            if let Err(error) = stdin.write_all(input).await
                && error.kind() != std::io::ErrorKind::BrokenPipe
            {
                return Err(error);
            }
            drop(stdin);
            Ok::<_, std::io::Error>(())
        };
        let result = tokio::try_join!(
            write,
            child.wait(),
            read_capped_output(stdout, max_stdout),
            read_capped_output(stderr, max_stderr)
        );
        match result {
            Ok(((), status, stdout, stderr)) => Ok(CommandOutput {
                status,
                stdout,
                stderr,
            }),
            Err(source) => Err(CommandError::Io {
                operation: "communicate",
                source,
            }),
        }
    })
    .await
    .map_err(|_| CommandError::Timeout)?
}

fn missing_pipe(operation: &'static str) -> CommandError {
    CommandError::Io {
        operation,
        source: std::io::Error::other("child pipe unavailable"),
    }
}

#[cfg(unix)]
struct ProcessGroup(Option<u32>);

#[cfg(unix)]
impl Drop for ProcessGroup {
    fn drop(&mut self) {
        if let Some(pid) = self.0.and_then(|pid| i32::try_from(pid).ok()) {
            // The child starts a private process group; terminate inherited pipe holders too.
            let _ = nix::sys::signal::killpg(
                nix::unistd::Pid::from_raw(pid),
                nix::sys::signal::Signal::SIGKILL,
            );
        }
    }
}

#[derive(Debug)]
pub(crate) struct CommandOutput {
    pub(crate) status: std::process::ExitStatus,
    pub(crate) stdout: CappedOutput,
    pub(crate) stderr: CappedOutput,
}

#[derive(Debug)]
pub(crate) struct CappedOutput {
    pub(crate) bytes: Vec<u8>,
    pub(crate) truncated: bool,
}

async fn read_capped_output<R>(mut reader: R, max_bytes: usize) -> std::io::Result<CappedOutput>
where
    R: AsyncRead + Unpin,
{
    let mut bytes = Vec::new();
    let mut truncated = false;
    let mut buffer = [0_u8; 8192];

    loop {
        let read = reader.read(&mut buffer).await?;
        if read == 0 {
            break;
        }
        let remaining = max_bytes.saturating_sub(bytes.len());
        if remaining > 0 {
            let keep = remaining.min(read);
            bytes.extend_from_slice(&buffer[..keep]);
            if keep < read {
                truncated = true;
            }
        } else {
            truncated = true;
        }
    }

    Ok(CappedOutput { bytes, truncated })
}

pub(crate) fn capped_utf8_lossy(output: &CappedOutput) -> String {
    let mut value = String::from_utf8_lossy(&output.bytes).into_owned();
    if output.truncated {
        value.push_str("<truncated>");
    }
    value
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn early_stdin_close_preserves_exit_status_and_output() {
        let mut command = Command::new("sh");
        command.args(["-c", "printf response; exit 7"]);
        let output = run_command(
            &mut command,
            &vec![0; 131072],
            Duration::from_secs(5),
            16,
            16,
        )
        .await
        .unwrap();
        assert_eq!(output.status.code(), Some(7));
        assert_eq!(output.stdout.bytes, b"response");
        assert!(!output.stdout.truncated);
    }

    #[tokio::test]
    async fn deadline_includes_blocked_stdin() {
        let mut command = Command::new("sh");
        command.args(["-c", "sleep 10"]);
        let error = run_command(
            &mut command,
            &vec![0; 2 * 1024 * 1024],
            Duration::from_millis(100),
            16,
            16,
        )
        .await
        .unwrap_err();
        assert!(matches!(error, CommandError::Timeout));
    }

    #[tokio::test]
    async fn drains_both_streams_while_writing_input_with_bounded_capture() {
        let mut command = Command::new("sh");
        command.args([
            "-c",
            "head -c 131072 /dev/zero; head -c 131072 /dev/zero >&2; cat >/dev/null",
        ]);
        let output = run_command(
            &mut command,
            &vec![0; 131072],
            Duration::from_secs(5),
            11,
            17,
        )
        .await
        .unwrap();
        assert!(output.status.success());
        assert_eq!(output.stdout.bytes.len(), 11);
        assert_eq!(output.stderr.bytes.len(), 17);
        assert!(output.stdout.truncated && output.stderr.truncated);
    }

    #[tokio::test]
    async fn cancellation_stops_descendant_writes() {
        let directory = tempfile::tempdir().unwrap();
        let marker = directory.path().join("late");
        let ready = directory.path().join("ready");
        let mut command = Command::new("sh");
        command
            .arg("-c")
            .arg("(sleep 0.3; touch \"$1\") & touch \"$2\"; wait")
            .arg("test")
            .arg(&marker)
            .arg(&ready);
        let task = tokio::spawn(async move {
            run_command(&mut command, &[], Duration::from_secs(5), 16, 16).await
        });
        tokio::time::timeout(Duration::from_secs(5), async {
            while !ready.exists() {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        tokio::time::sleep(Duration::from_millis(400)).await;
        assert!(!marker.exists());
    }
}
