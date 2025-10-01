mod bindings;

use anyhow::Result;
use bindings::*;
use wasm_component_layer::*;

struct HostImpl;

impl HostHost for HostImpl {
    fn param_list(&mut self, param_s16: Vec<i16>) {
        println!("  param-list: {:?}", param_s16);
    }

    fn param_record(&mut self, param_record: Event) {
        println!("  param-record: {:?}", param_record);
    }

    fn param_option(&mut self, param_option: Option<u16>) {
        println!("  param-option: {:?}", param_option);
    }

    fn param_result_all(&mut self, result_all: Result<u8, u8>) {
        println!("  param-result-all: {:?}", result_all);
    }

    fn param_result_ok(&mut self, result_ok: Result<u8, ()>) {
        println!("  param-result-ok: {:?}", result_ok);
    }

    fn param_result_err(&mut self, result_err: Result<(), u8>) {
        println!("  param-result-err: {:?}", result_err);
    }

    fn param_result_none(&mut self, result_none: Result<(), ()>) {
        println!("  param-result-none: {:?}", result_none);
    }

    fn param_mult(
        &mut self,
        param_list: Vec<String>,
        param_record: Event,
        param_option: Option<String>,
        result_all: Result<String, String>,
    ) {
        println!("  param-mult:");
        println!("    list: {:?}", param_list);
        println!("    record: {:?}", param_record);
        println!("    option: {:?}", param_option);
        println!("    result: {:?}", result_all);
    }
}

fn main() -> Result<()> {
    println!("🎯 Function Parameters Example");
    println!("===============================\n");

    let engine = Engine::new(wasmi_runtime_layer::Engine::default());
    let mut store = Store::new(&engine, HostImpl);
    let component_bytes = std::fs::read("examples/func_param/component/component.wasm")?;
    let component = Component::new(&engine, &component_bytes)?;

    let mut linker = Linker::default();
    // Register host functions
    imports::register_host_host(&mut linker, &mut store)?;

    let instance = linker.instantiate(&mut store, &component)?;
    println!("✅ Component loaded\n");
    println!("📝 Testing Complex Function Parameters:\n");

    let start_func = exports_run::get_start(&instance, &mut store)?;
    start_func.call(&mut store, ())?;

    println!("\n✅ All tests completed!\n");
    println!("💡 Key Features:");
    println!("   • List parameters");
    println!("   • Record/Variant parameters");
    println!("   • Option parameters");
    println!("   • Result parameters (all variants)");
    println!("   • Multiple complex parameters in single function");

    Ok(())
}
