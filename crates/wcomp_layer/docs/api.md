#
 API Reference

This document provides a comprehensive overview of `wasm_component_layer`'s public API. For detailed documentation with examples, see the [docs.rs documentation](https://docs.rs/wasm_component_layer).

## Core Types

### Engine

The compilation environment for WebAssembly components.

```rust
pub struct Engine<E: backend::WasmEngine>(/* fields */);

impl<E: backend::WasmEngine> Engine<E> {
    pub fn new(engine: E) -> Self;
}
```

### Store

Manages WebAssembly runtime state and user data.

```rust
pub struct Store<T, E: backend::WasmEngine> {
    inner: wasm_runtime_layer::Store<StoreInner<T, E>, E>,
}

impl<T, E: backend::WasmEngine> Store<T, E> {
    pub fn new(engine: &Engine<E>, data: T) -> Self;
}
```

### Component

A parsed and validated WebAssembly component.

```rust
pub struct Component(/* fields */);

impl Component {
    pub fn new<E: backend::WasmEngine>(engine: &Engine<E>, bytes: &[u8]) -> Result<Self>;
    pub fn exports(&self) -> &ComponentTypes;
    pub fn imports(&self) -> &ComponentTypes;
    pub fn package(&self) -> &PackageIdentifier;
}
```

### Instance

An instantiated WebAssembly component.

```rust
pub struct Instance(/* fields */);

impl Instance {
    pub fn component(&self) -> &Component;
    pub fn exports(&self) -> &Exports;
    pub fn drop<E: backend::WasmEngine>(self, ctx: &mut Store<T, E>) -> Result<Vec<Error>>;
}
```

### Linker

Manages import resolution for component instantiation.

```rust
pub struct Linker(/* fields */);

impl Linker {
    pub fn new() -> Self;
    pub fn root(&self) -> &LinkerInstance;
    pub fn root_mut(&mut self) -> &mut LinkerInstance;
    pub fn define_instance(&mut self, name: InterfaceIdentifier) -> Result<&mut LinkerInstance>;
    pub fn instance(&self, name: &InterfaceIdentifier) -> Option<&LinkerInstance>;
    pub fn instance_mut(&mut self, name: &InterfaceIdentifier) -> Option<&mut LinkerInstance>;
    pub fn instantiate(&self, ctx: impl AsContextMut, component: &Component) -> Result<Instance>;
}
```

## Type System

### Value Types

```rust
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub enum ValueType {
    Bool, S8, U8, S16, U16, S32, U32, S64, U64, F32, F64, Char, String,
    List(ListType), Record(RecordType), Tuple(TupleType),
    Variant(VariantType), Enum(EnumType), Option(OptionType),
    Result(ResultType), Flags(FlagsType), Own(ResourceType), Borrow(ResourceType),
}

impl ValueType {
    pub fn ty(&self) -> ValueType;
}
```

### Complex Types

#### List Types

```rust
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ListType(/* fields */);

impl ListType {
    pub fn new(element_ty: ValueType) -> Self;
    pub fn element_ty(&self) -> ValueType;
}
```

#### Record Types

```rust
#[derive(Clone, Debug)]
pub struct RecordType {/* fields */};

impl RecordType {
    pub fn new(name: Option<TypeIdentifier>, fields: impl IntoIterator<Item = (S, ValueType)>) -> Result<Self>
    where S: Into<Arc<str>>;
    pub fn field_ty(&self, name: impl AsRef<str>) -> Option<ValueType>;
    pub fn fields(&self) -> impl ExactSizeIterator<Item = (&str, ValueType)>;
    pub fn name(&self) -> Option<&TypeIdentifier>;
}
```

#### Tuple Types

```rust
#[derive(Clone, Debug)]
pub struct TupleType {/* fields */};

impl TupleType {
    pub fn new(name: Option<TypeIdentifier>, fields: impl IntoIterator<Item = ValueType>) -> Self;
    pub fn fields(&self) -> &[ValueType];
    pub fn name(&self) -> Option<&TypeIdentifier>;
}
```

