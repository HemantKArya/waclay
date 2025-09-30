# Dependency Update Migration Guide

## Overview
This PR updates the core dependencies to their latest versions:
- `wasmtime-environ`: 18.0.4 → 37.0.1
- `wit-component`: 0.19.1 → 0.239.0
- `wit-parser`: 0.13.2 → 0.239.0

These are major version jumps with significant API changes.

## Completed Changes

### 1. Cargo.toml Updates
- Updated all three dependencies to latest versions
- Added `"compile"` feature to wasmtime-environ (required for ModuleTranslation)

### 2. wit-parser API Changes
- **Function.results → Function.result**: Changed from `Results` enum to `Option<Type>`
  - Updated all `.results.len()` to `.result.iter().count()`
  - Updated all `.results.iter_types()` to `.result.iter()`
- **Type variants renamed**: `Float32/Float64` → `F32/F64`
- **WorldItem::Interface**: Changed from tuple variant to struct variant
  - `WorldItem::Interface(id)` → `WorldItem::Interface { id, .. }`

### 3. SizeAlign API Changes
All size/alignment methods now return typed wrappers instead of raw integers:
- `.size()` returns `ArchitectureSize` (use `.size_wasm32()` for usize)
- `.align()` returns `Alignment` (use `.align_wasm32()` for usize)
- `.record()` returns `ElementInfo` with `.size` and `.align` fields
- `.field_offsets()` returns `Vec<(ArchitectureSize, &Type)>`
- `.payload_offset()` returns `ArchitectureSize`

### 4. wasmtime-environ Changes
- **ComponentTypesBuilder**: No longer has `::default()`, must use `::new(&validator)`
- **Imports**: ModuleTranslation, ComponentTypesBuilder, and Translator now require explicit imports

## Remaining Work (37 compilation errors)

### 1. New wit-parser Type Variants
The component model has evolved with new type variants that need handling:

**Type::ErrorContext** (4 locations in src/abi.rs, src/types.rs, src/values.rs)
- Purpose: Represents error context information for better error handling
- Action needed: Add match arms for this variant

**TypeDefKind::FixedSizeList** (4 locations in src/types.rs, src/values.rs)
- Purpose: Fixed-size arrays (new feature in component model)
- Action needed: Add handling similar to List but with fixed size

### 2. New AbiVariant Async Variants
**AbiVariant async variants** (2 locations in src/abi.rs)
- GuestImportAsync
- GuestExportAsync  
- GuestExportAsyncStackful
- Purpose: Support for async functions in component model
- Action needed: Add match arms (can initially return errors for unsupported)

### 3. New WasmType Variants
**WasmType new variants** (2 locations in src/abi.rs)
- Pointer
- PointerOrI64
- Length
- Purpose: Enhanced canonical ABI types
- Action needed: Add join() logic for these variants

### 4. wasmtime-environ Structure Changes

**Component structure**:
- `num_resource_tables` field removed or renamed
- Need to find new way to access this information

**CanonicalOptions structure**:
- `.memory` and `.realloc` fields changed
- Now uses different accessors or structure
- Located in src/lib.rs around lines dealing with lowering options

**Export variants**:
- `Export::ModuleStatic` changed from tuple to struct variant
- Need to destructure differently

**TypeFuncIndex**:
- No longer implements Deref
- Need to access underlying value differently

**NameMap iteration**:
- No longer directly iterable with `for (k, v) in map`
- Need to use `.iter()` or similar method

### 5. Type System Issues

**semver::Error** (2 locations in src/identifier.rs)
- No longer implements std::error::Error in the same way
- Need to use `.map_err()` to convert to anyhow::Error

## Migration Strategy

### Phase 1: Pattern Matching (Immediate)
Add all missing match arms for new enum variants. Most can initially return errors or unimplemented!() for unsupported features.

### Phase 2: Structure Access (Requires API Investigation)
For wasmtime-environ changes, need to:
1. Find documentation or examples in wasmtime repo
2. Update field access patterns
3. Update method calls

### Phase 3: Testing
Once compilation succeeds:
1. Run existing test suite
2. Run all examples
3. Verify component model features still work

### Phase 4: Feature Support (Future)
Implement actual support for new features:
- Fixed-size lists
- Error contexts
- Async functions
- New canonical ABI types

## Files Modified

### Core Changes
- `Cargo.toml`: Dependency updates
- `src/lib.rs`: Import updates, WorldItem, ComponentTypesBuilder, Component/CanonicalOptions access
- `src/abi.rs`: Type patterns, Results→result, SizeAlign conversions, AbiVariant
- `src/types.rs`: Type patterns, FuncType construction
- `src/func.rs`: Alignment conversions, Type::Float patterns
- `src/values.rs`: Type patterns

### Unchanged
- `src/identifier.rs`: Just semver error conversion needed
- Test files should work once compilation succeeds

## Testing Checklist

Once compilation errors are fixed:
- [ ] `cargo test` passes
- [ ] `cargo run --example single_component` works
- [ ] `cargo run --example complex_return` works
- [ ] `cargo run --example func_param` works
- [ ] `cargo run --example guest_resource` works
- [ ] `cargo run --example multilevel_resource` works
- [ ] `cargo run --example option_result` works
- [ ] `cargo run --example record_response` works
- [ ] `cargo run --example resource` works
- [ ] `cargo run --example string_host_guest` works
- [ ] `cargo run --example variant_return` works

## References

- [Wasmtime Component Model Docs](https://docs.wasmtime.dev/api/wasmtime/component/)
- [Component Model Spec](https://github.com/WebAssembly/component-model)
- [wit-parser Changes](https://github.com/bytecodealliance/wasm-tools/tree/main/crates/wit-parser)
- [Canonical ABI](https://github.com/WebAssembly/component-model/blob/main/design/mvp/CanonicalABI.md)
