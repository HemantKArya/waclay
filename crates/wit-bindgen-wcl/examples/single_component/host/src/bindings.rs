// AUTO-GENERATED WIT BINDINGS for wasm-component-layer
// DO NOT EDIT - Regenerate using wit-bindgen-wcl

#![allow(dead_code, unused_imports, ambiguous_glob_reexports)]

use anyhow::*;
use waclay::*;
use wasm_runtime_layer::backend;

// ========== Type Definitions ==========

// ========== Guest Exports ==========

pub mod exports_foo {
    use super::*;

    pub const INTERFACE_NAME: &str = "test:guest/foo";

    #[allow(clippy::type_complexity)]
    pub fn get_select_nth<T, E: backend::WasmEngine>(
        instance: &Instance,
        _store: &mut Store<T, E>,
    ) -> Result<TypedFunc<(Vec<String>, u32), String>> {
        let interface = instance
            .exports()
            .instance(&INTERFACE_NAME.try_into().unwrap())
            .ok_or_else(|| anyhow!("Interface not found"))?;

        interface
            .func("select-nth")
            .ok_or_else(|| anyhow!("Function 'select-nth' not found"))?
            .typed::<(Vec<String>, u32), String>()
    }
}
