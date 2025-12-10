// AUTO-GENERATED WIT BINDINGS for wasm-component-layer
// DO NOT EDIT - Regenerate using wit-bindgen-wcl

#![allow(dead_code, unused_imports, ambiguous_glob_reexports)]

use anyhow::*;
use waclay::*;
use wasm_runtime_layer::{backend};


// ========== Type Definitions ==========

// ========== Host Imports ==========

/// Host trait for top-level function: multiply
pub trait MultiplyHost {
    fn multiply(&mut self, a: f32, b: f32) -> f32;
}

/// Host trait for top-level function: log-message
pub trait LogMessageHost {
    fn log_message(&mut self, message: String) -> ();
}

pub mod imports {
    use super::*;

    pub fn register_multiply_host<T: MultiplyHost + 'static, E: backend::WasmEngine>(
        linker: &mut Linker,
        store: &mut Store<T, E>,
    ) -> Result<()> {
        linker
            .root_mut()
            .define_func(
                "multiply",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [ValueType::F32, ValueType::F32, ],
                        [ValueType::F32],
                    ),
                    |mut ctx, params, results| {
                        let a = if let Value::F32(x) = &params[0] { *x } else { bail!("Expected f32") };
                        let b = if let Value::F32(x) = &params[1] { *x } else { bail!("Expected f32") };
                        let result = ctx.data_mut().multiply(a, b);
                        results[0] = Value::F32(result);
                        Ok(())
                    },
                ),
            )
            .context("Failed to define top-level function 'multiply'")?;

        Ok(())
    }

    pub fn register_log_message_host<T: LogMessageHost + 'static, E: backend::WasmEngine>(
        linker: &mut Linker,
        store: &mut Store<T, E>,
    ) -> Result<()> {
        linker
            .root_mut()
            .define_func(
                "log-message",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [ValueType::String, ],
                        [],
                    ),
                    |mut ctx, params, _results| {
                        let message = if let Value::String(s) = &params[0] { s.to_string() } else { bail!("Expected string") };
                        ctx.data_mut().log_message(message);
                        Ok(())
                    },
                ),
            )
            .context("Failed to define top-level function 'log-message'")?;

        Ok(())
    }

}

// ========== Guest Exports ==========

pub mod exports_add {
    use super::*;

    #[allow(clippy::type_complexity)]
    pub fn get_add<T, E: backend::WasmEngine>(
        instance: &Instance,
        _store: &mut Store<T, E>,
    ) -> Result<TypedFunc<(f32, f32), f32>> {
        instance
            .exports()
            .root()
            .func("add")
            .ok_or_else(|| anyhow!("Top-level function 'add' not found"))?
            .typed::<(f32, f32), f32>()
    }
}

pub mod exports_compute {
    use super::*;

    #[allow(clippy::type_complexity)]
    pub fn get_compute<T, E: backend::WasmEngine>(
        instance: &Instance,
        _store: &mut Store<T, E>,
    ) -> Result<TypedFunc<(f32, f32), f32>> {
        instance
            .exports()
            .root()
            .func("compute")
            .ok_or_else(|| anyhow!("Top-level function 'compute' not found"))?
            .typed::<(f32, f32), f32>()
    }
}

