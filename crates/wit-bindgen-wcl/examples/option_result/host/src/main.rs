mod bindings;

use anyhow::Result;
use bindings::*;
use wasm_component_layer::*;

struct HostImpl;

impl HostHost for HostImpl {
    fn log(&mut self, message: String) {
        println!("  [LOG] {}", message);
    }

    fn result_option(&mut self, is_some: bool) -> Option<String> {
        if is_some {
            Some("some-value".to_string())
        } else {
            None
        }
    }

    fn result_result(&mut self, is_ok: bool) -> Result<String, String> {
        if is_ok {
            Ok("ok-value".to_string())
        } else {
            Err("error-value".to_string())
        }
    }

    fn result_result_ok(&mut self, is_ok: bool) -> Result<String, ()> {
        if is_ok {
            Ok("ok-only".to_string())
        } else {
            Err(())
        }
    }

    fn result_result_err(&mut self, is_ok: bool) -> Result<(), String> {
        if is_ok {
            Ok(())
        } else {
            Err("err-only".to_string())
        }
    }

    fn result_result_none(&mut self, is_ok: bool) -> Result<(), ()> {
        if is_ok {
            Ok(())
        } else {
            Err(())
        }
    }
}

fn main() -> Result<()> {
    println!("🔀 Option/Result Example");
    println!("=========================\n");

    let engine = Engine::new(wasmi_runtime_layer::Engine::default());
    let mut store = Store::new(&engine, HostImpl);
    let component_bytes = std::fs::read("examples/option_result/component/component.wasm")?;
    let component = Component::new(&engine, &component_bytes)?;
    
    let mut linker = Linker::default();
        // Register host functions
    imports::register_host_host(&mut linker, &mut store)?;
    
    let instance = linker.instantiate(&mut store, &component)?;
    println!("✅ Component loaded\n");

    let start_func = exports_run::get_start(&instance, &mut store)?;
    start_func.call(&mut store, ())?;
    
    println!("\n✅ All tests completed!\n");
    println!("💡 Key Features:");
    println!("   • Option<T> return types from host functions");
    println!("   • Result<T, E> return types with both ok and error");
    println!("   • Result<T> with only ok type");
    println!("   • Result<_, E> with only error type");
    println!("   • Result with neither type (unit result)");
    
    Ok(())
}
