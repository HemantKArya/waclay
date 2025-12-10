<div align="center">

# 🚀 waclay

### WebAssembly Component Layer - Runtime Agnostic

[![License](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.70%2B-orange.svg)](https://www.rust-lang.org/)
[![Unsafe Forbidden](https://img.shields.io/badge/unsafe-forbidden-success.svg)](https://github.com/rust-secure-code/safety-dance/)
[![PRs Welcome](https://img.shields.io/badge/PRs-welcome-brightgreen.svg)](CONTRIBUTING.md)

*A maintained fork bringing the WebAssembly Component Model to life*

[Features](#-features) •
[Quick Start](#-quick-start) •
[Examples](#-examples) •
[Contributing](#-contributing) •
[Credits](#-credits)

</div>

> **⚠️ Experimental Warning:** This project is still experimental. Some features may work perfectly, others may not. It needs developer contributions to become stable.

---

## 📖 About This Project

**waclay** (WebAssembly Component Layer) is a **runtime-agnostic** implementation of the [WebAssembly Component Model](https://github.com/WebAssembly/component-model). This project enables you to load, link, and execute WASM components with full component model support across **any** WebAssembly runtime backend.

### 🎯 Why This Fork?

This is a **maintained fork** of the original `wasm_component_layer` project. After the original developer discontinued active maintenance, the project became difficult to compile and use with modern Rust toolchains. This fork aims to:

- ✅ **Keep it compiling** - Updated to work with latest Rust and dependencies (50+ commits of fixes and improvements)
- ✅ **Add new features** - Including the new `wit-bindgen-wcl` tool for generating host bindings
- ✅ **Maintain usability** - Making it a practical tool for plugin development and WASM-based applications
- ✅ **Stay runtime agnostic** - Preserving the brilliant design that works with Wasmi, Wasmtime, and other backends
- ✅ **Build community** - Welcoming contributions to help maintain and improve the project

### 💡 The Brilliant Design

The original author's vision of a **runtime-agnostic component layer** combined with the **wasm_runtime_layer** abstraction is truly innovative. This design allows you to:

- Switch between different WASM runtimes (Wasmi, Wasmtime, etc.) without changing your code
- Use the same Component Model API regardless of the underlying execution engine
- Build portable WASM applications that work across multiple platforms and backends

This architecture is **essential** for the WASM ecosystem as we wait for the Component Model to be finalized and fully supported across all runtimes.

---

## ✨ Features

### Core Capabilities

- 🔧 **Component Model Support** - Full implementation of WebAssembly Component Model
- 🔄 **Runtime Agnostic** - Works with Wasmi, Wasmtime, and any backend via `wasm_runtime_layer`
- 🎭 **Type System** - Complete support for WIT types: records, variants, enums, resources, etc.
- 🚀 **Zero Unsafe Code** - 100% safe Rust implementation
- 📦 **Resource Management** - Proper handling of owned and borrowed resources with destructors
- 🔗 **Dynamic Loading** - Runtime inspection and generation of component interface types
- ⚡ **Optimized Lists** - Specialized list types for faster lifting/lowering operations

### NEW: wit-bindgen-wcl

**`wit-bindgen-wcl`** is a command-line tool that generates ergonomic Rust host bindings from WIT files:

- ✨ **Type-Safe Bindings** - Generates strongly-typed Rust code from WIT definitions
- 🎯 **Easy Integration** - Simple workflow from WIT → Rust bindings → Host application
- 🚀 **Top-Level Functions** - Full support for top-level function imports and exports (not just interfaces)
- 🔧 **Active Development** - Basic features working for simple use cases (see examples)
- 🤝 **Community Needed** - Heavy development in progress, contributions welcome!

**Current Status:** Works for simple to moderate complexity WIT files. Supports top-level functions, interfaces, records, variants, enums, options, results, and more. See the `examples/` directory for supported patterns.

---

## 🏗️ Workspace Structure

This workspace contains two main crates:

```
waclay/
├── crates/
│   ├── waclay/             # 🎯 Core Component Layer Library
│   │   ├── src/            # Runtime-agnostic component model implementation
│   │   ├── examples/       # 10 comprehensive examples showing features
│   │   └── docs/           # API documentation and guides
│   │
│   └── wit-bindgen-wcl/    # 🔧 WIT Binding Generator (NEW!)
│       ├── src/            # Code generator for host bindings
│       └── examples/       # 9 examples with generated bindings
│
├── test-waclay.ps1         # Test suite for core library
├── test-wit-bindgen.ps1    # Test suite for binding generator
└── test-all.ps1            # Complete workspace tests
```

### 📦 Crates

#### `waclay` - Core Library

Runtime-agnostic WebAssembly Component Model implementation. Load and execute components on any WASM runtime!

- **Version:** 0.1.3
- **License:** Apache-2.0
- **Repository:** https://github.com/HemantKArya/waclay

#### `wit-bindgen-wcl` - Binding Generator

Generate type-safe Rust host bindings from WIT files. Makes working with components much more ergonomic!

- **Status:** Basic features working, contributions welcome
- **Use Cases:** Simple to moderate complexity WIT files (check examples)
- **Community:** Contributors needed to expand capabilities

---

## 🚀 Quick Start

### Prerequisites

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install wasm-tools for building components
cargo install wasm-tools
```

### Installation
```bash
cargo build
```

### Installation

#### Option 1: Use in Your Project (Recommended)

Add to your `Cargo.toml`:

```toml
[dependencies]
waclay = { git = "https://github.com/HemantKArya/waclay" }
# Choose your runtime backend
wasmi_runtime_layer = "0.51.0"
# OR
# wasmtime_runtime_layer = "21.0.0"
```

> **Note:** Not yet published to crates.io - may be published in the future. Use git dependency for now.

**Optional: Install wit-bindgen-wcl for generating bindings**

If you want to generate host bindings from WIT files:

```bash
# Install the binding generator tool globally
cargo install --git https://github.com/HemantKArya/waclay wit-bindgen-wcl

# Now you can use it
wit-bindgen-wcl ./path/to/wit ./bindings.rs
```

#### Option 2: Build from Source

```bash
git clone https://github.com/HemantKArya/waclay.git
cd waclay

# Build the workspace
cargo build --release

# Install the binding generator
cargo install --path crates/wit-bindgen-wcl
```

### Basic Usage Example

### Basic Usage Example

Here's a simple example of loading and calling a WASM component:

**1. Define your WIT interface** (`guest.wit`)
**1. Define your WIT interface** (`guest.wit`)

```wit
package test:guest

interface foo {
    // Selects the item in position n within list x
    select-nth: func(x: list<string>, n: u32) -> string
}

world guest {
    export foo
}
```

**2. Load and call the component** (Rust host code)

```rust
use waclay::*;

// The bytes of the component.
const WASM: &[u8] = include_bytes!("single_component/component.wasm");

pub fn main() {
    // Create a new engine for instantiating a component.
    let engine = Engine::new(wasmi_runtime_layer::Engine::default());

    // Create a store for managing WASM data and any custom user-defined state.
    let mut store = Store::new(&engine, ());

    // Parse the component bytes and load its imports and exports.
    let component = Component::new(&engine, WASM).unwrap();
    // Create a linker that will be used to resolve the component's imports, if any.
    let linker = Linker::default();
    // Create an instance of the component using the linker.
    let instance = linker.instantiate(&mut store, &component).unwrap();

    // Get the interface that the interface exports.
    let interface = instance.exports().instance(&"test:guest/foo".try_into().unwrap()).unwrap();
    // Get the function for selecting a list element.
    let select_nth = interface.func("select-nth").unwrap().typed::<(Vec<String>, u32), String>().unwrap();

    // Create an example list to test upon.
    let example = ["a", "b", "c"].iter().map(ToString::to_string).collect::<Vec<_>>();

    println!("Calling select-nth({example:?}, 1) == {}", select_nth.call(&mut store, (example.clone(), 1)).unwrap());
    // Prints 'Calling select-nth(["a", "b", "c"], 1) == b'
}
```

That's it! You've successfully loaded and executed a WebAssembly component. 🎉

### Using wit-bindgen-wcl for Better Ergonomics

Generate type-safe bindings from WIT files:

```bash
# Generate bindings from WIT directory
wit-bindgen-wcl ./path/to/wit ./bindings.rs

# Use in your code
cargo run --bin wit-bindgen-wcl -- ./guest.wit ./bindings.rs
```

Then use the generated bindings for type-safe, ergonomic API:

```rust
mod bindings;
use bindings::*;  // Type-safe functions generated from WIT

// Much more ergonomic than raw Value manipulation!
```

#### Top-Level Function Support (NEW!)

wit-bindgen-wcl now supports top-level functions in WIT worlds:

```wit
world example {
    // Top-level imports (host provides)
    import multiply: func(a: f32, b: f32) -> f32;
    
    // Top-level exports (guest provides)
    export add: func(a: f32, b: f32) -> f32;
}
```

The generator creates:
- **For imports**: Host traits and registration functions using `linker.root_mut()`
- **For exports**: Helper functions to access via `instance.exports().root()`

This matches the wasmtime/wit-bindgen behavior and enables more flexible component designs.

---

## 🎯 How It Works

### The Runtime Agnostic Architecture

```
┌─────────────────────────────────────────────┐
│           Your Application                  │
├─────────────────────────────────────────────┤
│         waclay (Component Layer)            │
│  • Component Model Implementation           │
│  • Type System & Lifting/Lowering           │
│  • Resource Management                      │
├─────────────────────────────────────────────┤
│    wasm_runtime_layer (Abstraction)         │
│  • Common Runtime Interface                 │
│  • Backend Agnostic API                     │
├──────────┬──────────┬──────────┬────────────┤
│  Wasmi   │ Wasmtime │  Wasmer  │  Browser   │
│ Runtime  │  Runtime │  Runtime │   (JS)     │
└──────────┴──────────┴──────────┴────────────┘
```

**Key Benefits:**

1. **Write once, run anywhere** - Your code works with any runtime
2. **Easy runtime switching** - Change one line to switch backends
3. **Future-proof** - As new runtimes emerge, they just work
4. **Testability** - Test with lightweight runtime, deploy with optimized one

---

## 🤝 Contributing

**We need your help!** This project is maintained and welcoming community contributions.

### Why Contribute?

- 🌟 **Shape the Future** - Help build critical WebAssembly infrastructure
- 📚 **Learn WASM** - Deep dive into Component Model internals
- 🎯 **Real Impact** - Used in production for plugin systems and WASM applications
- 🤗 **Welcoming Community** - All skill levels welcome, we'll help you get started

### What We Need

| Area | Priority | Description |
|------|----------|-------------|
| 🔧 **wit-bindgen-wcl** | 🔴 High | Expand binding generation for complex WIT patterns |
| 📝 **Documentation** | 🔴 High | API docs, tutorials, and guides |
| 🧪 **Testing** | 🟡 Medium | More comprehensive tests and edge cases |
| 🐛 **Bug Fixes** | 🟢 Ongoing | Report and fix issues |
| 💡 **Features** | 🟢 Ongoing | String transcoders, subtyping support |
| 🎨 **Examples** | 🟢 Ongoing | More real-world use cases |

### Getting Started

1. **Fork the repository**
2. **Pick an issue** or create a new one
3. **Make your changes** with tests
4. **Submit a PR** with a clear description

Check out existing examples and tests to understand the codebase. Don't hesitate to ask questions in issues!

### Development Setup

```bash
# Clone your fork
git clone https://github.com/YourUsername/waclay.git
cd waclay

# Run tests to ensure everything works
.\test-all.ps1 -Fast

# Make your changes and test again
# ... your awesome contributions ...

.\test-all.ps1 -Fast
```

---

## 🗺️ Roadmap

### Current Focus (v0.1.x)

- [x] Maintain compatibility with latest Rust
- [x] Basic `wit-bindgen-wcl` functionality
- [ ] Comprehensive documentation
- [ ] More `wit-bindgen-wcl` features
- [ ] String transcoder support
- [ ] Subtyping support

### Future (v0.2.x+)

- [ ] Publish to crates.io
- [ ] Host binding macro (`#[derive(HostBindings)]`)
- [ ] Performance optimizations
- [ ] More runtime backend support
- [ ] WASI Preview 2 integration examples

**Note:** This project aims to bridge the gap until the Component Model is finalized by the WASI community and fully supported across all major runtimes. Community contributions are essential to keep it up-to-date.

---

## 📚 Examples

This repository includes **20 comprehensive examples** demonstrating various features:

### 🔹 Core Library Examples (10 examples)

Using the raw component API - great for learning the fundamentals:

### 🔹 Core Library Examples (10 examples)

Using the raw component API - great for learning the fundamentals:

```bash
# From workspace root
cargo run --example single_component     # ✅ Simple component instantiation
cargo run --example string_host_guest    # ✅ String passing between host/guest
cargo run --example func_param           # ✅ Function parameters
cargo run --example record_response      # ✅ Record types
cargo run --example option_result        # ✅ Option and Result types
cargo run --example variant_return       # ✅ Variant types
cargo run --example complex_return       # ✅ Complex return types
cargo run --example resource             # ✅ Resource handling
cargo run --example guest_resource       # ✅ Guest-defined resources
cargo run --example multilevel_resource  # ✅ Multi-level resources
```

### 🔸 Generated Bindings Examples (10 examples)

Using `wit-bindgen-wcl` for type-safe, ergonomic code:

```bash
# From workspace root
cargo run --example bindgen-calculator         # ✅ Calculator with logging & error handling
cargo run --example bindgen-web-scraper        # ✅ Web scraping component
cargo run --example bindgen-single-component   # ✅ Basic binding generation
cargo run --example bindgen-string-host-guest  # ✅ String passing with bindings
cargo run --example bindgen-func-param         # ✅ Function parameters with bindings
cargo run --example bindgen-record-response    # ✅ Record types with bindings
cargo run --example bindgen-option-result      # ✅ Option and Result with bindings
cargo run --example bindgen-variant-return     # ✅ Variant types with bindings
cargo run --example bindgen-complex-return     # ✅ Complex return types with bindings

# NEW: Top-level function support
cd crates/wit-bindgen-wcl/examples/toplevel-functions/host && cargo run --release
                                                # ✅ Top-level function imports & exports
```

> **💡 Tip:** Examples prefixed with `bindgen-` use generated bindings (more ergonomic), while others use the raw API (more flexible).

### Building Your Own Components

Want to build the example components yourself?

```bash
# Navigate to any component example
cd crates/wit-bindgen-wcl/examples/calculator/component

# Install nightly toolchain
rustup toolchain install nightly
rustup override set nightly

# Build the WASM module
cargo build --target wasm32-unknown-unknown --release

# Convert to component
wasm-tools component new \
  target/wasm32-unknown-unknown/release/calculator.wasm \
  -o component.wasm
```

---

## 🧪 Testing

Comprehensive test suite with cross-platform support:

```bash
# 🚀 Quick tests (recommended for development)
.\test-all.ps1 -Fast

# 🌍 Full test suite (all platforms)
.\test-all.ps1

# 🎯 Test specific crate
.\test-waclay.ps1           # Test core library only
.\test-wit-bindgen.ps1      # Test binding generator only

# ⚙️ Advanced options
.\test-waclay.ps1 -SkipAndroid -SkipLinux    # Skip cross-compilation
.\test-wit-bindgen.ps1 -SkipExamples         # Skip example builds
```

**Test Coverage:**
- ✅ Unit tests and integration tests
- ✅ All 19 examples build and run
- ✅ Cross-platform compatibility (Windows, Linux, Android)
- ✅ Multiple runtime backends

---

## ⚙️ Optional Features

- **`serde`** - Enable serialization for types and values (resources excluded as they're instance-bound)

```toml
waclay = { git = "https://github.com/HemantKArya/waclay", features = ["serde"] }
```

---

## 📋 Supported Capabilities

> **📄 For a comprehensive feature comparison with wasmtime/wit-bindgen, see [FEATURES.md](FEATURES.md)**

### ✅ Fully Supported

- ✅ Component parsing and instantiation
- ✅ All WIT types (records, variants, enums, options, results, etc.)
- ✅ Top-level functions (imports and exports)
- ✅ Specialized list types for performance
- ✅ Structural type equality
- ✅ Guest resources
- ✅ Host resources with destructors
- ✅ Runtime type inspection
- ✅ Multiple runtime backends

### 🚧 In Progress

- 🚧 Resource type bindings in `wit-bindgen-wcl`
- 🚧 Comprehensive testing suite
- 🚧 Documentation and tutorials

### ❌ Not Supported

- ❌ Future types (`future<T>`) - Requires async runtime support in core
- ❌ Stream types (`stream<T>`) - Requires async runtime support in core
- ❌ Async/await patterns - Fundamental limitation requiring core changes

### 📋 Planned

- 📋 Resource bindings generation in wit-bindgen-wcl
- 📋 String transcoders
- 📋 Host binding macros
- 📋 Subtyping support
- 📋 Performance benchmarks

> **Note**: For detailed feature comparison and workarounds, see [FEATURES.md](FEATURES.md)

---

## 📜 License

Licensed under the Apache License, Version 2.0. See [LICENSE](LICENSE) for details.

---

## 🙏 Credits

### Original Author

**Huge thanks to [DouglasDwyer](https://github.com/DouglasDwyer)** for creating the original `wasm_component_layer` project. The core architecture and design are his brilliant work:

- 💡 **Visionary Design** - The runtime-agnostic component layer concept
- 🏗️ **Solid Foundation** - Clean, well-structured codebase  
- 🔧 **wasm_runtime_layer** - The abstraction that makes it all possible

Without this foundation, this project wouldn't exist.

### This Fork

**Maintained by [HemantKArya](https://github.com/HemantKArya)** since the original project was discontinued:

- 🚀 Trying to keep it compiling with modern Rust
- ✨ New `wit-bindgen-wcl` tool
- 📚 Improved documentation
- 🤝 Community building

### Built With

This project stands on the shoulders of giants:

- [wasmtime](https://github.com/bytecodealliance/wasmtime) - Component Model implementation reference
- [wit-parser](https://github.com/bytecodealliance/wasm-tools) - WIT parsing and validation
- [wasm_runtime_layer](https://github.com/DouglasDwyer/wasm_runtime_layer) - Runtime abstraction
- All the runtime backends: Wasmi, Wasmtime, and others

---

## 📞 Get in Touch

- 🐛 **Found a bug?** [Open an issue](https://github.com/HemantKArya/waclay/issues)
- 💡 **Have an idea?** [Start a discussion](https://github.com/HemantKArya/waclay/discussions)
- 🤝 **Want to contribute?** Check our [Contributing](#-contributing) section
- ⭐ **Like the project?** Give us a star on GitHub!

---

<div align="center">

**Built with ❤️ for the WebAssembly community**

*Making Component Model accessible to everyone, one runtime at a time*

[⬆ Back to Top](#-waclay)

</div>
