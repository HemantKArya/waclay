//! Canonical ABI: lifting (core → component) and lowering (component → core).
//!
//! Two representation spaces are handled:
//!
//! * **flat space** — the `[i32/i64/f32/f64]` core function signature; used
//!   for direct params/results. Strings, lists, and oversized groups travel
//!   through pointers into linear memory.
//! * **memory space** — the packed in-memory representation (used for list
//!   elements, indirect param areas and retptr areas), driven by
//!   [`load`]/[`store`].

use crate::abi::layout::{align_to, alignment, flat_types, size, CoreTy, TypeTable};
use crate::component::types::{Handle, Primitive, TypeDef, TypeRef, Val};
use crate::engine::{CallerView, Func, Memory, Val as CoreVal};
use crate::{Error, Result};

pub mod layout;

/// String encoding selected by `canon lift` / `canon lower` options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StringEncoding {
    /// UTF-8 (default).
    #[default]
    Utf8,
    /// UTF-16.
    Utf16,
    /// Latin-1 + UTF-16 (compact width).
    CompactUtf16,
}

/// Byte-level access to linear memory, engine-agnostic.
pub trait MemAccess {
    /// Read bytes from memory.
    fn read(&mut self, offset: usize, buf: &mut [u8]) -> Result<()>;
    /// Write bytes to memory.
    fn write(&mut self, offset: usize, buf: &[u8]) -> Result<()>;
    /// Memory length in bytes.
    fn len(&self) -> usize;

    /// True when the memory is empty (never true for live memories; present
    /// to satisfy the clippy `len_without_is_empty` lint).
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Invoke the component's `cabi_realloc`; errors when unavailable.
    fn realloc(&mut self, old_ptr: u32, old_size: u64, align: u64, new_size: u64) -> Result<u32>;
}

/// Adapter over a [`Store`](crate::Store) plus its memory (host-caller side).
pub struct StoreMem<'a> {
    pub store: &'a mut crate::Store,
    pub memory: &'a Memory,
    pub realloc_fn: Option<&'a Func>,
}

impl MemAccess for StoreMem<'_> {
    fn read(&mut self, offset: usize, buf: &mut [u8]) -> Result<()> {
        self.store.memory_read(self.memory, offset, buf)
    }
    fn write(&mut self, offset: usize, buf: &[u8]) -> Result<()> {
        self.store.memory_write(self.memory, offset, buf)
    }
    fn len(&self) -> usize {
        self.store.memory_size(self.memory).unwrap_or(0)
    }
    fn realloc(&mut self, old_ptr: u32, old_size: u64, align: u64, new_size: u64) -> Result<u32> {
        let f = self.realloc_fn.ok_or_else(|| {
            Error::unsupported(
                "component does not export `cabi_realloc`; cannot pass heap parameters",
            )
        })?;
        let mut out = [CoreVal::I32(0)];
        self.store.call(
            f,
            &[
                CoreVal::I32(old_ptr as i32),
                CoreVal::I32(old_size as i32),
                CoreVal::I32(align as i32),
                CoreVal::I32(new_size as i32),
            ],
            &mut out,
        )?;
        Ok(out[0].i32()? as u32)
    }
}

/// Adapter over a [`CallerView`] (host-callee side, inside a guest call).
pub struct CallerMem<'a, 'obj> {
    pub caller: &'a mut (dyn CallerView + 'obj),
    pub memory: &'a Memory,
    pub realloc_fn: Option<&'a Func>,
}

impl MemAccess for CallerMem<'_, '_> {
    fn read(&mut self, offset: usize, buf: &mut [u8]) -> Result<()> {
        self.caller.memory_read(self.memory, offset, buf)
    }
    fn write(&mut self, offset: usize, buf: &[u8]) -> Result<()> {
        self.caller.memory_write(self.memory, offset, buf)
    }
    fn len(&self) -> usize {
        self.caller.memory_size(self.memory)
    }
    fn realloc(&mut self, old_ptr: u32, old_size: u64, align: u64, new_size: u64) -> Result<u32> {
        let f = self
            .realloc_fn
            .ok_or_else(|| Error::unsupported("no realloc available for lowered function"))?;
        let mut out = [CoreVal::I32(0)];
        self.caller.call(
            f,
            &[
                CoreVal::I32(old_ptr as i32),
                CoreVal::I32(old_size as i32),
                CoreVal::I32(align as i32),
                CoreVal::I32(new_size as i32),
            ],
            &mut out,
        )?;
        Ok(out[0].i32()? as u32)
    }
}

// ---------------------------------------------------------------------------
// Flat cursor
// ---------------------------------------------------------------------------

/// Cursor over the flat core values of one call.
pub struct FlatCursor {
    pub vals: Vec<CoreVal>,
    pub idx: usize,
}

impl FlatCursor {
    pub fn new(vals: Vec<CoreVal>) -> Self {
        FlatCursor { vals, idx: 0 }
    }

    fn next(&mut self, expect: CoreTy) -> Result<CoreVal> {
        let v = self
            .vals
            .get(self.idx)
            .copied()
            .ok_or_else(|| Error::value("flat value stream exhausted"))?;
        let ok = match (v, expect) {
            (CoreVal::I32(_), CoreTy::I32) => true,
            (CoreVal::I64(_), CoreTy::I64) => true,
            (CoreVal::F32(_), CoreTy::F32) => true,
            (CoreVal::F64(_), CoreTy::F64) => true,
            // engines may hand back f32 in an i32 slot etc.; accept & coerce
            (CoreVal::I32(_), CoreTy::F32) => true,
            (CoreVal::I64(_), CoreTy::F64) => true,
            _ => false,
        };
        if !ok {
            return Err(Error::value(format!(
                "flat type mismatch: expected {expect:?}, got {:?}",
                v.ty()
            )));
        }
        self.idx += 1;
        Ok(v)
    }

    fn i32(&mut self) -> Result<i32> {
        match self.next(CoreTy::I32)? {
            CoreVal::I32(v) => Ok(v),
            CoreVal::I64(v) => Ok(v as i32),
            _ => unreachable!(),
        }
    }

    fn i64(&mut self) -> Result<i64> {
        match self.next(CoreTy::I64)? {
            CoreVal::I64(v) => Ok(v),
            CoreVal::I32(v) => Ok(v as i64),
            _ => unreachable!(),
        }
    }

