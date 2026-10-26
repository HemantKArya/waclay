//! Typed parameter/result conversion between Rust values and component `Val`s.
//!
//! The [`ComponentValue`] trait is implemented for every WIT-mappable Rust
//! type; `bindgen!` generates additional impls for user-defined record,
//! variant, enum and flags types.

use crate::component::types::Val;
use crate::{Error, Result};

/// Rust types that map to a component value.
pub trait ComponentValue: Sized {
    /// Convert into the dynamic component representation.
    fn into_val(self) -> Val;
    /// Convert from the dynamic component representation.
    fn from_val(v: &Val) -> Result<Self>;
    /// Whether this type is the WIT "unit" (empty `result` payload etc.).
    fn is_unit() -> bool {
        false
    }
    /// Build the unit value (only valid when [`is_unit`](Self::is_unit)).
    fn from_unit() -> Result<Self> {
        Err(Error::value("unit value not allowed here"))
    }
}

/// Tuple of component values (function parameters).
pub trait ComponentParams: Sized {
    /// Convert into the dynamic component representation, in order.
    fn to_values(self) -> Result<Vec<Val>>;
    /// Build from dynamic values.
    fn from_values(vals: &[Val]) -> Result<Self>;
}

/// Single component result value (or unit for no result).
pub trait ComponentResults: Sized {
    /// Number of result values (0 or 1).
    const COUNT: usize;
    /// Build from an optional result value.
    fn from_result(v: Option<Val>) -> Result<Self>;
    /// Convert into the optional dynamic representation.
    fn to_result(self) -> Result<Option<Val>>;
}

impl ComponentParams for () {
    fn to_values(self) -> Result<Vec<Val>> {
        Ok(Vec::new())
    }
    fn from_values(vals: &[Val]) -> Result<Self> {
        if !vals.is_empty() {
            return Err(Error::value("expected no parameters"));
        }
        Ok(())
    }
}

impl<T: ComponentValue> ComponentResults for T {
    const COUNT: usize = 1;
    fn from_result(v: Option<Val>) -> Result<Self> {
        if T::is_unit() {
            return T::from_unit();
        }
        let v = v.ok_or_else(|| Error::value("missing result value"))?;
        T::from_val(&v)
    }
    fn to_result(self) -> Result<Option<Val>> {
        if T::is_unit() {
            return Ok(None);
        }
        Ok(Some(self.into_val()))
    }
}

macro_rules! primitive_value {
    ($($t:ty => $variant:ident),* $(,)?) => {
        $(
            impl ComponentValue for $t {
                fn into_val(self) -> Val {
                    Val::$variant(self)
                }
                fn from_val(v: &Val) -> Result<Self> {
                    match v {
                        Val::$variant(x) => Ok(*x as $t),
                        other => Err(Error::value(format!(
                            "expected {}, found {}",
                            stringify!($variant),
                            other.type_name()
                        ))),
                    }
                }
            }
        )*
    };
}

primitive_value! {
    bool => Bool,
    i8 => S8,
    u8 => U8,
    i16 => S16,
    u16 => U16,
    i32 => S32,
    u32 => U32,
    i64 => S64,
    u64 => U64,
    f32 => F32,
    f64 => F64,
    char => Char,
}

/// Typed wrapper for one end of a `stream<T>` (bindgen! API).
///
/// WIT `stream<T>` parameters/results surface as this handle; use
/// [`stream_feed`](crate::ComponentInstance::stream_feed) / [`stream_drain`](crate::ComponentInstance::stream_drain)
/// to interact with the underlying pipe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StreamHandle {
    /// Dense stream type index.
    pub ty: u32,
    /// Waitable-table index of this end.
    pub index: u32,
}

impl ComponentValue for StreamHandle {
    fn into_val(self) -> Val {
        Val::Stream {
            ty: self.ty,
            index: self.index,
        }
    }
    fn from_val(v: &Val) -> Result<Self> {
        match v {
            Val::Stream { ty, index } => Ok(StreamHandle {
                ty: *ty,
                index: *index,
            }),
            other => Err(Error::value(format!(
                "expected stream handle, found {}",
                other.type_name()
            ))),
        }
    }
}

/// Typed wrapper for one end of a `future<T>` (bindgen! API).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FutureHandle {
    /// Dense future type index.
    pub ty: u32,
    /// Waitable-table index of this end.
    pub index: u32,
}

impl ComponentValue for FutureHandle {
    fn into_val(self) -> Val {
        Val::Future {
            ty: self.ty,
            index: self.index,
        }
    }
    fn from_val(v: &Val) -> Result<Self> {
        match v {
            Val::Future { ty, index } => Ok(FutureHandle {
                ty: *ty,
                index: *index,
            }),
            other => Err(Error::value(format!(
                "expected future handle, found {}",
                other.type_name()
            ))),
        }
    }
}

