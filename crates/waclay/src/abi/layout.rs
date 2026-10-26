//! Canonical ABI: static type layout, flattening and core signatures.
//!
//! Implements the rules from the Component Model `CanonicalABI.md` spec:
//! `MAX_FLAT_PARAMS = 16`, `MAX_FLAT_RESULTS = 1`, join-based variant
//! flattening, indirect parameter/return areas, and flags/enum packing.

use crate::component::types::{Primitive, TypeDef, TypeRef};

/// Core wasm value classes used by the Canonical ABI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoreTy {
    I32,
    I64,
    F32,
    F64,
}

impl CoreTy {
    /// Size in bytes of the core type.
    pub fn size(&self) -> u64 {
        match self {
            CoreTy::I32 | CoreTy::F32 => 4,
            CoreTy::I64 | CoreTy::F64 => 8,
        }
    }

    /// The "despecialized" class used for join computations.
    fn class(&self) -> u8 {
        match self {
            CoreTy::I32 | CoreTy::F32 => 0,
            CoreTy::I64 | CoreTy::F64 => 1,
        }
    }

    /// Join of two flat types (variant payload widening).
    pub fn join(a: CoreTy, b: CoreTy) -> CoreTy {
        if a == b {
            return a;
        }
        if a.class() >= b.class() {
            match a.class() {
                0 => CoreTy::I64,
                _ => CoreTy::I64,
            }
        } else {
            CoreTy::I64
        }
    }
}

/// `MAX_FLAT_PARAMS`: beyond this, parameters travel through memory.
pub const MAX_FLAT_PARAMS: usize = 16;
/// `MAX_FLAT_RESULTS`: beyond this, results travel through memory.
pub const MAX_FLAT_RESULTS: usize = 1;
/// `MAX_FLAT_ASYNC_PARAMS`: the (deliberately lower) parameter limit of
/// `async`-lowered core signatures (CanonicalABI.md).
pub const MAX_FLAT_ASYNC_PARAMS: usize = 4;

/// Static view over a component type table (used to walk `TypeRef`s).
pub type TypeTable = [TypeDef];

/// Alignment of a component type in memory (Canonical ABI `align`).
pub fn alignment(ty: &TypeRef, types: &TypeTable) -> u64 {
    match ty {
        TypeRef::Primitive(p) => match p {
            Primitive::Bool | Primitive::S8 | Primitive::U8 => 1,
            Primitive::S16 | Primitive::U16 => 2,
            Primitive::S32
            | Primitive::U32
            | Primitive::F32
            | Primitive::Char
            | Primitive::String => 4,
            Primitive::S64 | Primitive::U64 | Primitive::F64 => 8,
        },
        TypeRef::Def(idx) => {
            if types
                .get(*idx as usize)
                .is_some_and(|d| d.is_resource_marker())
            {
                // a bare resource type in value position denotes an own handle
                return 4;
            }
            match &types[*idx as usize] {
                TypeDef::Primitive(p) => match p {
                    Primitive::Bool | Primitive::S8 | Primitive::U8 => 1,
                    Primitive::S16 | Primitive::U16 => 2,
                    Primitive::S32
                    | Primitive::U32
                    | Primitive::F32
                    | Primitive::Char
                    | Primitive::String => 4,
                    Primitive::S64 | Primitive::U64 | Primitive::F64 => 8,
                },
                TypeDef::Record(fields) => fields
                    .iter()
                    .map(|(_, t)| alignment(t, types))
                    .max()
                    .unwrap_or(1),
                TypeDef::Variant(cases) => {
                    // discriminant alignment joined with payload alignments
                    let mut a = 4u64;
                    for (_, t) in cases.iter() {
                        if let Some(t) = t {
                            a = a.max(alignment(t, types));
                        }
                    }
                    a
                }
                TypeDef::List(_) => 4,
                TypeDef::Tuple(ts) => ts.iter().map(|t| alignment(t, types)).max().unwrap_or(1),
                TypeDef::Flags(names) => match names.len() {
                    0..=32 => 4,
                    33..=64 => 8,
                    _ => 4,
                },
                TypeDef::Enum(_) => 4,
                TypeDef::Option(t) => alignment(t, types).max(1),
                TypeDef::Result { ok, err } => {
                    let mut a = 4u64;
                    for t in [ok, err].into_iter().flatten() {
                        a = a.max(alignment(t, types));
                    }
                    a
                }
                TypeDef::Own(_) | TypeDef::Borrow(_) => 4,
                TypeDef::Stream(_) | TypeDef::Future(_) => 4,
            }
        }
    }
}