    fn f32(&mut self) -> Result<f32> {
        match self.next(CoreTy::F32)? {
            CoreVal::F32(v) => Ok(v),
            CoreVal::I32(v) => Ok(f32::from_bits(v as u32)),
            CoreVal::I64(v) => Ok(f32::from_bits(v as u32)),
            _ => unreachable!(),
        }
    }

    fn f64(&mut self) -> Result<f64> {
        match self.next(CoreTy::F64)? {
            CoreVal::F64(v) => Ok(v),
            CoreVal::I64(v) => Ok(f64::from_bits(v as u64)),
            _ => unreachable!(),
        }
    }
}

// ---------------------------------------------------------------------------
// Strings
// ---------------------------------------------------------------------------

fn encoding_align(enc: StringEncoding) -> u64 {
    match enc {
        StringEncoding::Utf8 => 1,
        StringEncoding::Utf16 | StringEncoding::CompactUtf16 => 2,
    }
}

/// The UTF-16 tag bit of the Canonical ABI: the top bit of the length word
/// marks "stored as UTF-16" under the `latin1+utf16` (compact) encoding.
/// See CanonicalABI.md `utf16_tag` (for 32-bit pointers).
const UTF16_TAG: u32 = 1 << 31;

/// Reject a guest-supplied `count * elem_size` byte span that does not fit in
/// linear memory *before* allocating for it, so a hostile guest cannot make
/// the host reserve gigabytes with a forged length word.
fn check_span(mem: &dyn MemAccess, ptr: u64, count: u64, elem_size: u64) -> Result<()> {
    let end = count
        .checked_mul(elem_size)
        .and_then(|n| n.checked_add(ptr))
        .ok_or_else(|| Error::value("guest length overflows the address space"))?;
    if end > mem.len() as u64 {
        return Err(Error::trap(format!(
            "out of bounds guest memory access: {count} x {elem_size} bytes at {ptr:#x}"
        )));
    }
    if elem_size == 0 && count > MAX_ZERO_SIZED_ELEMS {
        return Err(Error::unsupported(format!(
            "list of {count} zero-sized elements"
        )));
    }
    Ok(())
}

/// Upper bound on zero-sized list elements, which occupy no memory and so are
/// not covered by the bounds check above.
const MAX_ZERO_SIZED_ELEMS: u64 = 1 << 20;

pub(crate) fn read_string(
    mem: &mut dyn MemAccess,
    ptr: u32,
    tagged: u32,
    enc: StringEncoding,
) -> Result<Val> {
    match enc {
        StringEncoding::Utf8 => {
            check_span(mem, ptr as u64, tagged as u64, 1)?;
            let mut bytes = vec![0u8; tagged as usize];
            mem.read(ptr as usize, &mut bytes)?;
            Ok(Val::String(String::from_utf8(bytes)?))
        }
        StringEncoding::Utf16 => {
            check_span(mem, ptr as u64, tagged as u64, 2)?;
            let mut bytes = vec![0u8; tagged as usize * 2];
            mem.read(ptr as usize, &mut bytes)?;
            let units_u16: Vec<u16> = bytes
                .as_chunks::<2>()
                .0
                .iter()
                .map(|c| u16::from_le_bytes(*c))
                .collect();
            let s = char::decode_utf16(units_u16)
                .collect::<std::result::Result<String, _>>()
                .map_err(|_| Error::value("invalid utf-16 in guest string"))?;
            Ok(Val::String(s))
        }
        StringEncoding::CompactUtf16 => {
            // The length word is tagged: top bit set -> UTF-16 storage,
            // otherwise Latin-1 with `tagged` bytes (CanonicalABI.md
            // `load_string_from_range`).
            if tagged & UTF16_TAG != 0 {
                let units = tagged & !UTF16_TAG;
                check_span(mem, ptr as u64, units as u64, 2)?;
                let mut bytes = vec![0u8; units as usize * 2];
                mem.read(ptr as usize, &mut bytes)?;
                let units_u16: Vec<u16> = bytes
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|c| u16::from_le_bytes(*c))
                    .collect();
                let s = char::decode_utf16(units_u16)
                    .collect::<std::result::Result<String, _>>()
                    .map_err(|_| Error::value("invalid utf-16 in guest string"))?;
                Ok(Val::String(s))
            } else {
                check_span(mem, ptr as u64, tagged as u64, 1)?;
                let mut bytes = vec![0u8; tagged as usize];
                mem.read(ptr as usize, &mut bytes)?;
                Ok(Val::String(bytes.iter().map(|b| *b as char).collect()))
            }
        }
    }
}

pub(crate) fn write_string(
    mem: &mut dyn MemAccess,
    s: &str,
    enc: StringEncoding,
) -> Result<(u32, u32)> {
    // The (ptr, len) pair carries the length in the encoding's own unit:
    // UTF-8 -> bytes, UTF-16/compact -> UTF-16 code units. `read_string`
    // interprets it the same way, so keep both sides symmetric.
    let (bytes, units): (Vec<u8>, u32) = match enc {
        StringEncoding::Utf8 => (s.as_bytes().to_vec(), s.len() as u32),
        StringEncoding::Utf16 => {
            let mut b = Vec::with_capacity(s.len() * 2);
            for u in s.encode_utf16() {
                b.extend_from_slice(&u.to_le_bytes());
            }
            let units = s.encode_utf16().count() as u32;
            (b, units)
        }
        StringEncoding::CompactUtf16 => {
            if s.chars().all(|c| (c as u32) < 256) {
                // latin-1: one byte per code point, plain (untagged) length
                let units = s.chars().count() as u32;
                (s.chars().map(|c| c as u8).collect(), units)
            } else {
                // UTF-16 storage: tag the length word so the reader knows
                let mut b = Vec::with_capacity(s.len() * 2);
                for u in s.encode_utf16() {
                    b.extend_from_slice(&u.to_le_bytes());
                }
                let units = s.encode_utf16().count() as u32;
                (b, units | UTF16_TAG)
            }
        }
    };
    if bytes.is_empty() {
        return Ok((0, 0));
    }
    let ptr = mem.realloc(0, 0, encoding_align(enc), bytes.len() as u64)?;
    mem.write(ptr as usize, &bytes)?;
    Ok((ptr, units))
}

// ---------------------------------------------------------------------------
// Lifting: flat stream / memory → Val
// ---------------------------------------------------------------------------