#### Variant Types

```rust
#[derive(Clone, Debug)]
pub struct VariantType {/* fields */};

impl VariantType {
    pub fn new(name: Option<TypeIdentifier>, cases: impl IntoIterator<Item = VariantCase>) -> Result<Self>;
    pub fn cases(&self) -> &[VariantCase];
    pub fn name(&self) -> Option<&TypeIdentifier>;
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct VariantCase {/* fields */};

impl VariantCase {
    pub fn new(name: impl Into<Arc<str>>, ty: Option<ValueType>) -> Self;
    pub fn name(&self) -> &str;
    pub fn ty(&self) -> Option<ValueType>;
}
```

#### Enum Types

```rust
#[derive(Clone, Debug)]
pub struct EnumType {/* fields */};

impl EnumType {
    pub fn new(name: Option<TypeIdentifier>, cases: impl IntoIterator<Item = S>) -> Result<Self>
    where S: Into<Arc<str>>;
    pub fn name(&self) -> Option<&TypeIdentifier>;
    pub fn cases(&self) -> impl ExactSizeIterator<Item = &str>;
}
```

#### Option Types

```rust
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct OptionType {/* fields */};

impl OptionType {
    pub fn new(ty: ValueType) -> Self;
    pub fn some_ty(&self) -> ValueType;
}
```

#### Result Types

```rust
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct ResultType {/* fields */};

impl ResultType {
    pub fn new(ok: Option<ValueType>, err: Option<ValueType>) -> Self;
    pub fn ok_ty(&self) -> Option<ValueType>;
    pub fn err_ty(&self) -> Option<ValueType>;
}
```

#### Flags Types

```rust
#[derive(Clone, Debug)]
pub struct FlagsType {/* fields */};

impl FlagsType {
    pub fn new(name: Option<TypeIdentifier>, names: impl IntoIterator<Item = S>) -> Result<Self>
    where S: Into<Arc<str>>;
    pub fn name(&self) -> Option<&TypeIdentifier>;
    pub fn names(&self) -> impl ExactSizeIterator<Item = &str>;
}
```

### Resource Types

```rust
#[derive(Clone, Debug)]
pub struct ResourceType {/* fields */};

impl ResourceType {
    pub fn new<T: 'static + Send + Sync + Sized>(name: Option<TypeIdentifier>) -> Self;
    pub fn with_destructor<T, C, F>(ctx: C, name: Option<TypeIdentifier>, destructor: F) -> Result<Self>
    where
        T: 'static + Send + Sync + Sized,
        C: AsContextMut,
        F: 'static + Send + Sync + Fn(StoreContextMut<C::UserState, C::Engine>, T) -> Result<()>;
    pub fn name(&self) -> Option<&TypeIdentifier>;
}
```

### Function Types

```rust
#[derive(Clone, PartialEq, Eq)]
pub struct FuncType {/* fields */};

impl FuncType {
    pub fn new<P, R>(params: P, results: R) -> Self
    where
        P: IntoIterator<Item = ValueType>,
        R: IntoIterator<Item = ValueType>;
    pub fn params(&self) -> &[ValueType];
    pub fn results(&self) -> &[ValueType];
}
```

## Values

### Value Enum

```rust
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool), S8(i8), U8(u8), S16(i16), U16(u16), S32(i32), U32(u32),
    S64(i64), U64(u64), F32(f32), F64(f64), Char(char), String(Arc<str>),
    List(List), Record(Record), Tuple(Tuple), Variant(Variant), Enum(Enum),
    Option(OptionValue), Result(ResultValue), Flags(Flags), Own(ResourceOwn), Borrow(ResourceBorrow),
}

impl Value {
    pub fn ty(&self) -> ValueType;
}
```

### Complex Values