/// Byte size of a component type in memory (Canonical ABI `size`).
pub fn size(ty: &TypeRef, types: &TypeTable) -> u64 {
    match ty {
        TypeRef::Primitive(p) => match p {
            Primitive::Bool | Primitive::S8 | Primitive::U8 => 1,
            Primitive::S16 | Primitive::U16 => 2,
            Primitive::S32 | Primitive::U32 | Primitive::F32 | Primitive::Char => 4,
            Primitive::S64 | Primitive::U64 | Primitive::F64 => 8,
            Primitive::String => 8, // (ptr, len) pair, align 4
        },
        TypeRef::Def(idx) => {
            if types
                .get(*idx as usize)
                .is_some_and(|d| d.is_resource_marker())
            {
                // a bare resource type in value position denotes an own handle
                return 4;
            }
            let def = &types[*idx as usize];
            match def {
                TypeDef::Primitive(p) => match p {
                    Primitive::Bool | Primitive::S8 | Primitive::U8 => 1,
                    Primitive::S16 | Primitive::U16 => 2,
                    Primitive::S32 | Primitive::U32 | Primitive::F32 | Primitive::Char => 4,
                    Primitive::S64 | Primitive::U64 | Primitive::F64 => 8,
                    Primitive::String => 8,
                },
                TypeDef::Record(fields) => {
                    let mut off = 0u64;
                    let mut max_align = 1u64;
                    for (_, t) in fields {
                        let a = alignment(t, types);
                        off = align_to(off, a);
                        off += size(t, types);
                        max_align = max_align.max(a);
                    }
                    align_to(off, max_align)
                }
                TypeDef::Variant(cases) => {
                    let mut max_size = 0u64;
                    let mut max_align = 4u64; // discriminant
                    for (_, t) in cases.iter() {
                        if let Some(t) = t {
                            let a = alignment(t, types);
                            max_size = max_size.max(size(t, types));
                            max_align = max_align.max(a);
                        }
                    }
                    align_to(4, max_align) + max_size
                }
                TypeDef::List(_) => 8, // (ptr, len)
                TypeDef::Tuple(ts) => {
                    let mut off = 0u64;
                    let mut max_align = 1u64;
                    for t in ts {
                        let a = alignment(t, types);
                        off = align_to(off, a);
                        off += size(t, types);
                        max_align = max_align.max(a);
                    }
                    align_to(off, max_align)
                }
                TypeDef::Flags(names) => match names.len() {
                    0 => 0,
                    1..=32 => 4,
                    33..=64 => 8,
                    n => (n as u64).div_ceil(32) * 4,
                },
                TypeDef::Enum(_) => 4,
                TypeDef::Option(t) => {
                    let a = alignment(t, types).max(1);
                    align_to(4, a) + size(t, types)
                }
                TypeDef::Result { ok, err } => {
                    let mut max_size = 0u64;
                    let mut max_align = 4u64;
                    for t in [ok, err].into_iter().flatten() {
                        max_size = max_size.max(size(t, types));
                        max_align = max_align.max(a_alignment(t, types));
                    }
                    align_to(4, max_align) + max_size
                }
                TypeDef::Own(_) | TypeDef::Borrow(_) => 4,
                TypeDef::Stream(_) | TypeDef::Future(_) => 4,
            }
        }
    }
}

fn a_alignment(t: &TypeRef, types: &TypeTable) -> u64 {
    alignment(t, types)
}

/// Round `off` up to the next multiple of `align`.
pub fn align_to(off: u64, align: u64) -> u64 {
    if align == 0 {
        return off;
    }
    off.div_ceil(align) * align
}