/// Lift a value from the flat value stream.
pub fn lift(
    ty: &TypeRef,
    types: &TypeTable,
    cur: &mut FlatCursor,
    mem: &mut dyn MemAccess,
    enc: StringEncoding,
) -> Result<Val> {
    match ty {
        TypeRef::Primitive(p) => Ok(match p {
            Primitive::Bool => Val::Bool(cur.i32()? != 0),
            Primitive::S8 => Val::S8(cur.i32()? as i8),
            Primitive::U8 => Val::U8(cur.i32()? as u8),
            Primitive::S16 => Val::S16(cur.i32()? as i16),
            Primitive::U16 => Val::U16(cur.i32()? as u16),
            Primitive::S32 => Val::S32(cur.i32()?),
            Primitive::U32 => Val::U32(cur.i32()? as u32),
            Primitive::S64 => Val::S64(cur.i64()?),
            Primitive::U64 => Val::U64(cur.i64()? as u64),
            Primitive::F32 => Val::F32(cur.f32()?),
            Primitive::F64 => Val::F64(cur.f64()?),
            Primitive::Char => {
                let bits = cur.i32()? as u32;
                let c = char::from_u32(bits)
                    .ok_or_else(|| Error::value(format!("invalid char 0x{bits:08x}")))?;
                Val::Char(c)
            }
            Primitive::String => {
                let ptr = cur.i32()? as u32;
                let units = cur.i32()? as u32;
                read_string(mem, ptr, units, enc)?
            }
        }),
        TypeRef::Def(idx) => {
            // a bare resource type in value position denotes an own handle
            if types
                .get(*idx as usize)
                .is_some_and(|d| d.is_resource_marker())
            {
                return Ok(Val::Handle(Handle::Own {
                    resource: *idx,
                    index: cur.i32()? as u32,
                }));
            }
            Ok(match &types[*idx as usize] {
                TypeDef::Primitive(p) => lift(&TypeRef::Primitive(*p), types, cur, mem, enc)?,
                TypeDef::Record(fields) => {
                    let mut out = Vec::with_capacity(fields.len());
                    for (name, fty) in fields {
                        out.push((name.clone(), lift(fty, types, cur, mem, enc)?));
                    }
                    Val::Record(out)
                }
                TypeDef::Tuple(items) => {
                    let mut out = Vec::with_capacity(items.len());
                    for it in items {
                        out.push(lift(it, types, cur, mem, enc)?);
                    }
                    Val::Tuple(out)
                }
                TypeDef::List(et) => {
                    let ptr = cur.i32()? as u32;
                    let len = cur.i32()? as u32;
                    let elem_size = size(et, types);
                    check_span(mem, ptr as u64, len as u64, elem_size)?;
                    let mut out = Vec::with_capacity(len as usize);
                    for i in 0..len {
                        let addr = ptr as u64 + i as u64 * elem_size;
                        out.push(load(et, types, addr, mem, enc)?);
                    }
                    Val::List(out)
                }
                TypeDef::Variant(cases) => lift_variant(cases, types, cur, mem, enc)?,
                TypeDef::Enum(names) => {
                    let disc = cur.i32()? as u32;
                    let name = names
                        .get(disc as usize)
                        .ok_or_else(|| {
                            Error::value(format!("enum discriminant {disc} out of range"))
                        })?
                        .clone();
                    Val::Enum(name)
                }
                TypeDef::Option(inner) => {
                    // option<T> == variant { none, some(T) }; route through
                    // lift_variant so the full joined width is always consumed.
                    let cases = [
                        (String::from("none"), None),
                        (String::from("some"), Some(*inner)),
                    ];
                    match lift_variant(&cases, types, cur, mem, enc)? {
                        Val::Variant { case, payload } => match case.as_str() {
                            "none" => Val::Option(None),
                            _ => Val::Option(payload),
                        },
                        _ => return Err(Error::value("option lift produced non-variant")),
                    }
                }
                TypeDef::Result { ok, err } => {
                    // result<T, E> == variant { ok(T), err(E) }
                    let cases = [(String::from("ok"), *ok), (String::from("err"), *err)];
                    match lift_variant(&cases, types, cur, mem, enc)? {
                        Val::Variant { case, payload } => match case.as_str() {
                            "ok" => Val::Result(Ok(payload)),
                            _ => Val::Result(Err(payload)),
                        },
                        _ => return Err(Error::value("result lift produced non-variant")),
                    }
                }
                TypeDef::Flags(_) => {
                    let names = flags_from_flat(ty, types, cur)?;
                    Val::Flags(names)
                }
                TypeDef::Own(r) => Val::Handle(Handle::Own {
                    resource: *r,
                    index: cur.i32()? as u32,
                }),
                TypeDef::Borrow(r) => Val::Handle(Handle::Borrow {
                    resource: *r,
                    index: cur.i32()? as u32,
                }),
                // stream/future ends lift to opaque waitable handles
                TypeDef::Stream(_) => Val::Stream {
                    ty: *idx,
                    index: cur.i32()? as u32,
                },
                TypeDef::Future(_) => Val::Future {
                    ty: *idx,
                    index: cur.i32()? as u32,
                },
            })
        }
    }
}

fn lift_variant(
    cases: &[(String, Option<TypeRef>)],
    types: &TypeTable,
    cur: &mut FlatCursor,
    mem: &mut dyn MemAccess,
    enc: StringEncoding,
) -> Result<Val> {
    let disc = cur.i32()? as u32;
    let (name, payload_ty) = cases
        .get(disc as usize)
        .ok_or_else(|| Error::value(format!("variant discriminant {disc} out of range")))?
        .clone();

    // consume the full joined width, then interpret the leading slots
    let width = variant_flat_width(cases, types);
    let joined_tys = variant_joined_flats(cases, types);
    let mut payload_vals = Vec::with_capacity(width);
    for _ in 0..width {
        payload_vals.push(match cur.vals.get(cur.idx) {
            Some(_) => cur.vals[cur.idx],
            None => CoreVal::I32(0),
        });
        cur.idx += 1;
    }

    let payload = match payload_ty {
        Some(pt) => {
            // Reinterpret each joined slot back into the case's own flat type.
            let case_flats = flat_types(&pt, types);
            let converted: Vec<CoreVal> = (0..width)
                .map(|i| {
                    convert_from_slot(
                        payload_vals[i],
                        joined_tys.get(i).copied(),
                        case_flats.get(i).copied(),
                    )
                })
                .collect();
            let mut sub = FlatCursor::new(converted);
            Some(Box::new(lift(&pt, types, &mut sub, mem, enc)?))
        }
        None => None,
    };
    Ok(Val::Variant {
        case: name,
        payload,
    })
}

