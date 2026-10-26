//! Canonical ABI string-encoding round-trips driven directly against the
//! `load`/`store` memory layer with an in-memory mock — no engine needed.
//!
//! Covers all three `canon lift`/`canon lower` string encodings (UTF-8,
//! UTF-16, compact latin-1/UTF-16), including multi-byte content and nested
//! (record) positions.

use waclay::abi::layout::{align_to, alignment, size};
use waclay::abi::{load, store, MemAccess, StringEncoding};
use waclay::component::types::{Primitive, TypeDef, TypeRef, Val};
use waclay::Result;

// ---------------------------------------------------------------------------
// Mock linear memory with a bump allocator
// ---------------------------------------------------------------------------

struct MockMem {
    bytes: Vec<u8>,
    heap: usize,
}

impl MockMem {
    fn new() -> MockMem {
        MockMem {
            bytes: vec![0u8; 1 << 16],
            heap: 16,
        }
    }
}

impl MemAccess for MockMem {
    fn read(&mut self, offset: usize, buf: &mut [u8]) -> Result<()> {
        let end = offset + buf.len();
        if end > self.bytes.len() {
            return Err(waclay::Error::trap(format!(
                "mock mem oob read: offset={offset} len={} size={}",
                buf.len(),
                self.bytes.len()
            )));
        }
        buf.copy_from_slice(&self.bytes[offset..end]);
        Ok(())
    }

    fn write(&mut self, offset: usize, buf: &[u8]) -> Result<()> {
        let end = offset + buf.len();
        if end > self.bytes.len() {
            return Err(waclay::Error::trap(format!(
                "mock mem oob write: offset={offset} len={} size={}",
                buf.len(),
                self.bytes.len()
            )));
        }
        self.bytes[offset..end].copy_from_slice(buf);
        Ok(())
    }

    fn len(&self) -> usize {
        self.bytes.len()
    }

    fn realloc(&mut self, old: u32, old_size: u64, align: u64, new_size: u64) -> Result<u32> {
        let aligned = align_to(self.heap as u64, align.max(1)) as usize;
        self.heap = aligned + new_size as usize;
        let ret = aligned as u32;
        if old != 0 && old_size > 0 {
            let src = old as usize;
            let n = old_size as usize;
            let chunk: Vec<u8> = self.bytes[src..src + n].to_vec();
            self.bytes[ret as usize..ret as usize + n].copy_from_slice(&chunk);
        }
        Ok(ret)
    }
}

const STRING: TypeRef = TypeRef::Primitive(Primitive::String);

fn roundtrip(s: &str, enc: StringEncoding) -> String {
    let mut mem = MockMem::new();
    let val = Val::String(s.to_string());
    // encode through the same machinery used by list elements: store a
    // one-element list<string> then load it back.
    let types = [TypeDef::List(STRING)];
    let addr = mem
        .realloc(0, 0, 4, size(&TypeRef::Def(0), &types))
        .unwrap();
    store(
        &TypeRef::Def(0),
        &types,
        &Val::List(vec![val]),
        addr as u64,
        &mut mem,
        enc,
    )
    .unwrap();
    let v = load(&TypeRef::Def(0), &types, addr as u64, &mut mem, enc).unwrap();
    match v {
        Val::List(items) => match items.into_iter().next() {
            Some(Val::String(s)) => s,
            other => panic!("expected string, got {other:?}"),
        },
        other => panic!("expected list, got {other:?}"),
    }
}

#[test]
fn utf8_roundtrip() {
    for s in ["", "hello", "héllo wörld", "emoji: 🦀🚀", "日本語テキスト"] {
        assert_eq!(
            roundtrip(s, StringEncoding::Utf8),
            s,
            "utf8 roundtrip of {s:?}"
        );
    }
}

#[test]
fn utf16_roundtrip() {
    for s in ["", "hello", "héllo wörld", "emoji: 🦀🚀", "日本語テキスト"] {
        assert_eq!(
            roundtrip(s, StringEncoding::Utf16),
            s,
            "utf16 roundtrip of {s:?}"
        );
    }
}

#[test]
fn compact_utf16_latin1_roundtrip() {
    // all chars < 256 -> stored as latin-1, one byte per char
    for s in ["", "hello", "héllo wörld ünïcode"] {
        assert_eq!(
            roundtrip(s, StringEncoding::CompactUtf16),
            s,
            "compact latin1 roundtrip of {s:?}"
        );
    }
}

