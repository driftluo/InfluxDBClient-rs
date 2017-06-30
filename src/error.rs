use std::fmt;

#[derive(Debug)]
pub enum Error {
    SyntaxError(String),
    InvalidCredentials(String),
    DataBaseDoesNotExist(String),
    RetentionPolicyDoesNotExist(String),
    Unknow(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::SyntaxError(ref t) => write!(f, "{}", t),
            Error::InvalidCredentials(ref t) => write!(f, "{}", t),
            Error::DataBaseDoesNotExist(ref t) => write!(f, "{}", t),
            Error::RetentionPolicyDoesNotExist(ref t) => write!(f, "{}", t),
            Error::Unknow(ref t) => write!(f, "{}", t)
        }
    }
}
