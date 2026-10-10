use std::{error::Error, fmt, io, ops::Deref, path::Path, process::Termination, sync::Arc};

pub struct BaursakError(Box<ErrorInner>);

impl BaursakError {
    pub fn from_io(error: io::Error, message: &str) -> Self {
        ErrorInner {
            message: Some(message.into()),
            kind: ErrorKind::Io(error),
            source: None,
        }
        .into()
    }

    #[allow(unused)]
    pub fn from_lua(error: mlua::Error, message: &str) -> Self {
        ErrorInner {
            message: Some(message.into()),
            kind: ErrorKind::Lua(error),
            source: None,
        }
        .into()
    }

    pub fn from_workspace(error: WorkspaceError) -> Self {
        ErrorInner {
            message: None,
            kind: ErrorKind::Workspace(error),
            source: None,
        }
        .into()
    }

    // TODO: task errors print "error running task `x`: ..." without the "error: " prefix every other error has
    pub fn from_run(error: RunError) -> Self {
        let task = match &error {
            RunError::Lua { task, .. } => Some(task),
            RunError::DependencyNotFound { task, .. } => Some(task),
            RunError::SetCurrentDir { task, .. } => Some(task),
            RunError::Shell { task, .. } => Some(task),
            RunError::ExitCode { task, .. } => Some(task),
            RunError::SubstitutionError { task, .. } => Some(task),
            _ => None,
        };

        let message = task.map(|t| format!("error running task `{t}`").into());
        ErrorInner {
            message,
            kind: ErrorKind::Run(error),
            source: None,
        }
        .into()
    }

    #[allow(unused)]
    pub fn from_other(error: Box<dyn Error + Send + Sync + 'static>, message: &str) -> Self {
        ErrorInner {
            message: Some(message.into()),
            kind: ErrorKind::Other(error),
            source: None,
        }
        .into()
    }
}

impl std::error::Error for BaursakError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self.source.as_deref() {
            Some(source) => Some(source),
            None => match &self.kind {
                ErrorKind::Io(error) => Some(error),
                ErrorKind::Lua(error) => Some(error),
                ErrorKind::Workspace(error) => Some(error),
                ErrorKind::Run(error) => Some(error),
                ErrorKind::Other(error) => Some(error.deref()),
            },
        }
    }
}

impl Deref for BaursakError {
    type Target = ErrorInner;

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

// TODO: dead code: `source` is always None; `Lua`/`Other` kinds, `from_lua`/`from_other` and the clap leftovers are unused
pub struct ErrorInner {
    message: Option<Arc<str>>,
    kind: ErrorKind,
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

enum ErrorKind {
    Io(io::Error),
    #[allow(unused)]
    Lua(mlua::Error),
    Run(RunError),
    Workspace(WorkspaceError),
    #[allow(unused)]
    Other(Box<dyn Error + Send + Sync + 'static>),
}

impl Termination for BaursakError {
    fn report(self) -> std::process::ExitCode {
        let code = 1;

        code.into()
    }
}

// TODO: always print "error: " here, then the optional message, and remove the hardcoded "error: " from `from_io` messages in main.rs
impl fmt::Display for BaursakError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match &self.message {
            Some(message) => format!("{message}: "),
            None => "error: ".to_owned(),
        };

        let msg = match &self.kind {
            ErrorKind::Io(error) => format!("{msg}{error:#}"),
            ErrorKind::Lua(error) => format!("{msg}{error:#}"),
            ErrorKind::Workspace(error) => format!("{msg}{error:#}"),
            ErrorKind::Run(error) => format!("{msg}{error:#}"),
            ErrorKind::Other(error) => format!("{msg}{error:#}"),
        };

        write!(f, "{msg}")
    }
}

impl fmt::Debug for BaursakError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

impl From<ErrorInner> for BaursakError {
    fn from(value: ErrorInner) -> Self {
        Self(Box::new(value))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("could not read current working directory: {0}")]
    GetCurrentDir(io::Error),

    #[error("could not cd into {1:?}: {0}")]
    SetCurrentDir(io::Error, Arc<Path>),

    #[error("no task file found")]
    NoTaskFileFound,

    #[error("{0:?} is a directory")]
    TaskFileIsDir(Arc<Path>),

    #[error("could not create Lua environment: {0:#}")]
    CreatingLuaEnvironment(mlua::Error),

    #[error("{0:?} does not exist")]
    SpecifiedScriptDoesNotExist(Arc<Path>),

    #[error("could not read task file {1:?}: {0}")]
    ReadingScript(io::Error, Arc<Path>),

    #[error("{0:#}")]
    ExecutingScript(mlua::Error, Arc<Path>),
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("{error:#}")]
    Lua { task: Arc<str>, error: mlua::Error },

    #[error("task `{search}` does not exist in current context")]
    DependencyNotFound { task: Arc<str>, search: Arc<str> },

    #[error("task `{task}` does not exist in current context")]
    TaskNotFound { task: Arc<str> },

    #[error("could not cd into {path:?}: {error}")]
    SetCurrentDir {
        task: Arc<str>,
        error: io::Error,
        path: Arc<Path>,
    },

    #[error("could not spawn shell command {command:?}: {error}")]
    Shell {
        task: Arc<str>,
        error: io::Error,
        command: Arc<str>,
    },

    #[error("command {command:?} exited with non-zero exit code {code}")]
    ExitCode {
        task: Arc<str>,
        code: i32,
        command: Arc<str>,
    },

    #[error("could not substitute variables in command {command:?}: {error}")]
    SubstitutionError {
        task: Arc<str>,
        error: subst::Error,
        command: Arc<str>,
    },

    #[error("there are no tasks defined")]
    NoTasksDefined,
}
