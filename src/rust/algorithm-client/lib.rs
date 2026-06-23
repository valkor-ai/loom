use serde_json::Value;
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug)]
pub struct AlgorithmClient {
    python: PathBuf,
    worker: PathBuf,
}

#[derive(Debug)]
pub enum AlgorithmClientError {
    Io(std::io::Error),
    Json(serde_json::Error),
    WorkerFailed(String),
    MissingPipe(&'static str),
}

impl fmt::Display for AlgorithmClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::Json(error) => write!(formatter, "JSON error: {error}"),
            Self::WorkerFailed(message) => write!(formatter, "worker failed: {message}"),
            Self::MissingPipe(name) => write!(formatter, "missing worker {name} pipe"),
        }
    }
}

impl std::error::Error for AlgorithmClientError {}

impl From<std::io::Error> for AlgorithmClientError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for AlgorithmClientError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl AlgorithmClient {
    pub fn new(python: impl Into<PathBuf>, worker: impl Into<PathBuf>) -> Self {
        Self {
            python: python.into(),
            worker: worker.into(),
        }
    }

    pub fn call(&self, request: &Value) -> Result<Value, AlgorithmClientError> {
        let mut child = Command::new(&self.python)
            .arg(&self.worker)
            .env("PYTHONPATH", self.python_path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        {
            let stdin = child
                .stdin
                .as_mut()
                .ok_or(AlgorithmClientError::MissingPipe("stdin"))?;
            serde_json::to_writer(&mut *stdin, request)?;
            stdin.write_all(b"\n")?;
        }

        let output = child.wait_with_output()?;
        if !output.status.success() {
            return Err(AlgorithmClientError::WorkerFailed(
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ));
        }

        let response: Value = serde_json::from_slice(&output.stdout)?;
        if response.get("ok").and_then(Value::as_bool) == Some(false) {
            let message = response
                .get("error")
                .and_then(Value::as_str)
                .unwrap_or("unknown worker error")
                .to_string();
            return Err(AlgorithmClientError::WorkerFailed(message));
        }
        Ok(response)
    }

    fn python_path(&self) -> PathBuf {
        self.worker
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."))
    }
}
