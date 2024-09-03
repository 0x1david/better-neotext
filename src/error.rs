use std::fmt::Debug;

use derive_more::From;

use crate::LineCol;

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Debug, From)]
pub enum Error {
    InvalidPosition,
    ExitCall,
    InvalidRange(LineCol, LineCol),
    InvalidLineNumber,
    InvalidInput,
    PatternNotFound,
    NoCommandAvailable,
    UnexpectedRegisterData,
    ProgrammingBug {
        descr: String,
    },
    NowhereToGo,
    ImATeacup,

    #[from]
    Io(std::io::Error),
}

impl core::fmt::Display for Error {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::result::Result<(), core::fmt::Error> {
        write!(fmt, "{self:?}")
    }
}

impl std::error::Error for Error {}