#### Lists

```rust
#[derive(Clone, Debug)]
pub struct List {/* fields */};

impl List {
    pub fn new(ty: ListType, values: impl IntoIterator<Item = Value>) -> Result<Self>;
    pub fn ty(&self) -> ListType;
    pub fn typed<T: ListPrimitive>(&self) -> Result<&[T]>;
    pub fn iter(&self) -> impl Iterator<Item = Value>;
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
}

impl<T: ListPrimitive> From<&[T]> for List {
    fn from(value: &[T]) -> Self;
}
```

#### Records

```rust
#[derive(Clone, Debug)]
pub struct Record {/* fields */};

impl Record {
    pub fn new<S: Into<Arc<str>>>(ty: RecordType, values: impl IntoIterator<Item = (S, Value)>) -> Result<Self>;
    pub fn from_fields<S: Into<Arc<str>>>(name: Option<TypeIdentifier>, values: impl IntoIterator<Item = (S, Value)>) -> Result<Self>;
    pub fn field(&self, field: impl AsRef<str>) -> Option<Value>;
    pub fn fields(&self) -> impl ExactSizeIterator<Item = (&str, Value)>;
    pub fn ty(&self) -> RecordType;
}
```

#### Tuples

```rust
#[derive(Clone, Debug)]
pub struct Tuple {/* fields */};

impl Tuple {
    pub fn new(ty: TupleType, fields: impl IntoIterator<Item = Value>) -> Result<Self>;
    pub fn from_fields(name: Option<TypeIdentifier>, fields: impl IntoIterator<Item = Value>) -> Self;
    pub fn ty(&self) -> TupleType;
}

impl Deref for Tuple {
    type Target = [Value];
    fn deref(&self) -> &Self::Target;
}
```

#### Variants

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct Variant {/* fields */};

impl Variant {
    pub fn new(ty: VariantType, discriminant: usize, value: Option<Value>) -> Result<Self>;
    pub fn discriminant(&self) -> usize;
    pub fn value(&self) -> Option<Value>;
    pub fn ty(&self) -> VariantType;
}
```

#### Enums

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct Enum {/* fields */};

impl Enum {
    pub fn new(ty: EnumType, discriminant: usize) -> Result<Self>;
    pub fn discriminant(&self) -> usize;
    pub fn ty(&self) -> EnumType;
}
```

#### Options

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct OptionValue {/* fields */};

impl OptionValue {
    pub fn new(ty: OptionType, value: Option<Value>) -> Result<Self>;
    pub fn ty(&self) -> OptionType;
}

impl Deref for OptionValue {
    type Target = Option<Value>;
    fn deref(&self) -> &Self::Target;
}
```

#### Results

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct ResultValue {/* fields */};

impl ResultValue {
    pub fn new(ty: ResultType, value: Result<Option<Value>, Option<Value>>) -> Result<Self>;
    pub fn ty(&self) -> ResultType;
}

impl Deref for ResultValue {
    type Target = Result<Option<Value>, Option<Value>>;
    fn deref(&self) -> &Self::Target;
}
```

#### Flags

```rust
#[derive(Clone, Debug, PartialEq)]
pub struct Flags {/* fields */};

impl Flags {
    pub fn new(ty: FlagsType) -> Self;
    pub fn get(&self, name: impl AsRef<str>) -> bool;
    pub fn set(&mut self, name: impl AsRef<str>, value: bool);
    pub fn ty(&self) -> FlagsType;
}
```

### Resources

#### Owned Resources

