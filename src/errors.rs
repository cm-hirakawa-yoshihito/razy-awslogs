use std::fmt::{self, Display, Formatter};

use failure::{Backtrace, Context, Fail};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Fail)]
pub enum ErrorKind {
    #[fail(display = "Clap error.")]
    Clap,

    #[fail(display = "Argument error.")]
    InsufficientArguments,

    #[fail(display = "No subcommand given.")]
    NoSubCommand,

    #[fail(display = "Any rusoto error occurred")]
    Rusoto,

    #[fail(display = "Any sync error occurred")]
    SyncChannel,
}

unsafe impl Send for ErrorKind {}

#[derive(Debug)]
pub struct Error {
    inner: Context<ErrorKind>,
}

unsafe impl Send for Error {}

impl Error {
    pub fn kind(&self) -> ErrorKind {
        *self.inner.get_context()
    }
}

impl Fail for Error {
    fn cause(&self) -> Option<&Fail> {
        self.inner.cause()
    }

    fn backtrace(&self) -> Option<&Backtrace> {
        self.inner.backtrace()
    }
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        Display::fmt(&self.inner, f)
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Error {
            inner: Context::new(kind),
        }
    }
}

impl From<Context<ErrorKind>> for Error {
    fn from(context: Context<ErrorKind>) -> Self {
        Error { inner: context }
    }
}