/// Lower one variant case into a flat-params sink: push the payload using the
/// case's own flat types, then widen/pad every slot to the JOINED width with
/// correctly-typed values (Canonical ABI `flatten_variant`).
fn push_case_flat(
    cases: &[(String, Option<TypeRef>)],
    disc: usize,
    types: &TypeTable,
    payload: Option<&Val>,
    sink: &mut FlatSink,
    mem: &mut dyn MemAccess,
    enc: StringEncoding,
) -> Result<()> {
    let width = variant_flat_width(cases, types);
    let joined = variant_joined_flats(cases, types);
    let mut sub = FlatSink::default();
    if let (Some(pt), Some(p)) = (&cases[disc].1, payload) {
        lower(pt, types, p, &mut sub, mem, enc)?;
    }
    for i in 0..width {
        let slot = joined.get(i).copied();
        match sub.vals.get(i) {
            Some(v) => sink.vals.push(convert_to_slot(*v, slot)),
            None => sink.vals.push(slot_zero(slot)),
        }
    }
    Ok(())
}

pub fn variant_flat_width(cases: &[(String, Option<TypeRef>)], types: &TypeTable) -> usize {
    cases
        .iter()
        .filter_map(|(_, t)| t.as_ref())
        .map(|t| flat_types(t, types).len())
        .max()
        .unwrap_or(0)
}

/// The joined flat types of a variant payload (no discriminant).
pub fn variant_joined_flats(cases: &[(String, Option<TypeRef>)], types: &TypeTable) -> Vec<CoreTy> {
    let mut out: Vec<CoreTy> = Vec::new();
    for (_, t) in cases {
        if let Some(t) = t {
            for (i, f) in flat_types(t, types).into_iter().enumerate() {
                if let Some(slot) = out.get_mut(i) {
                    *slot = CoreTy::join(*slot, f);
                } else {
                    out.push(f);
                }
            }
        }
    }
    out
}

fn flags_from_flat(ty: &TypeRef, types: &TypeTable, cur: &mut FlatCursor) -> Result<Vec<String>> {
    let TypeDef::Flags(names) = &types[match ty {
        TypeRef::Def(i) => *i as usize,
        _ => return Err(Error::value("flags type expected")),
    }] else {
        return Err(Error::value("flags type expected"));
    };
    let n = names.len() as u32;
    let mut out = Vec::new();
    match n {
        0 => {}
        1..=32 => {
            let bits = cur.i32()? as u32;
            for (i, name) in names.iter().enumerate() {
                if bits & (1 << i) != 0 {
                    out.push(name.clone());
                }
            }
        }
        33..=64 => {
            let bits = cur.i64()? as u64;
            for (i, name) in names.iter().enumerate() {
                if bits & (1u64 << i) != 0 {
                    out.push(name.clone());
                }
            }
        }
        _ => {
            let chunks = (n as usize).div_ceil(32);
            for (c, name) in names.iter().enumerate() {
                if c >= chunks {
                    break;
                }
                let bits = cur.i32()? as u32;
                if bits & (1 << (c % 32)) != 0 {
                    out.push(name.clone());
                }
            }
        }
    }
    Ok(out)
}

/// Memory-representation flags loader (used by [`load`]).
fn flags_from_flat_mem(
    ty: &TypeRef,
    types: &TypeTable,
    addr: u64,
    mem: &mut dyn MemAccess,
) -> Result<Vec<String>> {
    let TypeDef::Flags(names) = &types[match ty {
        TypeRef::Def(i) => *i as usize,
        _ => return Err(Error::value("flags type expected")),
    }] else {
        return Err(Error::value("flags type expected"));
    };
    let n = names.len() as u32;
    let mut out = Vec::new();
    match n {
        0 => {}
        1..=32 => {
            let mut b = [0u8; 4];
            mem.read(addr as usize, &mut b)?;
            let bits = u32::from_le_bytes(b);
            for (i, name) in names.iter().enumerate() {
                if bits & (1 << i) != 0 {
                    out.push(name.clone());
                }
            }
        }
        33..=64 => {
            let mut b = [0u8; 8];
            mem.read(addr as usize, &mut b)?;
            let bits = u64::from_le_bytes(b);
            for (i, name) in names.iter().enumerate() {
                if bits & (1u64 << i) != 0 {
                    out.push(name.clone());
                }
            }
        }
        _ => {
            let chunks = (n as usize).div_ceil(32);
            for (c, name) in names.iter().enumerate() {
                if c >= chunks {
                    break;
                }
                let mut b = [0u8; 4];
                mem.read(addr as usize + c * 4, &mut b)?;
                let bits = u32::from_le_bytes(b);
                if bits & (1 << (c % 32)) != 0 {
                    out.push(name.clone());
                }
            }
        }
    }
    Ok(out)
}