```rust
#[derive(Clone, Debug)]
pub struct ResourceOwn {/* fields */};

impl ResourceOwn {
    pub fn new<T, C>(ctx: C, value: T, ty: ResourceType) -> Result<Self>
    where
        T: 'static + Send + Sync + Sized,
        C: AsContextMut;
    pub fn borrow(&self, ctx: impl AsContextMut) -> Result<ResourceBorrow>;
    pub fn rep<T, S, E>(&self, ctx: &StoreContext<S, E>) -> Result<&T>
    where
        T: 'static + Send + Sync;
    pub fn rep_mut<T, S, E>(&self, ctx: &mut StoreContextMut<S, E>) -> Result<&mut T>
    where
        T: 'static + Send + Sync;
    pub fn ty(&self) -> ResourceType;
    pub fn drop(self, ctx: impl AsContextMut) -> Result<()>;
}
```

#### Borrowed Resources

```rust
#[derive(Clone, Debug)]
pub struct ResourceBorrow {/* fields */};

impl ResourceBorrow {
    pub fn drop(self, ctx: impl AsContextMut) -> Result<()>;
}
```

## Functions

### Functions

```rust
#[derive(Clone, Debug)]
pub struct Func {/* fields */};

impl Func {
    pub fn new<C: AsContextMut>(
        ctx: C,
        ty: FuncType,
        f: impl 'static + Send + Sync + Fn(StoreContextMut<C::UserState, C::Engine>, &[Value], &mut [Value]) -> Result<()>,
    ) -> Self;
    pub fn call<C: AsContextMut>(
        &self,
        ctx: C,
        arguments: &[Value],
        results: &mut [Value],
    ) -> Result<()>;
    pub fn typed<P: ComponentList, R: ComponentList>(&self) -> Result<TypedFunc<P, R>>;
    pub fn ty(&self) -> FuncType;
}
```

### Typed Functions

```rust
#[derive(Clone, Debug)]
pub struct TypedFunc<P: ComponentList, R: ComponentList> {/* fields */};

impl<P: ComponentList, R: ComponentList> TypedFunc<P, R> {
    pub fn new<C: AsContextMut>(
        ctx: C,
        f: impl 'static + Send + Sync + Fn(StoreContextMut<C::UserState, C::Engine>, P) -> Result<R>,
    ) -> Self;
    pub fn call(&self, ctx: impl AsContextMut, params: P) -> Result<R>;
    pub fn func(&self) -> Func;
    pub fn ty(&self) -> FuncType;
}
```

## Component Types and Exports

### Component Types

```rust
#[derive(Debug)]
pub struct ComponentTypes {/* fields */};

impl ComponentTypes {
    pub fn root(&self) -> &ComponentTypesInstance;
    pub fn instance(&self, name: &InterfaceIdentifier) -> Option<&ComponentTypesInstance>;
    pub fn instances(&self) -> impl Iterator<Item = (&InterfaceIdentifier, &ComponentTypesInstance)>;
}
```

### Component Types Instance

```rust
#[derive(Debug)]
pub struct ComponentTypesInstance {/* fields */};

impl ComponentTypesInstance {
    pub fn func(&self, name: impl AsRef<str>) -> Option<FuncType>;
    pub fn funcs(&self) -> impl Iterator<Item = (&str, FuncType)>;
    pub fn resource(&self, name: impl AsRef<str>) -> Option<ResourceType>;
    pub fn resources(&self) -> impl Iterator<Item = (&str, ResourceType)>;
}
```

### Exports

```rust
#[derive(Debug)]
pub struct Exports {/* fields */};

impl Exports {
    pub fn root(&self) -> &ExportInstance;
    pub fn instance(&self, name: &InterfaceIdentifier) -> Option<&ExportInstance>;
    pub fn instances(&self) -> impl Iterator<Item = (&InterfaceIdentifier, &ExportInstance)>;
}
```

### Export Instance

```rust
#[derive(Debug)]
pub struct ExportInstance {/* fields */};

impl ExportInstance {
    pub fn func(&self, name: impl AsRef<str>) -> Option<Func>;
    pub fn funcs(&self) -> impl Iterator<Item = (&str, Func)>;
    pub fn resource(&self, name: impl AsRef<str>) -> Option<ResourceType>;
    pub fn resources(&self) -> impl Iterator<Item = (&str, ResourceType)>;
}
```

