# wit-bindgen-wcl

**WIT Binding Generator for waclay** - Transform WIT interfaces into beautiful, type-safe Rust code.

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)

> **⚠️ Experimental Warning:** This tool is still experimental. Some features may work perfectly, others may not. It needs developer contributions to become stable.

## ✨ What makes this special?

`wit-bindgen-wcl` turns verbose WebAssembly Component Model code into clean, idiomatic Rust. **Only works with waclay** - purpose-built for the runtime-agnostic architecture.

**Before (manual):**
```rust
// 50+ lines of error-prone boilerplate...
let host_interface = linker.define_instance("example:demo/logger".try_into().unwrap()).unwrap();
host_interface.define_func("log", Func::new(&mut store, FuncType::new([ValueType::String], []), /* ... */))?;
```

**After (generated):**
```rust
impl LoggerHost for MyHost {
    fn log(&mut self, message: String) { println!("{}", message); }
}
imports::register_loggerHost(&mut linker, &mut store)?;
```

## 🚀 Quick Start

```bash
# Install
cargo install wit-bindgen-wcl

# Generate bindings
wit-bindgen-wcl my-interface.wit bindings.rs

# Use in code
mod bindings;
use bindings::*;
```

## 🎯 Features

- **🔒 Type Safety** - Full Rust type system for WIT types
- **🎭 Host Traits** - Clean trait-based host function implementations
- **⚡ Export Helpers** - Type-safe access to guest functions
- **🚀 Zero Cost** - Same performance as hand-written code
- **🔄 Hot Reload** - Regenerate bindings instantly

## 📚 Examples

Nine real-world examples in `examples/`:

- **🧮 Calculator** - Complex math with error handling
- **📁 File Manager** - Permissions and file operations
- **🕷️ Web Scraper** - Deeply nested data structures
- **🔧 Utilities** - Lists, records, variants, options, results

```bash
# Test all examples
./test-wit-bindgen.ps1
```

## 🛠️ WIT Support

- ✅ Records, Variants, Enums
- ✅ Options, Results, Lists
- ✅ Primitives & Nested Types
- 🚧 Resources, Flags (soon)

## ⚠️ Important

- **waclay exclusive** - Built specifically for waclay
- **Regenerate on changes** - Update bindings when WIT files change
- **Version aware** - Different versions may have incompatible bindings

## 🤝 Contributing

Help us add resources, flags, and more WIT features!

## 📄 License

Apache-2.0
