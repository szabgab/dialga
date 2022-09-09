/*!
Extremely opinionated crate for an intermediate representation between [`kdl`](https://crates.io/crates/kdl) and serde.
*/

mod literal;
mod node;

use std::{char::CharTryFromError, convert::Infallible, num::TryFromIntError};

use ahash::AHashMap;
use smol_str::SmolStr;

use serde::de::{self, Unexpected, Visitor};
use thiserror::Error;

/// An node featuring the arguments, properties, and children, and a name.
#[derive(Debug, Clone)]
pub struct KdlNode {
    pub name: SmolStr,
    pub arguments: Vec<KdlAnnotatedLiteral>,
    pub properties: AHashMap<SmolStr, KdlAnnotatedLiteral>,
    pub children: Option<Vec<KdlNode>>,
}

/// Raw literal: argument or property
#[derive(Debug, Clone)]
pub enum KdlLiteral {
    String(SmolStr),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
}

/// A literal and possible annotation
#[derive(Debug, Clone)]
pub struct KdlAnnotatedLiteral {
    pub annotation: Option<SmolStr>,
    pub literal: KdlLiteral,
}

impl KdlAnnotatedLiteral {
    pub fn new(annotation: Option<SmolStr>, literal: KdlLiteral) -> Self {
        Self {
            annotation,
            literal,
        }
    }
}

impl KdlLiteral {
    fn unexpected(&self) -> Unexpected {
        match self {
            KdlLiteral::String(s) => Unexpected::Str(s.as_str()),
            KdlLiteral::Int(_) => Unexpected::Other("int"),
            KdlLiteral::Float(f) => Unexpected::Float(*f),
            KdlLiteral::Bool(b) => Unexpected::Bool(*b),
            KdlLiteral::Null => Unexpected::Unit,
        }
    }
}

#[derive(Error, Debug)]
pub enum DeError {
    #[error("the deserialize impl on the type reported an error: {0}")]
    VisitorError(String),
    #[error("tuple struct {0} requires only arguments, no properties or children")]
    TupleStructWithNotJustArgs(&'static str),
    #[error("on type {type_name}, expected {expected} fields but got {got}")]
    MismatchedTupleStructCount {
        expected: usize,
        got: usize,
        type_name: &'static str,
    },
    #[error("could not turn fit the given int into the target size: {0}")]
    IntSize(#[from] TryFromIntError),
    #[error("could not interpret the int as a char: {0}")]
    InvalidChar(#[from] CharTryFromError),
    #[error("could not decode base64: {0}")]
    Base64Error(#[from] base64::DecodeError),

    #[error("a string annotated with (byte) must be 1 byte long to be interpreted as a u8")]
    ByteAnnotationLen,
    #[error("a string annotated with (char) must be 1 char long to be interpreted as a char")]
    CharAnnotationLen,

    #[error("{0}")]
    MismatchedType(String),
}

impl de::Error for DeError {
    fn custom<T>(msg: T) -> Self
    where
        T: std::fmt::Display,
    {
        Self::VisitorError(msg.to_string())
    }
}

impl From<Infallible> for DeError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}
