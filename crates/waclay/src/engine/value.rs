//! Core value types shared by every engine backend.

use crate::error::{Error, Result};
use std::fmt;

/// Core wasm value type (subset used by the Canonical ABI).
///
/// The component model flattens every WIT type down to exactly these four
/// core value types, so waclay deliberately keeps the abstraction surface
/// this small. (GC/vector types are out of scope for 2.0.)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ValType {
    I32,
    I64,
    F32,
    F64,
}

impl fmt::Display for ValType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValType::I32 => write!(f, "i32"),
            ValType::I64 => write!(f, "i64"),
            ValType::F32 => write!(f, "f32"),
            ValType::F64 => write!(f, "f64"),
        }
    }
}

/// A core wasm value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Val {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
}

impl Val {
    /// Type of this value.
    pub fn ty(&self) -> ValType {
        match self {
            Val::I32(_) => ValType::I32,
            Val::I64(_) => ValType::I64,
            Val::F32(_) => ValType::F32,
            Val::F64(_) => ValType::F64,
        }
    }

    /// Raw i32 view (handles, pointers, enum discriminants...).
    pub fn i32(&self) -> Result<i32> {
        match self {
            Val::I32(v) => Ok(*v),
            other => Err(Error::value(format!("expected i32, found {other:?}"))),
        }
    }

    /// Raw i64 view.
    pub fn i64(&self) -> Result<i64> {
        match self {
            Val::I64(v) => Ok(*v),
            other => Err(Error::value(format!("expected i64, found {other:?}"))),
        }
    }
}

impl From<i32> for Val {
    fn from(v: i32) -> Self {
        Val::I32(v)
    }
}
impl From<i64> for Val {
    fn from(v: i64) -> Self {
        Val::I64(v)
    }
}
impl From<u32> for Val {
    fn from(v: u32) -> Self {
        Val::I32(v as i32)
    }
}
impl From<u64> for Val {
    fn from(v: u64) -> Self {
        Val::I64(v as i64)
    }
}
impl From<f32> for Val {
    fn from(v: f32) -> Self {
        Val::F32(v)
    }
}
impl From<f64> for Val {
    fn from(v: f64) -> Self {
        Val::F64(v)
    }
}

/// Type of a core function.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuncType {
    pub params: Vec<ValType>,
    pub results: Vec<ValType>,
}

impl FuncType {
    pub fn new(params: impl Into<Vec<ValType>>, results: impl Into<Vec<ValType>>) -> Self {
        FuncType {
            params: params.into(),
            results: results.into(),
        }
    }
}

impl fmt::Display for FuncType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ps = self
            .params
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        let rs = self
            .results
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        write!(f, "({ps}) -> ({rs})")
    }
}

/// Type of a linear memory (page counts; 1 page = 64 KiB).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryType {
    pub min: u32,
    pub max: Option<u32>,
}

/// Static description of an external item (import or export).
#[derive(Debug, Clone)]
pub enum ExternType {
    Func(FuncType),
    Memory(MemoryType),
}

impl ExternType {
    /// Declared function type, if this is a function.
    pub fn func(&self) -> Option<&FuncType> {
        match self {
            ExternType::Func(t) => Some(t),
            _ => None,
        }
    }
}

/// A statically known import of a core module.
#[derive(Debug, Clone)]
pub struct ImportType {
    pub module: String,
    pub name: String,
    pub ty: ExternType,
}

/// A statically known export of a core module.
#[derive(Debug, Clone)]
pub struct ExportType {
    pub name: String,
    pub ty: ExternType,
}
