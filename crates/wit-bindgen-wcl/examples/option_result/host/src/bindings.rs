// AUTO-GENERATED WIT BINDINGS for wasm-component-layer
// DO NOT EDIT - Regenerate using wit-bindgen-wcl

#![allow(dead_code, unused_imports, ambiguous_glob_reexports)]

use anyhow::*;
use waclay::*;
use wasm_runtime_layer::{backend};


// ========== Type Definitions ==========






// ========== Host Imports ==========

/// Host trait for interface: test:guest/host
pub trait HostHost {
    fn log(&mut self, message: String) -> ();
    fn result_option(&mut self, is_some: bool) -> Option<String>;
    fn result_result(&mut self, is_ok: bool) -> Result<String, String>;
    fn result_result_ok(&mut self, is_ok: bool) -> Result<String, ()>;
    fn result_result_err(&mut self, is_ok: bool) -> Result<(), String>;
    fn result_result_none(&mut self, is_ok: bool) -> Result<(), ()>;
}

pub mod imports {
    use super::*;

    pub fn register_host_host<T: HostHost + 'static, E: backend::WasmEngine>(
        linker: &mut Linker,
        store: &mut Store<T, E>,
    ) -> Result<()> {
        let host_interface = linker
            .define_instance("test:guest/host".try_into().unwrap())
            .context("Failed to define host interface")?;

        host_interface
            .define_func(
                "log",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [ValueType::String, ],
                        [],
                    ),
                    |mut ctx, params, _results| {
                        let message = if let Value::String(s) = &params[0] { s.to_string() } else { bail!("Expected string") };
                        ctx.data_mut().log(message);
                        Ok(())
                    },
                ),
            )
            .context("Failed to define log function")?;

        host_interface
            .define_func(
                "result-option",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [ValueType::Bool, ],
                        [ValueType::Option(OptionType::new(ValueType::String))],
                    ),
                    |mut ctx, params, results| {
                        let is_some = if let Value::Bool(x) = &params[0] { *x } else { bail!("Expected bool") };
                        let result = ctx.data_mut().result_option(is_some);
                        results[0] = result.into_value()?;
                        Ok(())
                    },
                ),
            )
            .context("Failed to define result-option function")?;

        host_interface
            .define_func(
                "result-result",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [ValueType::Bool, ],
                        [ValueType::Result(ResultType::new(Some(ValueType::String), Some(ValueType::String)))],
                    ),
                    |mut ctx, params, results| {
                        let is_ok = if let Value::Bool(x) = &params[0] { *x } else { bail!("Expected bool") };
                        let result = ctx.data_mut().result_result(is_ok);
                        results[0] = result.into_value()?;
                        Ok(())
                    },
                ),
            )
            .context("Failed to define result-result function")?;

        host_interface
            .define_func(
                "result-result-ok",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [ValueType::Bool, ],
                        [ValueType::Result(ResultType::new(Some(ValueType::String), None))],
                    ),
                    |mut ctx, params, results| {
                        let is_ok = if let Value::Bool(x) = &params[0] { *x } else { bail!("Expected bool") };
                        let result = ctx.data_mut().result_result_ok(is_ok);
                        results[0] = result.into_value()?;
                        Ok(())
                    },
                ),
            )
            .context("Failed to define result-result-ok function")?;

        host_interface
            .define_func(
                "result-result-err",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [ValueType::Bool, ],
                        [ValueType::Result(ResultType::new(None, Some(ValueType::String)))],
                    ),
                    |mut ctx, params, results| {
                        let is_ok = if let Value::Bool(x) = &params[0] { *x } else { bail!("Expected bool") };
                        let result = ctx.data_mut().result_result_err(is_ok);
                        results[0] = result.into_value()?;
                        Ok(())
                    },
                ),
            )
            .context("Failed to define result-result-err function")?;

        host_interface
            .define_func(
                "result-result-none",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [ValueType::Bool, ],
                        [ValueType::Result(ResultType::new(None, None))],
                    ),
                    |mut ctx, params, results| {
                        let is_ok = if let Value::Bool(x) = &params[0] { *x } else { bail!("Expected bool") };
                        let result = ctx.data_mut().result_result_none(is_ok);
                        results[0] = result.into_value()?;
                        Ok(())
                    },
                ),
            )
            .context("Failed to define result-result-none function")?;

        Ok(())
    }

}

// ========== Guest Exports ==========

pub mod exports_run {
    use super::*;

    pub const INTERFACE_NAME: &str = "test:guest/run";

    #[allow(clippy::type_complexity)]
    pub fn get_start<T, E: backend::WasmEngine>(
        instance: &Instance,
        _store: &mut Store<T, E>,
    ) -> Result<TypedFunc<(), ()>> {
        let interface = instance
            .exports()
            .instance(&INTERFACE_NAME.try_into().unwrap())
            .ok_or_else(|| anyhow!("Interface not found"))?;

        interface
            .func("start")
            .ok_or_else(|| anyhow!("Function 'start' not found"))?
            .typed::<(), ()>()
    }

}