/// Load a value from its **memory representation** at `addr`.
pub fn load(
    ty: &TypeRef,
    types: &TypeTable,
    addr: u64,
    mem: &mut dyn MemAccess,
    enc: StringEncoding,
) -> Result<Val> {
    match ty {
        TypeRef::Primitive(p) => {
            let mut tmp = [0u8; 8];
            let (n, dec): (usize, fn(&[u8]) -> Val) = match p {
                Primitive::Bool => (1, |b| Val::Bool(b[0] != 0)),
                Primitive::S8 => (1, |b| Val::S8(b[0] as i8)),
                Primitive::U8 => (1, |b| Val::U8(b[0])),
                Primitive::S16 => (2, |b| Val::S16(i16::from_le_bytes([b[0], b[1]]))),
                Primitive::U16 => (2, |b| Val::U16(u16::from_le_bytes([b[0], b[1]]))),
                Primitive::S32 => (4, |b| {
                    Val::S32(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                }),
                Primitive::U32 => (4, |b| {
                    Val::U32(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                }),
                Primitive::S64 => (8, |b| {
                    Val::S64(i64::from_le_bytes(b[..8].try_into().unwrap()))
                }),
                Primitive::U64 => (8, |b| {
                    Val::U64(u64::from_le_bytes(b[..8].try_into().unwrap()))
                }),
                Primitive::F32 => (4, |b| {
                    Val::F32(f32::from_bits(u32::from_le_bytes([b[0], b[1], b[2], b[3]])))
                }),
                Primitive::F64 => (8, |b| {
                    Val::F64(f64::from_bits(u64::from_le_bytes(
                        b[..8].try_into().unwrap(),
                    )))
                }),
                Primitive::Char => (4, |b| {
                    let bits = u32::from_le_bytes([b[0], b[1], b[2], b[3]]);
                    Val::Char(char::from_u32(bits).unwrap_or('\u{FFFD}'))
                }),
                Primitive::String => (0, |_| Val::Bool(false)), // handled below
            };
            if *p == Primitive::String {
                mem.read(addr as usize, &mut tmp[..8])?;
                let ptr = u32::from_le_bytes(tmp[..4].try_into().unwrap());
                let units = u32::from_le_bytes(tmp[4..8].try_into().unwrap());
                return read_string(mem, ptr, units, enc);
            }
            mem.read(addr as usize, &mut tmp[..n])?;
            Ok(dec(&tmp))
        }
        TypeRef::Def(idx) => {
            let mut def = types[*idx as usize].clone();
            if def.is_resource_marker() {
                // a bare resource type in value position denotes an own handle
                def = TypeDef::Own(*idx);
            }
            match def {
                TypeDef::Primitive(p) => load(&TypeRef::Primitive(p), types, addr, mem, enc),
                TypeDef::Record(fields) => {
                    let mut out = Vec::with_capacity(fields.len());
                    let mut off = 0u64;
                    let mut max_align = 1u64;
                    for (name, fty) in &fields {
                        let a = alignment(fty, types);
                        off = align_to(off, a);
                        out.push((name.clone(), load(fty, types, addr + off, mem, enc)?));
                        off += size(fty, types);
                        max_align = max_align.max(a);
                    }
                    let _ = align_to(off, max_align);
                    Ok(Val::Record(out))
                }
                TypeDef::Tuple(items) => {
                    let mut out = Vec::with_capacity(items.len());
                    let mut off = 0u64;
                    for it in &items {
                        let a = alignment(it, types);
                        off = align_to(off, a);
                        out.push(load(it, types, addr + off, mem, enc)?);
                        off += size(it, types);
                    }
                    Ok(Val::Tuple(out))
                }
                TypeDef::List(et) => {
                    let mut tmp = [0u8; 8];
                    mem.read(addr as usize, &mut tmp)?;
                    let ptr = u32::from_le_bytes(tmp[..4].try_into().unwrap());
                    let len = u32::from_le_bytes(tmp[4..8].try_into().unwrap());
                    let elem_size = size(&et, types);
                    check_span(mem, ptr as u64, len as u64, elem_size)?;
                    let mut out = Vec::with_capacity(len as usize);
                    for i in 0..len {
                        out.push(load(
                            &et,
                            types,
                            ptr as u64 + i as u64 * elem_size,
                            mem,
                            enc,
                        )?);
                    }
                    Ok(Val::List(out))
                }
                TypeDef::Variant(cases) => {
                    let mut tmp = [0u8; 4];
                    mem.read(addr as usize, &mut tmp)?;
                    let disc = u32::from_le_bytes(tmp);
                    let (name, payload_ty) = cases
                        .get(disc as usize)
                        .cloned()
                        .ok_or_else(|| Error::value("variant discriminant out of range"))?;
                    let payload = match payload_ty {
                        Some(pt) => {
                            let a = alignment(&pt, types).max(1);
                            let paddr = addr + align_to(4, a);
                            Some(Box::new(load(&pt, types, paddr, mem, enc)?))
                        }
                        None => None,
                    };
                    Ok(Val::Variant {
                        case: name,
                        payload,
                    })
                }
                TypeDef::Enum(n) => {
                    let mut tmp = [0u8; 4];
                    mem.read(addr as usize, &mut tmp)?;
                    let disc = u32::from_le_bytes(tmp) as usize;
                    if disc >= n.len() {
                        return Err(Error::value("enum discriminant out of range"));
                    }
                    Ok(Val::Enum(n[disc].clone()))
                }
                TypeDef::Option(inner) => {
                    let mut tmp = [0u8; 4];
                    mem.read(addr as usize, &mut tmp)?;
                    let disc = u32::from_le_bytes(tmp);
                    match disc {
                        0 => Ok(Val::Option(None)),
                        1 => {
                            let a = alignment(&inner, types).max(1);
                            let paddr = addr + align_to(4, a);
                            Ok(Val::Option(Some(Box::new(load(
                                &inner, types, paddr, mem, enc,
                            )?))))
                        }
                        d => Err(Error::value(format!("option discriminant {d} invalid"))),
                    }
                }
                TypeDef::Result { ok, err } => {
                    let mut tmp = [0u8; 4];
                    mem.read(addr as usize, &mut tmp)?;
                    let disc = u32::from_le_bytes(tmp);
                    let mut payload = |t: &Option<TypeRef>| -> Result<Option<Box<Val>>> {
                        match t {
                            Some(t) => {
                                let a = alignment(t, types).max(1);
                                let paddr = addr + align_to(4, a);
                                Ok(Some(Box::new(load(t, types, paddr, mem, enc)?)))
                            }
                            None => Ok(None),
                        }
                    };
                    match disc {
                        0 => Ok(Val::Result(Ok(payload(&ok)?))),
                        1 => Ok(Val::Result(Err(payload(&err)?))),
                        d => Err(Error::value(format!("result discriminant {d} invalid"))),
                    }
                }
                TypeDef::Flags(_) => {
                    let names = flags_from_flat_mem(ty, types, addr, mem)?;
                    Ok(Val::Flags(names))
                }
                TypeDef::Own(r) => {
                    let h = load_handle(addr, mem)?;
                    Ok(Val::Handle(Handle::Own {
                        resource: r,
                        index: h,
                    }))
                }
                TypeDef::Borrow(r) => {
                    let h = load_handle(addr, mem)?;
                    Ok(Val::Handle(Handle::Borrow {
                        resource: r,
                        index: h,
                    }))
                }
                TypeDef::Stream(_) | TypeDef::Future(_) => {
                    let h = load_handle(addr, mem)?;
                    let is_stream = matches!(def, TypeDef::Stream(_));
                    Ok(if is_stream {
                        Val::Stream { ty: *idx, index: h }
                    } else {
                        Val::Future { ty: *idx, index: h }
                    })
                }
            }
        }
    }
}

fn load_handle(addr: u64, mem: &mut dyn MemAccess) -> Result<u32> {
    let mut b = [0u8; 4];
    mem.read(addr as usize, &mut b)?;
    Ok(u32::from_le_bytes(b))
}

// ---------------------------------------------------------------------------
// Lowering: Val → flat stream / memory
// ---------------------------------------------------------------------------

/// Accumulator for lowered flat values.
#[derive(Default)]
pub struct FlatSink {
    pub vals: Vec<CoreVal>,
}

impl FlatSink {
    pub fn push_i32(&mut self, v: i32) {
        self.vals.push(CoreVal::I32(v));
    }
    fn push_i64(&mut self, v: i64) {
        self.vals.push(CoreVal::I64(v));
    }
    fn push_f32(&mut self, v: f32) {
        self.vals.push(CoreVal::F32(v));
    }
    fn push_f64(&mut self, v: f64) {
        self.vals.push(CoreVal::F64(v));
    }
}

/// Lower a value into the flat value stream (heap data written via `mem`).
pub fn lower(
    ty: &TypeRef,
    types: &TypeTable,
    val: &Val,
    sink: &mut FlatSink,
    mem: &mut dyn MemAccess,
    enc: StringEncoding,
) -> Result<()> {
    match ty {
        TypeRef::Primitive(p) => match (p, val) {
            (Primitive::Bool, Val::Bool(b)) => sink.push_i32(*b as i32),
            (Primitive::S8, Val::S8(v)) => sink.push_i32(*v as i32),
            (Primitive::U8, Val::U8(v)) => sink.push_i32(*v as i32),
            (Primitive::S16, Val::S16(v)) => sink.push_i32(*v as i32),
            (Primitive::U16, Val::U16(v)) => sink.push_i32(*v as i32),
            (Primitive::S32, Val::S32(v)) => sink.push_i32(*v),
            (Primitive::U32, Val::U32(v)) => sink.push_i32(*v as i32),
            (Primitive::S64, Val::S64(v)) => sink.push_i64(*v),
            (Primitive::U64, Val::U64(v)) => sink.push_i64(*v as i64),
            (Primitive::F32, Val::F32(v)) => sink.push_f32(*v),
            (Primitive::F64, Val::F64(v)) => sink.push_f64(*v),
            (Primitive::Char, Val::Char(c)) => sink.push_i32(*c as u32 as i32),
            (Primitive::String, Val::String(s)) => {
                let (ptr, units) = write_string(mem, s, enc)?;
                sink.push_i32(ptr as i32);
                sink.push_i32(units as i32);
            }
            (p, v) => {
                return Err(Error::value(format!(
                    "cannot lower {} as {p}",
                    v.type_name()
                )))
            }
        },
        TypeRef::Def(idx) => {
            let mut def = types[*idx as usize].clone();
            if def.is_resource_marker() {
                // a bare resource type in value position denotes an own handle
                def = TypeDef::Own(*idx);
            }
            match (&def, val) {
                (TypeDef::Primitive(p), _) => {
                    return lower(&TypeRef::Primitive(*p), types, val, sink, mem, enc)
                }
                (TypeDef::Record(fields), Val::Record(vals)) => {
                    for (name, fty) in fields {
                        let v = field_val(vals, name)?;
                        lower(fty, types, v, sink, mem, enc)?;
                    }
                }
                (TypeDef::Tuple(items), Val::Tuple(vals)) => {
                    for (it, v) in items.iter().zip(vals.iter()) {
                        lower(it, types, v, sink, mem, enc)?;
                    }
                }
                (TypeDef::List(et), Val::List(items)) => {
                    let elem_size = size(et, types);
                    let total = elem_size * items.len() as u64;
                    let ptr = if total == 0 {
                        0u32
                    } else {
                        let p = mem.realloc(0, 0, alignment(et, types), total)?;
                        for (i, item) in items.iter().enumerate() {
                            store(et, types, item, p as u64 + i as u64 * elem_size, mem, enc)?;
                        }
                        p
                    };
                    sink.push_i32(ptr as i32);
                    sink.push_i32(items.len() as i32);
                }
                (TypeDef::Variant(cases), Val::Variant { case, payload }) => {
                    let disc = cases
                        .iter()
                        .position(|(n, _)| n == case)
                        .ok_or_else(|| Error::value(format!("unknown variant case `{case}`")))?;
                    sink.push_i32(disc as i32);
                    push_case_flat(cases, disc, types, payload.as_deref(), sink, mem, enc)?;
                }
                (TypeDef::Enum(names), Val::Enum(name)) => {
                    let disc = names
                        .iter()
                        .position(|n| n == name)
                        .ok_or_else(|| Error::value(format!("unknown enum case `{name}`")))?;
                    sink.push_i32(disc as i32);
                }
                (TypeDef::Option(inner), Val::Option(opt)) => {
                    // option<T> == variant { none, some(T) }
                    let cases = [
                        (String::from("none"), None),
                        (String::from("some"), Some(*inner)),
                    ];
                    let disc = match opt {
                        Some(_) => 1usize,
                        None => 0usize,
                    };
                    sink.push_i32(disc as i32);
                    push_case_flat(&cases, disc, types, opt.as_deref(), sink, mem, enc)?;
                }
                (TypeDef::Result { ok, err }, Val::Result(res)) => {
                    // result<T, E> == variant { ok(T), err(E) }
                    let cases = [(String::from("ok"), *ok), (String::from("err"), *err)];
                    match res {
                        Ok(p) => {
                            sink.push_i32(0);
                            push_case_flat(&cases, 0, types, p.as_deref(), sink, mem, enc)?
                        }
                        Err(p) => {
                            sink.push_i32(1);
                            push_case_flat(&cases, 1, types, p.as_deref(), sink, mem, enc)?
                        }
                    }
                }
                (TypeDef::Flags(_), Val::Flags(set)) => {
                    flags_to_flat(ty, types, set, sink)?;
                }
                (TypeDef::Own(_), Val::Handle(Handle::Own { index, .. })) => {
                    // the pushed i32 is context-dependent: the rep for
                    // lowered results (the shim wraps it via resource-new),
                    // or a table handle for lifted params
                    sink.push_i32(*index as i32);
                }
                (TypeDef::Borrow(_), Val::Handle(Handle::Borrow { index, .. })) => {
                    sink.push_i32(*index as i32);
                }
                (TypeDef::Stream(_), Val::Stream { index, .. })
                | (TypeDef::Future(_), Val::Future { index, .. }) => {
                    sink.push_i32(*index as i32);
                }
                (d, v) => {
                    return Err(Error::value(format!(
                        "cannot lower {} as {}",
                        v.type_name(),
                        d.describe(types)
                    )))
                }
            }
        }
    }
    Ok(())
}

/// Convert a case payload's own flat value into the JOINED slot representation
/// (Canonical ABI `flatten_variant` widening). Ints and floats share slots by
/// bit-reinterpretation when the joined type wins.
fn convert_to_slot(v: CoreVal, slot: Option<CoreTy>) -> CoreVal {
    match slot {
        Some(CoreTy::I64) => match v {
            CoreVal::I64(_) => v,
            CoreVal::I32(x) => CoreVal::I64(x as u32 as i64),
            CoreVal::F32(x) => CoreVal::I64(x.to_bits() as i64),
            CoreVal::F64(x) => CoreVal::I64(x.to_bits() as i64),
        },
        Some(CoreTy::F64) => match v {
            CoreVal::F64(_) => v,
            CoreVal::I64(x) => CoreVal::F64(f64::from_bits(x as u64)),
            CoreVal::F32(x) => CoreVal::F64(x as f64),
            CoreVal::I32(x) => CoreVal::F64(x as f64),
        },
        _ => v,
    }
}

/// Inverse of [`convert_to_slot`]: reinterpret a JOINED slot value back into a
/// specific case's own flat type before lifting the payload.
fn convert_from_slot(v: CoreVal, slot: Option<CoreTy>, case_ty: Option<CoreTy>) -> CoreVal {
    match (slot, case_ty) {
        (Some(CoreTy::I64), Some(CoreTy::F64)) => match v {
            CoreVal::I64(x) => CoreVal::F64(f64::from_bits(x as u64)),
            other => other,
        },
        (Some(CoreTy::I64), Some(CoreTy::F32)) => match v {
            CoreVal::I64(x) => CoreVal::F32(f32::from_bits(x as u32)),
            other => other,
        },
        (Some(CoreTy::I64), Some(CoreTy::I32)) => match v {
            CoreVal::I64(x) => CoreVal::I32(x as u32 as i32),
            other => other,
        },
        (Some(CoreTy::F64), Some(CoreTy::I32)) => match v {
            CoreVal::F64(x) => CoreVal::I32(x as i32),
            other => other,
        },
        (Some(CoreTy::F64), Some(CoreTy::F32)) => match v {
            CoreVal::F64(x) => CoreVal::F32(x as f32),
            other => other,
        },
        _ => v,
    }
}

/// Typed zero for a joined slot (variants with payload-less cases must still
/// write a correctly-typed filler so core signatures type-check).
fn slot_zero(slot: Option<CoreTy>) -> CoreVal {
    match slot {
        Some(CoreTy::I64) => CoreVal::I64(0),
        Some(CoreTy::F64) => CoreVal::F64(0.0),
        Some(CoreTy::F32) => CoreVal::F32(0.0),
        _ => CoreVal::I32(0),
    }
}

fn field_val<'v>(fields: &'v [(String, Val)], name: &str) -> Result<&'v Val> {
    fields
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, v)| v)
        .ok_or_else(|| Error::value(format!("record missing field `{name}`")))
}

