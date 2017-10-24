use std::fmt;
use std::io;
use hyper;

#[derive(Debug)]
pub enum Error {
    SyntaxError(String),
    InvalidCredentials(String),
    DataBaseDoesNotExist(String),
    RetentionPolicyDoesNotExist(String),
    Communication(String),
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
            Error::Unknow(ref t) => write!(f, "{}", t)
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
