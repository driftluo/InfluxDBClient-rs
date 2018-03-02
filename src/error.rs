use std::fmt;
use std::io;
use hyper;

/// The error of influxdb client
#[derive(Debug)]
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

impl From<hyper::Error> for Error {
    fn from(err: hyper::Error) -> Self {
        Error::Communication(format!("{}", err))
    }
}