fn flags_to_flat(
    ty: &TypeRef,
    types: &TypeTable,
    set: &[String],
    sink: &mut FlatSink,
) -> Result<()> {
    let TypeDef::Flags(names) = &types[match ty {
        TypeRef::Def(i) => *i as usize,
        _ => return Err(Error::value("flags type expected")),
    }] else {
        return Err(Error::value("flags type expected"));
    };
    let n = names.len() as u32;
    let mut bits: Vec<bool> = vec![false; n as usize];
    for s in set {
        if let Some(idx) = names.iter().position(|name| name == s) {
            bits[idx] = true;
        }
    }
    match n {
        0 => {}
        1..=32 => {
            let mut b = 0u32;
            for (i, on) in bits.iter().enumerate() {
                if *on {
                    b |= 1 << i;
                }
            }
            sink.push_i32(b as i32);
        }
        33..=64 => {
            let mut b = 0u64;
            for (i, on) in bits.iter().enumerate() {
                if *on {
                    b |= 1u64 << i;
                }
            }
            sink.push_i64(b as i64);
        }
        _ => {
            let chunks = (n as usize).div_ceil(32);
            for c in 0..chunks {
                let mut b = 0u32;
                for i in 0..32u32 {
                    let f = c * 32 + i as usize;
                    if f < bits.len() && bits[f] {
                        b |= 1 << i;
                    }
                }
                sink.push_i32(b as i32);
            }
        }
    }
    Ok(())
}