/// Canonical ABI `flatten_type`: the flat core types a value occupies.
pub fn flat_types(ty: &TypeRef, types: &TypeTable) -> Vec<CoreTy> {
    match ty {
        TypeRef::Primitive(p) => match p {
            Primitive::Bool
            | Primitive::S8
            | Primitive::U8
            | Primitive::S16
            | Primitive::U16
            | Primitive::S32
            | Primitive::U32
            | Primitive::Char => vec![CoreTy::I32],
            Primitive::S64 | Primitive::U64 => vec![CoreTy::I64],
            Primitive::F32 => vec![CoreTy::F32],
            Primitive::F64 => vec![CoreTy::F64],
            Primitive::String => vec![CoreTy::I32, CoreTy::I32],
        },
        TypeRef::Def(idx) => {
            if types
                .get(*idx as usize)
                .is_some_and(|d| d.is_resource_marker())
            {
                // a bare resource type in value position denotes an own handle
                return vec![CoreTy::I32];
            }
            match &types[*idx as usize] {
                TypeDef::Primitive(p) => flat_types(&TypeRef::Primitive(*p), types),
                TypeDef::Record(fields) => {
                    let mut out = Vec::new();
                    for (_, t) in fields {
                        out.extend(flat_types(t, types));
                    }
                    out
                }
                TypeDef::Variant(cases) => variant_flats(cases, types),
                TypeDef::List(_) => vec![CoreTy::I32, CoreTy::I32],
                TypeDef::Tuple(ts) => {
                    let mut out = Vec::new();
                    for t in ts {
                        out.extend(flat_types(t, types));
                    }
                    out
                }
                TypeDef::Flags(names) => match names.len() {
                    0 => vec![],
                    1..=32 => vec![CoreTy::I32],
                    33..=64 => vec![CoreTy::I64],
                    n => vec![CoreTy::I32; n.div_ceil(32)],
                },
                TypeDef::Enum(_) => vec![CoreTy::I32],
                TypeDef::Option(t) => {
                    // despecialized option = variant { none, some(t) }; the case
                    // payloads are resolved against the REAL table, never a
                    // synthetic one (payloads may reference Def indices).
                    variant_flats(
                        &[("none".to_string(), None), ("some".to_string(), Some(*t))],
                        types,
                    )
                }
                TypeDef::Result { ok, err } => {
                    // despecialized result = variant { ok, err }
                    variant_flats(&[("ok".to_string(), *ok), ("err".to_string(), *err)], types)
                }
                TypeDef::Own(_) | TypeDef::Borrow(_) => vec![CoreTy::I32],
                // stream/future ends are waitable handles: a single i32
                TypeDef::Stream(_) | TypeDef::Future(_) => vec![CoreTy::I32],
            }
        }
    }
}

/// Canonical ABI flattening of a variant: `[i32 discriminant] ++ join` of
/// the flattened case payloads (each padded to the widest case).
fn variant_flats(cases: &[(String, Option<TypeRef>)], types: &TypeTable) -> Vec<CoreTy> {
    let mut max_flats: Vec<CoreTy> = Vec::new();
    for (_, t) in cases.iter() {
        if let Some(t) = t {
            let flats = flat_types(t, types);
            for (i, f) in flats.into_iter().enumerate() {
                if let Some(slot) = max_flats.get_mut(i) {
                    *slot = CoreTy::join(*slot, f);
                } else {
                    max_flats.push(f);
                }
            }
        }
    }
    let mut out = vec![CoreTy::I32];
    out.extend(max_flats);
    out
}

/// Computed layout for one parameter or one result slot.
#[derive(Debug, Clone)]
pub struct SlotLayout {
    /// Byte offset inside the (indirect) parameter area.
    pub offset: u64,
    /// The component type stored in this slot.
    pub ty: TypeRef,
}

/// Full computed calling convention for a component function.
#[derive(Debug, Clone)]
pub struct FuncLayout {
    /// Per-parameter layouts (offsets used only when params are indirect).
    pub params: Vec<SlotLayout>,
    /// Result layout (offset 0 when indirect).
    pub result: Option<SlotLayout>,
    /// Flat types of all params concatenated (before adjustment).
    pub param_flat: Vec<CoreTy>,
    /// Flat types of the result (empty if none).
    pub result_flat: Vec<CoreTy>,
    /// Total byte size of the indirect param area (0 when direct).
    pub param_area_size: u64,
    /// Total byte size of the result area (0 when direct).
    pub result_area_size: u64,
    /// True when parameters are passed via a pointer to a memory area.
    pub params_indirect: bool,
    /// True when results are passed via a return pointer.
    pub results_indirect: bool,
}

