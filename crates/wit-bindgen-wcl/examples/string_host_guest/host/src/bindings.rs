// AUTO-GENERATED WIT BINDINGS for wasm-component-layer
// DO NOT EDIT - Regenerate using wit-bindgen-wcl

#![allow(dead_code, unused_imports, ambiguous_glob_reexports)]

use anyhow::*;
use wasm_component_layer::*;
use wasm_runtime_layer::backend;

// ========== Type Definitions ==========

// ========== Host Imports ==========

/// Host trait for interface: test:guest/host-logger
pub trait HostLoggerHost {
    fn host_log(&mut self, message: String) -> ();
}

pub mod imports {
    use super::*;

    pub fn register_host_logger_host<T: HostLoggerHost + 'static, E: backend::WasmEngine>(
        linker: &mut Linker,
        store: &mut Store<T, E>,
    ) -> Result<()> {
        let host_interface = linker
            .define_instance("test:guest/host-logger".try_into().unwrap())
            .context("Failed to define host interface")?;

        host_interface
            .define_func(
                "host-log",
                Func::new(
                    &mut *store,
                    FuncType::new([ValueType::String], []),
                    |mut ctx, params, _results| {
                        let message = if let Value::String(s) = &params[0] {
                            s.to_string()
                        } else {
                            bail!("Expected string")
                        };
                        ctx.data_mut().host_log(message);
                        Ok(())
                    },
                ),
            )
            .context("Failed to define host-log function")?;

        Ok(())
    }
}

// ========== Guest Exports ==========

pub mod exports_message {
    use super::*;

    pub const INTERFACE_NAME: &str = "test:guest/message";

    #[allow(clippy::type_complexity)]
    pub fn get_process_message<T, E: backend::WasmEngine>(
        instance: &Instance,
        _store: &mut Store<T, E>,
    ) -> Result<TypedFunc<String, String>> {
        let interface = instance
            .exports()
            .instance(&INTERFACE_NAME.try_into().unwrap())
            .ok_or_else(|| anyhow!("Interface not found"))?;

        interface
            .func("process-message")
            .ok_or_else(|| anyhow!("Function 'process-message' not found"))?
            .typed::<String, String>()
    }
}