#[test]
fn compact_utf16_fallback_roundtrip() {
    // chars >= 256 -> stored as utf-16 units
    for s in ["🦀", "mixed a🦀b", "日本語"] {
        assert_eq!(
            roundtrip(s, StringEncoding::CompactUtf16),
            s,
            "compact utf16 roundtrip of {s:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Nested position: record with string fields per encoding
// ---------------------------------------------------------------------------

#[test]
fn string_inside_record_roundtrip() {
    let types: Vec<TypeDef> = vec![TypeDef::Record(vec![
        ("a".into(), TypeRef::Primitive(Primitive::String)),
        ("b".into(), TypeRef::Primitive(Primitive::U32)),
        ("c".into(), TypeRef::Primitive(Primitive::String)),
    ])];
    let ty = TypeRef::Def(0);
    let expect = Val::Record(vec![
        ("a".into(), Val::String("first".into())),
        ("b".into(), Val::U32(7)),
        ("c".into(), Val::String("second 🦀".into())),
    ]);
    for enc in [
        StringEncoding::Utf8,
        StringEncoding::Utf16,
        StringEncoding::CompactUtf16,
    ] {
        let mut mem = MockMem::new();
        let addr = mem
            .realloc(0, 0, alignment(&ty, &types), size(&ty, &types))
            .unwrap();
        store(&ty, &types, &expect, addr as u64, &mut mem, enc).unwrap();
        let got = load(&ty, &types, addr as u64, &mut mem, enc).unwrap();
        assert_eq!(got, expect, "record roundtrip under {enc:?}");
    }
}

// ---------------------------------------------------------------------------
// Byte-level encoding checks (the exact wire format)
// ---------------------------------------------------------------------------

#[test]
fn utf8_uses_byte_length() {
    let mut mem = MockMem::new();
    let types = [TypeDef::List(STRING)];
    let val = Val::List(vec![Val::String("hé".into())]); // 3 bytes utf8
    let addr = mem.realloc(0, 0, 4, 8).unwrap();
    store(
        &TypeRef::Def(0),
        &types,
        &val,
        addr as u64,
        &mut mem,
        StringEncoding::Utf8,
    )
    .unwrap();
    // list header at addr: (element_ptr, element_count)
    let mut hdr = [0u8; 8];
    mem.read(addr as usize, &mut hdr).unwrap();
    let elem_ptr = u32::from_le_bytes(hdr[..4].try_into().unwrap());
    let count = u32::from_le_bytes(hdr[4..8].try_into().unwrap());
    assert_eq!(count, 1, "list length is the element count");
    // string element: (data_ptr, byte_length)
    let mut pair = [0u8; 8];
    mem.read(elem_ptr as usize, &mut pair).unwrap();
    let str_len = u32::from_le_bytes(pair[4..8].try_into().unwrap());
    assert_eq!(str_len, 3, "utf8 length must be byte length");
}

#[test]
fn utf16_uses_unit_length() {
    let mut mem = MockMem::new();
    let types = [TypeDef::List(STRING)];
    let val = Val::List(vec![Val::String("hé".into())]); // 2 utf16 units
    let addr = mem.realloc(0, 0, 4, 8).unwrap();
    store(
        &TypeRef::Def(0),
        &types,
        &val,
        addr as u64,
        &mut mem,
        StringEncoding::Utf16,
    )
    .unwrap();
    let mut hdr = [0u8; 8];
    mem.read(addr as usize, &mut hdr).unwrap();
    let elem_ptr = u32::from_le_bytes(hdr[..4].try_into().unwrap());
    let mut pair = [0u8; 8];
    mem.read(elem_ptr as usize, &mut pair).unwrap();
    let str_len = u32::from_le_bytes(pair[4..8].try_into().unwrap());
    assert_eq!(str_len, 2, "utf16 length must be code-unit count");
    // and the bytes really are utf-16le
    let mut buf = vec![0u8; str_len as usize * 2];
    let data_ptr = u32::from_le_bytes(pair[..4].try_into().unwrap());
    mem.read(data_ptr as usize, &mut buf).unwrap();
    assert_eq!(&buf[..2], &0x68u16.to_le_bytes()); // 'h'
    assert_eq!(&buf[2..4], &0xE9u16.to_le_bytes()); // 'é'
}

#[test]
fn compact_utf16_tags_utf16_storage() {
    let mut mem = MockMem::new();
    let types = [TypeDef::List(STRING)];
    // latin-1 storable: plain length
    let latin = Val::List(vec![Val::String("abc".into())]);
    let addr = mem.realloc(0, 0, 4, 8).unwrap();
    store(
        &TypeRef::Def(0),
        &types,
        &latin,
        addr as u64,
        &mut mem,
        StringEncoding::CompactUtf16,
    )
    .unwrap();
    let mut hdr = [0u8; 8];
    mem.read(addr as usize, &mut hdr).unwrap();
    let elem_ptr = u32::from_le_bytes(hdr[..4].try_into().unwrap());
    let mut pair = [0u8; 8];
    mem.read(elem_ptr as usize, &mut pair).unwrap();
    let str_len = u32::from_le_bytes(pair[4..8].try_into().unwrap());
    assert_eq!(str_len, 3, "latin-1 storage must carry an untagged length");

    // non-latin-1: tagged length (top bit set) + utf-16le bytes
    let uni = Val::List(vec![Val::String("🦀".into())]); // 2 utf16 units
    let addr2 = mem.realloc(0, 0, 4, 8).unwrap();
    store(
        &TypeRef::Def(0),
        &types,
        &uni,
        addr2 as u64,
        &mut mem,
        StringEncoding::CompactUtf16,
    )
    .unwrap();
    mem.read(addr2 as usize, &mut hdr).unwrap();
    let elem_ptr2 = u32::from_le_bytes(hdr[..4].try_into().unwrap());
    mem.read(elem_ptr2 as usize, &mut pair).unwrap();
    let tagged = u32::from_le_bytes(pair[4..8].try_into().unwrap());
    assert_eq!(
        tagged & (1 << 31),
        1 << 31,
        "utf-16 storage must set the tag bit"
    );
    assert_eq!(tagged & !(1 << 31), 2, "tagged units = code-unit count");
}