impl ComponentValue for String {
    fn into_val(self) -> Val {
        Val::String(self)
    }
    fn from_val(v: &Val) -> Result<Self> {
        match v {
            Val::String(s) => Ok(s.clone()),
            other => Err(Error::value(format!(
                "expected string, found {}",
                other.type_name()
            ))),
        }
    }
}

impl ComponentValue for Val {
    fn into_val(self) -> Val {
        self
    }
    fn from_val(v: &Val) -> Result<Self> {
        Ok(v.clone())
    }
}

impl ComponentValue for () {
    fn into_val(self) -> Val {
        Val::Bool(false)
    }
    fn from_val(_v: &Val) -> Result<Self> {
        Ok(())
    }
    fn is_unit() -> bool {
        true
    }
    fn from_unit() -> Result<Self> {
        Ok(())
    }
}

impl<T: ComponentValue> ComponentValue for Vec<T> {
    fn into_val(self) -> Val {
        Val::List(self.into_iter().map(ComponentValue::into_val).collect())
    }
    fn from_val(v: &Val) -> Result<Self> {
        match v {
            Val::List(items) => items.iter().map(T::from_val).collect(),
            other => Err(Error::value(format!(
                "expected list, found {}",
                other.type_name()
            ))),
        }
    }
}

impl<T: ComponentValue> ComponentValue for Option<T> {
    fn into_val(self) -> Val {
        Val::Option(self.map(|v| Box::new(v.into_val())))
    }
    fn from_val(v: &Val) -> Result<Self> {
        match v {
            Val::Option(inner) => match inner {
                Some(v) => Ok(Some(T::from_val(v)?)),
                None => Ok(None),
            },
            other => Err(Error::value(format!(
                "expected option, found {}",
                other.type_name()
            ))),
        }
    }
}

impl<T: ComponentValue, E: ComponentValue> ComponentValue for std::result::Result<T, E> {
    fn into_val(self) -> Val {
        match self {
            Ok(v) => Val::Result(Ok(if T::is_unit() {
                None
            } else {
                Some(Box::new(v.into_val()))
            })),
            Err(e) => Val::Result(Err(if E::is_unit() {
                None
            } else {
                Some(Box::new(e.into_val()))
            })),
        }
    }
    fn from_val(v: &Val) -> Result<Self> {
        match v {
            Val::Result(Ok(inner)) => Ok(Ok(if T::is_unit() {
                T::from_unit()?
            } else {
                T::from_val(
                    inner
                        .as_deref()
                        .ok_or_else(|| Error::value("missing ok payload"))?,
                )?
            })),
            Val::Result(Err(inner)) => Ok(Err(if E::is_unit() {
                E::from_unit()?
            } else {
                E::from_val(
                    inner
                        .as_deref()
                        .ok_or_else(|| Error::value("missing err payload"))?,
                )?
            })),
            other => Err(Error::value(format!(
                "expected result, found {}",
                other.type_name()
            ))),
        }
    }
}

macro_rules! tuple_params {
    ($($name:ident),+) => {
        impl<$($name: ComponentValue),+> ComponentParams for ($($name,)+) {
            fn to_values(self) -> Result<Vec<Val>> {
                #[allow(non_snake_case)]
                let ($($name,)+) = self;
                Ok(vec![$($name.into_val(),)+])
            }
            fn from_values(vals: &[Val]) -> Result<Self> {
                let mut it = vals.iter();
                #[allow(non_snake_case)]
                Ok(($(
                    {
                        let v = it.next().ok_or_else(|| Error::value("missing parameter value"))?;
                        $name::from_val(v)?
                    },
                )+))
            }
        }
    };
}

tuple_params!(A);
tuple_params!(A, B);
tuple_params!(A, B, C);
tuple_params!(A, B, C, D);
tuple_params!(A, B, C, D, E);
tuple_params!(A, B, C, D, E, F);

/// A typed handle to a component export.
///
/// Created with [`ComponentInstance::typed_func`](crate::ComponentInstance::typed_func).
pub struct TypedFunc<P, R> {
    pub(crate) inner: crate::component::ExportedFunc,
    pub(crate) _marker: std::marker::PhantomData<fn(P) -> R>,
}

impl<P, R> TypedFunc<P, R>
where
    P: ComponentParams,
    R: ComponentResults,
{
    /// Invoke the component function with typed parameters.
    pub fn call(&self, store: &mut crate::Store, params: P) -> Result<R> {
        let vals = params.to_values()?;
        let out = self.inner.call_dynamic(store, &vals)?;
        R::from_result(out)
    }
}

impl<P, R> Clone for TypedFunc<P, R> {
    fn clone(&self) -> Self {
        TypedFunc {
            inner: self.inner.clone(),
            _marker: std::marker::PhantomData,
        }
    }
}

impl<P, R> std::fmt::Debug for TypedFunc<P, R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TypedFunc").finish_non_exhaustive()
    }
}
