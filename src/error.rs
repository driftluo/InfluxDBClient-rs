use serde::{Deserialize, Serialize};
use std::error::Error as StdError;
use std::fmt;
use std::io;

/// The error of influxdb client
#[derive(Debug, Deserialize, PartialEq, Eq, Serialize)]
pub enum Error {
    /// Syntax error, some is bug, some is SQL error. If it's a bug, welcome to PR.
    SyntaxError(String),
    /// Invalid credentials
    InvalidCredentials(String),
    /// The specified database does not exist
    DataBaseDoesNotExist(String),
    /// The specified retention policy does not exist
    RetentionPolicyDoesNotExist(String),
    /// Some error on build url or io.
    Communication(String),
    /// Some other error, I don't expect
    Unknow(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::SyntaxError(ref t) => write!(f, "{}", t),
            Error::InvalidCredentials(ref t) => write!(f, "{}", t),
            Error::DataBaseDoesNotExist(ref t) => write!(f, "{}", t),
            Error::RetentionPolicyDoesNotExist(ref t) => write!(f, "{}", t),
            Error::Communication(ref t) => write!(f, "{}", t),
            Error::Unknow(ref t) => write!(f, "{}", t),
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Communication(format!("{}", err))
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::Communication(format!("{}", err))
    }
}

impl StdError for Error {
    fn description(&self) -> &str {
        match *self {
            Error::SyntaxError(ref t) => t,
            Error::InvalidCredentials(ref t) => t,
            Error::DataBaseDoesNotExist(ref t) => t,
            Error::RetentionPolicyDoesNotExist(ref t) => t,
            Error::Communication(ref t) => t,
            Error::Unknow(ref t) => t,
        }
    }
}
