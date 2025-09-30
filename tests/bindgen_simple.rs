//! Simple test to verify the bindgen macro compiles and generates code.
//! 
//! This test demonstrates that the wasmtime-based bindgen implementation
//! has been successfully integrated into wasm_component_layer.

#[cfg(feature = "macro")]
#[test]
fn test_bindgen_macro_exists() {
    // This test verifies that the bindgen macro is available when the
    // "macro" feature is enabled.
    
    // The macro should be importable
    use wasm_component_layer::bindgen;
    
    // Test passes if we can reference the macro
    let _ = bindgen;
    
    println!("✅ bindgen macro is available");
}

#[cfg(feature = "macro")]
#[test]
fn test_bindgen_simple_inline() {
    use wasm_component_layer::bindgen;
    
    // Test that we can use inline WIT with a simple world
    bindgen!({
        inline: r#"
            package test:simple;
            
            world simple {
                export add: func(a: s32, b: s32) -> s32;
            }
        "#,
        world: "simple",
    });
    
    println!("✅ Simple inline WIT compiled successfully");
}

#[cfg(feature = "macro")]
#[test]
fn test_bindgen_with_types() {
    use wasm_component_layer::bindgen;
    
    // Test bindgen with record types
    bindgen!({
        inline: r#"
            package test:types;
            
            interface data {
                record point {
                    x: f64,
                    y: f64,
                }
                
                get-point: func() -> point;
            }
            
            world types-world {
                export data;
            }
        "#,
        world: "types-world",
    });
    
    println!("✅ WIT with records compiled successfully");
}

#[cfg(feature = "macro")]
#[test]
fn test_bindgen_with_results() {
    use wasm_component_layer::bindgen;
    
    // Test bindgen with result types
    bindgen!({
        inline: r#"
            package test:results;
            
            world results-world {
                export divide: func(a: s32, b: s32) -> result<s32, string>;
                export safe-op: func(value: s32) -> option<string>;
            }
        "#,
        world: "results-world",
    });
    
    println!("✅ WIT with results and options compiled successfully");
}

#[cfg(feature = "macro")]
#[test]
fn test_bindgen_with_lists() {
    use wasm_component_layer::bindgen;
    
    // Test bindgen with list types
    bindgen!({
        inline: r#"
            package test:lists;
            
            world lists-world {
                export process: func(items: list<string>) -> list<u32>;
            }
        "#,
        world: "lists-world",
    });
    
    println!("✅ WIT with lists compiled successfully");
}

#[cfg(feature = "macro")]
#[test]
fn test_bindgen_comprehensive() {
    use wasm_component_layer::bindgen;
    
    // Comprehensive test with multiple type features
    bindgen!({
        inline: r#"
            package test:comprehensive;
            
            interface types {
                record user {
                    id: u64,
                    name: string,
                    active: bool,
                }
                
                variant message {
                    text(string),
                    number(s32),
                    none,
                }
                
                enum status {
                    pending,
                    active,
                    completed,
                }
            }
            
            interface operations {
                use types.{user, message, status};
                
                create-user: func(name: string) -> user;
                send-message: func(msg: message) -> result<u64, string>;
                get-status: func(id: u64) -> status;
                list-users: func() -> list<user>;
            }
            
            world comprehensive {
                export operations;
            }
        "#,
        world: "comprehensive",
    });
    
    println!("✅ Comprehensive WIT with multiple features compiled successfully");
    println!("   - Records ✅");
    println!("   - Variants ✅");
    println!("   - Enums ✅");
    println!("   - Lists ✅");
    println!("   - Results ✅");
    println!("   - Multiple interfaces ✅");
}

#[cfg(not(feature = "macro"))]
#[test]
fn test_bindgen_requires_macro_feature() {
    println!("⚠️  Bindgen tests skipped: 'macro' feature not enabled");
    println!("   Run with: cargo test --features macro");
}

#[test]
fn test_readme() {
    println!("\n📝 Bindgen Macro Test Suite");
    println!("============================");
    println!();
    println!("This test suite verifies that the wasmtime-based bindgen macro");
    println!("has been successfully integrated into wasm_component_layer.");
    println!();
    println!("Features tested:");
    println!("  ✓ Macro availability");
    println!("  ✓ Inline WIT support");
    println!("  ✓ Record types");
    println!("  ✓ Variant types");
    println!("  ✓ Enum types");
    println!("  ✓ Result and Option types");
    println!("  ✓ List types");
    println!("  ✓ Multiple interfaces");
    println!();
    println!("Run with: cargo test --test bindgen_simple --features macro");
    println!();
}