## Linker Types

### Linker Instance

```rust
#[derive(Clone, Debug, Default)]
pub struct LinkerInstance {/* fields */};

impl LinkerInstance {
    pub fn define_func(&mut self, name: impl Into<Arc<str>>, func: Func) -> Result<()>;
    pub fn func(&self, name: impl AsRef<str>) -> Option<Func>;
    pub fn define_resource(&mut self, name: impl Into<Arc<str>>, resource_ty: ResourceType) -> Result<()>;
    pub fn resource(&self, name: impl AsRef<str>) -> Option<ResourceType>;
    pub fn funcs(&self) -> impl Iterator<Item = (&str, Func)>;
    pub fn resources(&self) -> impl Iterator<Item = (&str, ResourceType)>;
}
```

## Identifiers

### Package Identifier

```rust
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct PackageIdentifier {/* fields */};

impl PackageIdentifier {
    pub fn new(namespace: impl Into<Arc<str>>, name: impl Into<Arc<str>>, version: Option<semver::Version>) -> Self;
    pub fn namespace(&self) -> &str;
    pub fn name(&self) -> &str;
    pub fn version(&self) -> Option<&semver::Version>;
}
```

### Interface Identifier

```rust
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct InterfaceIdentifier {/* fields */};

impl InterfaceIdentifier {
    pub fn new(package: PackageIdentifier, name: impl Into<Arc<str>>) -> Self;
    pub fn package(&self) -> &PackageIdentifier;
    pub fn name(&self) -> &str;
}
```

### Type Identifier

```rust
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct TypeIdentifier {/* fields */};

impl TypeIdentifier {
    pub fn new(name: impl Into<Arc<str>>, interface: Option<InterfaceIdentifier>) -> Self;
    pub fn name(&self) -> &str;
    pub fn interface(&self) -> Option<&InterfaceIdentifier>;
}
```

## Traits

### ComponentList

```rust
pub trait ComponentList: 'static + Sized {
    const LEN: usize;
    fn into_tys(types: &mut [ValueType]);
    fn into_values(self, values: &mut [Value]) -> Result<()>;
    fn from_values(values: &[Value]) -> Result<Self>;
}
```

### ListPrimitive

```rust
pub trait ListPrimitive: Copy + 'static {
    fn ty() -> ValueType;
    fn from_value_iter(values: impl IntoIterator<Item = Value>) -> Result<Arc<[Self]>>;
}
```

## Context Traits

### AsContext

```rust
pub trait AsContext {
    type UserState: 'static;
    type Engine: backend::WasmEngine;
    fn as_context(&self) -> StoreContext<Self::UserState, Self::Engine>;
}
```

### AsContextMut

```rust
pub trait AsContextMut: AsContext {
    fn as_context_mut(&mut self) -> StoreContextMut<Self::UserState, Self::Engine>;
}
```

## Error Types

### FuncError

```rust
pub struct FuncError {/* fields */};

impl FuncError {
    pub fn name(&self) -> &str;
    pub fn instance(&self) -> &Instance;
}
```

## Feature Flags

- `serde`: Enables serialization/deserialization support for types and values

## Backend Requirements

The library requires a WebAssembly runtime that implements the `wasm_runtime_layer::backend::WasmEngine` trait, providing:

- Module compilation and instantiation
- Function calling interface
- Memory management
- Table operations

Supported backends include:
- `wasmi_runtime_layer`
- `wasmtime_runtime_layer`
- `js_wasm_runtime_layer`

---

> **⚠️ Warning: Documentation**
>
> This documentation was completely refined by AI and may contain inaccuracies, errors, or incomplete information. Please use it as a reference but verify critical details against the actual codebase. If you find any useful corrections, improvements, or additional content that would benefit other users, please submit a pull request to help improve this documentation for the community.
