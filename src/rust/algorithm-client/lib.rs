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
    WorkerNotFound(Vec<PathBuf>),
    MissingPipe(&'static str),
}

impl fmt::Display for AlgorithmClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "I/O error: {error}"),
            Self::Json(error) => write!(formatter, "JSON error: {error}"),
            Self::WorkerFailed(message) => write!(formatter, "worker failed: {message}"),
            Self::WorkerNotFound(candidates) => write!(
                formatter,
                "Python algorithm worker not found in {}",
                candidates
                    .iter()
                    .map(|path| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
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

    pub fn from_environment() -> Result<Self, AlgorithmClientError> {
        let runtime_home = std::env::var_os("LOOM_RUNTIME_HOME").map(PathBuf::from);
        let python = std::env::var_os("PYTHON")
            .map(PathBuf::from)
            .or_else(|| runtime_home.as_ref().and_then(runtime_python))
            .unwrap_or_else(|| PathBuf::from("python3"));
        let mut candidates = Vec::new();
        if let Some(runtime_home) = runtime_home {
            candidates.push(runtime_home.join("python/algorithms/worker.py"));
        }
        candidates.extend([
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../python/algorithms/worker.py"),
            PathBuf::from("src/python/algorithms/worker.py"),
            PathBuf::from("../python/algorithms/worker.py"),
            PathBuf::from("../../src/python/algorithms/worker.py"),
        ]);
        let worker = candidates
            .iter()
            .find(|path| path.exists())
            .cloned()
            .ok_or_else(|| AlgorithmClientError::WorkerNotFound(candidates.clone()))?;
        Ok(Self::new(python, worker))
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

fn runtime_python(runtime_home: &PathBuf) -> Option<PathBuf> {
    let candidates = if cfg!(windows) {
        vec![
            runtime_home.join("python/runtime/python.exe"),
            runtime_home.join("python/runtime/Scripts/python.exe"),
        ]
    } else {
        vec![runtime_home.join("python/runtime/bin/python")]
    };
    candidates.into_iter().find(|path| path.exists())
}