/// Store a value into its **memory representation** at `addr`.
pub fn store(
    ty: &TypeRef,
    types: &TypeTable,
    val: &Val,
    addr: u64,
    mem: &mut dyn MemAccess,
    enc: StringEncoding,
) -> Result<()> {
    match ty {
        TypeRef::Primitive(p) => {
            let bytes: Vec<u8> = match (p, val) {
                (Primitive::Bool, Val::Bool(b)) => vec![*b as u8],
                (Primitive::S8, Val::S8(v)) => vec![*v as u8],
                (Primitive::U8, Val::U8(v)) => vec![*v],
                (Primitive::S16, Val::S16(v)) => v.to_le_bytes().to_vec(),
                (Primitive::U16, Val::U16(v)) => v.to_le_bytes().to_vec(),
                (Primitive::S32, Val::S32(v)) => v.to_le_bytes().to_vec(),
                (Primitive::U32, Val::U32(v)) => v.to_le_bytes().to_vec(),
                (Primitive::S64, Val::S64(v)) => v.to_le_bytes().to_vec(),
                (Primitive::U64, Val::U64(v)) => v.to_le_bytes().to_vec(),
                (Primitive::F32, Val::F32(v)) => v.to_bits().to_le_bytes().to_vec(),
                (Primitive::F64, Val::F64(v)) => v.to_bits().to_le_bytes().to_vec(),
                (Primitive::Char, Val::Char(c)) => (*c as u32).to_le_bytes().to_vec(),
                (Primitive::String, Val::String(s)) => {
                    let (ptr, units) = write_string(mem, s, enc)?;
                    let mut b = Vec::with_capacity(8);
                    b.extend_from_slice(&ptr.to_le_bytes());
                    b.extend_from_slice(&units.to_le_bytes());
                    mem.write(addr as usize, &b)?;
                    return Ok(());
                }
                (p, v) => {
                    return Err(Error::value(format!(
                        "cannot store {} as {p}",
                        v.type_name()
                    )))
                }
            };
            mem.write(addr as usize, &bytes)
        }
        TypeRef::Def(idx) => {
            let mut def = types[*idx as usize].clone();
            if def.is_resource_marker() {
                // a bare resource type in value position denotes an own handle
                def = TypeDef::Own(*idx);
            }
            match (&def, val) {
                (TypeDef::Primitive(p), _) => {
                    store(&TypeRef::Primitive(*p), types, val, addr, mem, enc)
                }
                (TypeDef::Record(fields), Val::Record(vals)) => {
                    let mut off = 0u64;
                    for (name, fty) in fields {
                        let a = alignment(fty, types);
                        off = align_to(off, a);
                        let v = field_val(vals, name)?;
                        store(fty, types, v, addr + off, mem, enc)?;
                        off += size(fty, types);
                    }
                    Ok(())
                }
                (TypeDef::Tuple(items), Val::Tuple(vals)) => {
                    let mut off = 0u64;
                    for (it, v) in items.iter().zip(vals.iter()) {
                        let a = alignment(it, types);
                        off = align_to(off, a);
                        store(it, types, v, addr + off, mem, enc)?;
                        off += size(it, types);
                    }
                    Ok(())
                }
                (TypeDef::List(et), Val::List(items)) => {
                    let elem_size = size(et, types);
                    let total = elem_size * items.len() as u64;
                    let ptr = if total == 0 {
                        0u32
                    } else {
                        let p = mem.realloc(0, 0, alignment(et, types), total)?;
                        for (i, item) in items.iter().enumerate() {
                            store(et, types, item, p as u64 + i as u64 * elem_size, mem, enc)?;
                        }
                        p
                    };
                    let mut b = Vec::with_capacity(8);
                    b.extend_from_slice(&ptr.to_le_bytes());
                    b.extend_from_slice(&(items.len() as u32).to_le_bytes());
                    mem.write(addr as usize, &b)
                }
                (TypeDef::Variant(cases), Val::Variant { case, payload }) => {
                    let disc = cases
                        .iter()
                        .position(|(n, _)| n == case)
                        .ok_or_else(|| Error::value(format!("unknown variant case `{case}`")))?;
                    mem.write(addr as usize, &(disc as u32).to_le_bytes())?;
                    if let (Some(pt), Some(p)) = (&cases[disc].1, payload.as_deref()) {
                        let a = alignment(pt, types).max(1);
                        store(pt, types, p, addr + align_to(4, a), mem, enc)?;
                    }
                    Ok(())
                }
                (TypeDef::Enum(names), Val::Enum(name)) => {
                    let disc = names
                        .iter()
                        .position(|n| n == name)
                        .ok_or_else(|| Error::value(format!("unknown enum case `{name}`")))?;
                    mem.write(addr as usize, &(disc as u32).to_le_bytes())
                }
                (TypeDef::Option(inner), Val::Option(opt)) => match opt {
                    Some(v) => {
                        mem.write(addr as usize, &1u32.to_le_bytes())?;
                        let a = alignment(inner, types).max(1);
                        store(inner, types, v, addr + align_to(4, a), mem, enc)
                    }
                    None => mem.write(addr as usize, &0u32.to_le_bytes()),
                },
                (TypeDef::Result { ok, err }, Val::Result(res)) => {
                    match res {
                        Ok(p) => {
                            mem.write(addr as usize, &0u32.to_le_bytes())?;
                            if let (Some(t), Some(p)) = (ok, p.as_deref()) {
                                let a = alignment(t, types).max(1);
                                store(t, types, p, addr + align_to(4, a), mem, enc)?;
                            }
                        }
                        Err(p) => {
                            mem.write(addr as usize, &1u32.to_le_bytes())?;
                            if let (Some(t), Some(p)) = (err, p.as_deref()) {
                                let a = alignment(t, types).max(1);
                                store(t, types, p, addr + align_to(4, a), mem, enc)?;
                            }
                        }
                    }
                    Ok(())
                }
                (TypeDef::Flags(_), Val::Flags(set)) => {
                    let TypeDef::Flags(names) = &types[match ty {
                        TypeRef::Def(i) => *i as usize,
                        _ => unreachable!(),
                    }] else {
                        unreachable!()
                    };
                    let n = names.len() as u32;
                    let mut bits: Vec<bool> = vec![false; n as usize];
                    for s in set {
                        if let Some(idx) = names.iter().position(|name| name == s) {
                            bits[idx] = true;
                        }
                    }
                    match n {
                        0 => Ok(()),
                        1..=32 => {
                            let mut b = 0u32;
                            for (i, on) in bits.iter().enumerate() {
                                if *on {
                                    b |= 1 << i;
                                }
                            }
                            mem.write(addr as usize, &b.to_le_bytes())
                        }
                        33..=64 => {
                            let mut b = 0u64;
                            for (i, on) in bits.iter().enumerate() {
                                if *on {
                                    b |= 1u64 << i;
                                }
                            }
                            mem.write(addr as usize, &b.to_le_bytes())
                        }
                        _ => {
                            let chunks = n.div_ceil(32);
                            for c in 0..chunks {
                                let mut b = 0u32;
                                for i in 0..32u32 {
                                    let f = c * 32 + i;
                                    if (f as usize) < bits.len() && bits[f as usize] {
                                        b |= 1 << i;
                                    }
                                }
                                mem.write(addr as usize + c as usize * 4, &b.to_le_bytes())?;
                            }
                            Ok(())
                        }
                    }
                }
                (TypeDef::Own(_), Val::Handle(Handle::Own { index, .. }))
                | (TypeDef::Borrow(_), Val::Handle(Handle::Borrow { index, .. })) => {
                    mem.write(addr as usize, &index.to_le_bytes())
                }
                (TypeDef::Stream(_), Val::Stream { index, .. })
                | (TypeDef::Future(_), Val::Future { index, .. }) => {
                    mem.write(addr as usize, &index.to_le_bytes())
                }
                (d, v) => Err(Error::value(format!(
                    "cannot store {} as {}",
                    v.type_name(),
                    d.describe(types)
                ))),
            }
        }
    }
}
