# Architecture Guide

This document provides a comprehensive overview of `wasm_component_layer`'s architecture, design decisions, and implementation details. It's intended for developers who want to understand how the library works internally, extend its functionality, or contribute to its development.

## Overview

`wasm_component_layer` implements the [WebAssembly Component Model](https://github.com/WebAssembly/component-model) specification atop any WebAssembly runtime backend. The library is structured as a layered architecture:

```
┌─────────────────────────────────────┐
│         WASM Runtime with Component |
|               Model Support         │
├─────────────────────────────────────┤
│       wasm_component_layer          │
│   ┌─────────────────────────────┐   │
│   │    Component Model Layer    │   │
│   │  (Types, Values, Functions) │   │
│   ├─────────────────────────────┤   │
│   │     Canonical ABI Layer     │   │
│   │   (Lifting/Lowering Logic)  │   │
│   ├─────────────────────────────┤   │
│   │   Runtime Abstraction Layer │   │
│   │    (wasm_runtime_layer)     │   │
│   └─────────────────────────────┘   │
├─────────────────────────────────────┤
│       WebAssembly Runtime           │
│   (wasmi, wasmtime, etc.)           │
└─────────────────────────────────────┘
```

## Core Components

### Component Model Layer

This layer implements the component model types and values:

#### Types System (`types.rs`)

The type system is the foundation of the component model. All types implement structural equality as mandated by the specification.

**Key Design Decisions:**
- All types are immutable and cheaply cloneable using `Arc`
- Complex types (records, variants, etc.) store their constituent types in sorted order for efficient comparison
- Resource types are special - they can be abstract (uninstantiated) or concrete (instantiated/host)

**Type Hierarchy:**
```
ValueType (enum)
├── Primitive types (Bool, S8, U8, ..., String, Char)
├── List(ListType)
├── Record(RecordType)
├── Tuple(TupleType)
├── Variant(VariantType)
├── Enum(EnumType)
├── Option(OptionType)
├── Result(ResultType)
├── Flags(FlagsType)
├── Own(ResourceType)
└── Borrow(ResourceType)
```

**Resource Type States:**
- `Abstract`: Guest resource type that hasn't been instantiated yet
- `Instantiated`: Guest resource type tied to a specific instance
- `Host`: Host-defined resource type with optional destructor

#### Values System (`values.rs`)

Values represent runtime data that can be passed between host and guest.

**Specialized Storage:**
For performance, lists of primitive types use specialized storage:

```rust
enum ListSpecialization {
    Bool(Arc<[bool]>),
    S8(Arc<[i8]>),
    U8(Arc<[u8]>),
    // ... other primitives
    Other(Vec<Value>),  // For complex types
}
```

This avoids boxing each primitive value individually.

**Resource Ownership:**
Resources use reference counting for ownership tracking:
- `ResourceOwn`: Owned resource with destructor
- `ResourceBorrow`: Borrowed resource with lifetime tracking

### Canonical ABI Layer (`abi.rs`, `func.rs`)

Implements the [Canonical ABI](https://github.com/WebAssembly/component-model/blob/main/design/mvp/CanonicalABI.md) for lifting/lowering values between host and guest representations.

#### Function Calling Architecture

Function calls go through several phases:

1. **Host Call Path:**
   ```
   User Code → TypedFunc → Func::call → FuncBindgen → ABI Instructions → WASM Call
   ```

2. **Guest Call Path:**
   ```
   WASM → Trampoline → Func::call_from_guest → FuncBindgen → ABI Instructions → Host Function
   ```

#### ABI Instruction Set

The `Instruction` enum represents the canonical ABI operations:

```rust
enum Instruction {
    // Argument/result handling
    GetArg { nth: usize },
    Return { amt: usize, func: Function },

    // Memory operations
    I32Load { offset: usize },
    I32Store { offset: usize },
    // ... many more

    // Type operations
    RecordLower { record: Record, ty: TypeId },
    RecordLift { record: Record, ty: TypeId },
    ListLower { element: Type, .. },
    // ... etc

    // Control flow
    CallWasm { name: Option<String>, sig: WasmSignature },
    CallInterface { func: Function },
}
```

#### Bindgen Pattern

`FuncBindgen` implements the `Bindgen` trait, which processes ABI instructions:

```rust
trait Bindgen {
    type Operand = Value;
    fn emit(&mut self, resolve: &Resolve, inst: &Instruction, operands: &mut Vec<Self::Operand>, results: &mut Vec<Self::Operand>) -> Result<()>;
}
```

This design allows the same ABI logic to work for both lifting (guest→host) and lowering (host→guest).

### Runtime Abstraction Layer

The library uses `wasm_runtime_layer` to abstract over different WebAssembly runtimes. This provides:

- `Engine`: Compilation environment
- `Store`: Runtime state management
- `Module`: Compiled WebAssembly module
- `Instance`: Instantiated module
- `Func`: Callable function
- `Memory`: Linear memory

## Component Loading and Instantiation

### Component Structure

A `Component` represents a parsed WebAssembly component:

```rust
struct ComponentInner {
    // Type information
    export_types: ComponentTypes,
    import_types: ComponentTypes,

    // Runtime modules and linking
    modules: FxHashMap<StaticModuleIndex, ModuleTranslation>,
    generated_trampolines: FxHashMap<TrampolineIndex, GeneratedTrampoline>,

    // Resource management
    resource_map: Vec<TypeResourceTableIndex>,

    // WIT resolution
    resolve: Resolve,
    world_id: Id<World>,
}
```

### Instantiation Process

1. **Parse Component:** Decode WIT and WASM bytes
2. **Translate Modules:** Convert component to core WASM modules
3. **Generate Types:** Create type information for exports/imports
4. **Extract Initializers:** Process component initialization logic
5. **Link and Instantiate:** Resolve imports and create instances

### Linking Architecture

The `Linker` manages import resolution:

```rust
struct Linker {
    root: LinkerInstance,
    instances: FxHashMap<InterfaceIdentifier, LinkerInstance>,
}

struct LinkerInstance {
    functions: FxHashMap<Arc<str>, Func>,
    resources: FxHashMap<Arc<str>, ResourceType>,
}
```

This allows hierarchical import organization matching the component model design.

## Resource Management

### Resource Tables

Resources are managed through tables that track ownership and borrowing:

```rust
struct HandleTable {
    array: Slab<HandleElement>,
    destructor: Option<Func>,
}

struct HandleElement {
    rep: i32,           // Guest representation
    own: bool,          // Owned or borrowed?
    lend_count: i32,    // Number of active borrows
}
```

### Ownership Semantics

- **Owned Resources:** Can be transferred between host/guest, have destructors
- **Borrowed Resources:** Temporary access, must be returned before owner is dropped
- **Reference Counting:** Prevents use-after-free and double-free

### Host vs Guest Resources

- **Host Resources:** Backed by Rust values, stored in `Slab<Box<dyn Any>>`
- **Guest Resources:** Opaque handles managed by guest code

## Type System Deep Dive

### Structural Equality

All component model types must support structural equality. This is implemented through:

1. **Sorted Storage:** Complex types store fields in sorted order
2. **Recursive Comparison:** Types compare their structure, not identity
3. **Arc-based Sharing:** Types can be shared efficiently

### Type Interning

Types are not interned globally - each component has its own type universe. This simplifies the implementation but means identical types from different components are not equal.

### Resource Type Instantiation

Abstract resource types become concrete during instantiation:

```rust
// Before instantiation
ResourceType::Abstract { id: 0, component: 123 }

// After instantiation
ResourceType::Instantiated { id: 0, instance: 456 }
```

## Memory Management

### Canonical ABI Memory Layout

The Canonical ABI defines how complex types are laid out in linear memory:

- **Alignment:** Types have alignment requirements
- **Size Calculation:** Static size computation for memory allocation
- **Little-Endian:** All data is stored in little-endian byte order

### Memory Operations

Memory access is abstracted through the `Bindgen` trait:

```rust
fn load<B: Blittable>(&self, offset: usize) -> Result<B>
fn store<B: Blittable>(&mut self, offset: usize, value: B) -> Result<()>
```

The `Blittable` trait handles primitive type serialization.

## Error Handling

The library uses `anyhow::Error` for flexible error handling. Specific error types include:

- `FuncError`: Function call failures with context
- Type mismatch errors
- Resource lifetime violations
- ABI validation failures

## Performance Characteristics

### Optimizations

1. **Specialized Lists:** Primitive lists avoid individual boxing
2. **Arc-based Cloning:** Cheap type/value duplication
3. **Direct Memory Access:** Blittable types bypass serialization
4. **Trampoline Reuse:** Generated trampolines are cached

### Trade-offs

1. **Memory Usage:** Arc overhead for all types
2. **Allocation Pressure:** Many small allocations for complex types
3. **Validation Cost:** Full type checking on every operation

## Extension Points

### Adding New Types

To add a new component model type:

1. Add variant to `ValueType` enum
2. Implement `Display` and `Debug`
3. Add to `ValueType::from_component` conversion
4. Implement ABI lifting/lowering instructions
5. Add to `ComponentList` trait if needed

### Runtime Backend Integration

To support a new WebAssembly runtime:

1. Implement `wasm_runtime_layer::backend::WasmEngine`
2. Ensure all required traits are implemented
3. Test with existing examples

### Feature Extensions

Common extension points:

- **String Transcoding:** Support for different string encodings
- **Subtyping:** Type coercion and subtyping rules
- **Async Functions:** Support for async component functions
- **Streaming:** Support for streams and futures

## Testing Strategy

### Unit Tests

- Type system correctness
- ABI instruction validation
- Resource lifetime management

### Integration Tests

- End-to-end component instantiation
- Cross-runtime compatibility
- Example validation

### Fuzzing

ABI instruction sequences are fuzzed to ensure robustness.

## Future Directions

### Planned Features

- String transcoding support
- Component model subtyping
- Async function support
- Enhanced debugging tools

### Architecture Improvements

- Type interning for memory efficiency
- JIT compilation of ABI sequences
- Parallel instantiation
- Advanced optimization passes

## Contributing Guidelines

### Code Organization

- `lib.rs`: Main API and documentation
- `types.rs`: Type system implementation
- `values.rs`: Value representations
- `func.rs`: Function calling and ABI
- `abi.rs`: Canonical ABI implementation
- `*.rs`: Supporting utilities

### Testing Requirements

- All new features must have comprehensive tests
- Performance regressions must be justified
- ABI compatibility must be maintained

### Documentation Standards

- All public APIs must have documentation
- Complex algorithms need explanatory comments
- Architecture decisions should be documented

This architecture provides a solid foundation for WebAssembly component model implementation while remaining extensible for future enhancements.

---

> **⚠️ Warning: Documentation**
>
> This documentation was completely refined by AI and may contain inaccuracies, errors, or incomplete information. Please use it as a reference but verify critical details against the actual codebase. If you find any useful corrections, improvements, or additional content that would benefit other users, please submit a pull request to help improve this documentation for the community.