/// Which side of the call a core signature is computed for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Context {
    /// Function is lifted (guest implements, host calls): heap results come
    /// back as a returned i32 retptr.
    Lift,
    /// Function is lowered (host implements, guest calls): heap results are
    /// written into a retptr the caller passes as an extra trailing param.
    Lower,
    /// Async-lifted (guest implements with the `async` canon option): core
    /// results are the callback packing (`[i32]`) or empty; the actual
    /// result travels through `task.return`.
    AsyncLift,
    /// Async-lowered (host implements with the `async` canon option):
    /// `MAX_FLAT_ASYNC_PARAMS`-limited params, an out-pointer whenever a
    /// result exists, and an `i32` synchronous-completion return.
    AsyncLower,
}

impl FuncLayout {
    /// Compute the complete layout of a component function.
    pub fn new(
        params: &[(String, TypeRef)],
        result: Option<TypeRef>,
        types: &TypeTable,
    ) -> FuncLayout {
        // per-param flat + indirect area offsets
        let mut slots = Vec::with_capacity(params.len());
        let mut flat = Vec::new();
        let mut off = 0u64;
        let mut max_align = 1u64;
        for (_, ty) in params {
            off = align_to(off, alignment(ty, types));
            flat.extend(flat_types(ty, types));
            slots.push(SlotLayout {
                offset: off,
                ty: *ty,
            });
            off += size(ty, types);
            max_align = max_align.max(alignment(ty, types));
        }
        let param_area_size = align_to(off, max_align);

        let (result_slot, result_flat, result_area_size) = match result {
            Some(ty) => {
                let flats = flat_types(&ty, types);
                let sz = size(&ty, types);
                (
                    Some(SlotLayout { offset: 0, ty }),
                    flats,
                    align_to(sz, alignment(&ty, types)),
                )
            }
            None => (None, Vec::new(), 0),
        };

        let params_indirect = flat.len() > MAX_FLAT_PARAMS;
        let results_indirect = result_flat.len() > MAX_FLAT_RESULTS;

        FuncLayout {
            params: slots,
            result: result_slot,
            param_flat: flat,
            result_flat,
            param_area_size,
            result_area_size,
            params_indirect,
            results_indirect,
        }
    }

    /// True when this layout carries params limited to
    /// `MAX_FLAT_ASYNC_PARAMS` (the async-lower ABI) instead of 16.
    pub fn params_indirect_for(&self, ctx: Context) -> bool {
        match ctx {
            Context::AsyncLower => self.param_flat.len() > MAX_FLAT_ASYNC_PARAMS,
            _ => self.params_indirect,
        }
    }

    /// Core parameter types for the given canonical context.
    ///
    /// * `Lift` / `AsyncLift`: `[flat params...]` or `[param-ptr]`.
    /// * `Lower`: `[flat params..., retptr?]` when heap results exist.
    /// * `AsyncLower`: `[flat params..., outptr?]` whenever *any* result
    ///   exists (all results travel through memory under the async ABI),
    ///   with the 4-flat parameter limit.
    pub fn core_params(&self, ctx: Context) -> Vec<CoreTy> {
        match ctx {
            Context::AsyncLower => {
                let mut out = if self.param_flat.len() > MAX_FLAT_ASYNC_PARAMS {
                    vec![CoreTy::I32]
                } else {
                    self.param_flat.clone()
                };
                if !self.result_flat.is_empty() {
                    out.push(CoreTy::I32); // out-pointer
                }
                out
            }
            _ => {
                let mut out = if self.params_indirect {
                    vec![CoreTy::I32]
                } else {
                    self.param_flat.clone()
                };
                if ctx == Context::Lower && self.results_indirect {
                    out.push(CoreTy::I32);
                }
                out
            }
        }
    }

    /// Core result types for the given canonical context.
    pub fn core_results(&self, ctx: Context) -> Vec<CoreTy> {
        match ctx {
            // the synchronous-completion / callback-packing word
            Context::AsyncLower | Context::AsyncLift => vec![CoreTy::I32],
            _ => {
                if self.results_indirect {
                    if ctx == Context::Lift {
                        vec![CoreTy::I32]
                    } else {
                        vec![]
                    }
                } else {
                    self.result_flat.clone()
                }
            }
        }
    }
}
