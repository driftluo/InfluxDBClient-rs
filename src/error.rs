#[derive(Debug)]
pub enum Error {
    SyntaxError(String),
    InvalidCredentials(String),
    DataBaseDoesNotExist(String),
    RetentionPolicyDoesNotExist(String),
    Unknow(String),
}
