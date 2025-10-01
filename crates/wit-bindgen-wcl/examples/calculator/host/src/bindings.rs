// AUTO-GENERATED WIT BINDINGS for wasm-component-layer
// DO NOT EDIT - Regenerate using wit-bindgen-wcl

#![allow(dead_code, unused_imports, ambiguous_glob_reexports)]

use anyhow::*;
use wasm_component_layer::*;
use wasm_runtime_layer::{backend};


// ========== Type Definitions ==========

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warning,
    Error,
}

impl ComponentType for LogLevel {
    fn ty() -> ValueType {
        ValueType::Enum(EnumType::new(None, [
            "info",
            "warning",
            "error",
        ]).unwrap())
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Enum(enum_val) = value {
            let discriminant = enum_val.discriminant();
            match discriminant {
                0 => Ok(LogLevel::Info),
                1 => Ok(LogLevel::Warning),
                2 => Ok(LogLevel::Error),
                _ => bail!("Invalid enum discriminant: {}", discriminant),
            }
        } else {
            bail!("Expected Enum value")
        }
    }

    fn into_value(self) -> Result<Value> {
        let enum_type = EnumType::new(None, [
            "info",
            "warning",
            "error",
        ]).unwrap();

        let discriminant = match self {
            LogLevel::Info => 0,
            LogLevel::Warning => 1,
            LogLevel::Error => 2,
        };

        Ok(Value::Enum(Enum::new(enum_type, discriminant)?))
    }
}

impl UnaryComponentType for LogLevel {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    Add,
    Subtract,
    Multiply,
    Divide,
}

impl ComponentType for Operation {
    fn ty() -> ValueType {
        ValueType::Enum(EnumType::new(None, [
            "add",
            "subtract",
            "multiply",
            "divide",
        ]).unwrap())
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Enum(enum_val) = value {
            let discriminant = enum_val.discriminant();
            match discriminant {
                0 => Ok(Operation::Add),
                1 => Ok(Operation::Subtract),
                2 => Ok(Operation::Multiply),
                3 => Ok(Operation::Divide),
                _ => bail!("Invalid enum discriminant: {}", discriminant),
            }
        } else {
            bail!("Expected Enum value")
        }
    }

    fn into_value(self) -> Result<Value> {
        let enum_type = EnumType::new(None, [
            "add",
            "subtract",
            "multiply",
            "divide",
        ]).unwrap();

        let discriminant = match self {
            Operation::Add => 0,
            Operation::Subtract => 1,
            Operation::Multiply => 2,
            Operation::Divide => 3,
        };

        Ok(Value::Enum(Enum::new(enum_type, discriminant)?))
    }
}

impl UnaryComponentType for Operation {}

#[derive(Debug, Clone)]
pub struct CalcResult {
    pub value: f64,
    pub operation: Operation,
    pub success: bool,
}

impl ComponentType for CalcResult {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("value", ValueType::F64),
                    ("operation", Operation::ty()),
                    ("success", ValueType::Bool),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let value = record
                .field("value")
                .ok_or_else(|| anyhow!("Missing 'value' field"))?;
            let operation = record
                .field("operation")
                .ok_or_else(|| anyhow!("Missing 'operation' field"))?;
            let success = record
                .field("success")
                .ok_or_else(|| anyhow!("Missing 'success' field"))?;

            let value = if let Value::F64(x) = value { x } else { bail!("Expected f64") };
            let operation = Operation::from_value(&operation)?;
            let success = if let Value::Bool(x) = success { x } else { bail!("Expected bool") };

            Ok(CalcResult {
                value,
                operation,
                success,
            })
        } else {
            bail!("Expected Record value")
        }
    }

    fn into_value(self) -> Result<Value> {
        let record = Record::new(
            RecordType::new(
                None,
                [
                    ("value", ValueType::F64),
                    ("operation", Operation::ty()),
                    ("success", ValueType::Bool),
                ],
            ).unwrap(),
            [
                ("value", Value::F64(self.value)),
                ("operation", self.operation.into_value()?),
                ("success", Value::Bool(self.success)),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for CalcResult {}

#[derive(Debug, Clone)]
pub enum CalcError {
    DivisionByZero,
    Overflow,
    InvalidOperation(String),
}

impl ComponentType for CalcError {
    fn ty() -> ValueType {
        ValueType::Variant(
            VariantType::new(
                None,
                [
                    VariantCase::new("division-by-zero", None),
                    VariantCase::new("overflow", None),
                    VariantCase::new("invalid-operation", Some(ValueType::String)),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Variant(variant) = value {
            let discriminant = variant.discriminant();
            let variant_ty = variant.ty();
            let case = &variant_ty.cases()[discriminant];
            let case_name = case.name();
            let payload = variant.value();

            match case_name {
                "division-by-zero" => Ok(CalcError::DivisionByZero),
                "overflow" => Ok(CalcError::Overflow),
                "invalid-operation" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::String(s) = payload_value { s.to_string() } else { bail!("Expected string") };
                        Ok(CalcError::InvalidOperation(converted))
                    } else {
                        bail!("Expected payload for invalid-operation case")
                    }
                }
                _ => bail!("Unknown variant case: {}", case_name),
            }
        } else {
            bail!("Expected Variant value")
        }
    }

    fn into_value(self) -> Result<Value> {
        let variant_type = VariantType::new(
            None,
            [
                VariantCase::new("division-by-zero", None),
                VariantCase::new("overflow", None),
                VariantCase::new("invalid-operation", Some(ValueType::String)),
            ],
        ).unwrap();

        let (discriminant, payload) = match self {
            CalcError::DivisionByZero => (0, None),
            CalcError::Overflow => (1, None),
            CalcError::InvalidOperation(val) => (2, Some(Value::String(val.into()))),
        };

        let variant = Variant::new(variant_type, discriminant, payload)?;
        Ok(Value::Variant(variant))
    }
}

impl UnaryComponentType for CalcError {}



// ========== Host Imports ==========

/// Host trait for interface: example:calculator/logger
pub trait LoggerHost {
    fn log(&mut self, level: LogLevel, message: String) -> ();
}

pub mod imports {
    use super::*;

    pub fn register_logger_host<T: LoggerHost + 'static, E: backend::WasmEngine>(
        linker: &mut Linker,
        store: &mut Store<T, E>,
    ) -> Result<()> {
        let host_interface = linker
            .define_instance("example:calculator/logger".try_into().unwrap())
            .context("Failed to define host interface")?;

        host_interface
            .define_func(
                "log",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [LogLevel::ty(), ValueType::String, ],
                        [],
                    ),
                    |mut ctx, params, _results| {
                        let level = LogLevel::from_value(&params[0])?;
                        let message = if let Value::String(s) = &params[1] { s.to_string() } else { bail!("Expected string") };
                        ctx.data_mut().log(level, message);
                        Ok(())
                    },
                ),
            )
            .context("Failed to define log function")?;

        Ok(())
    }

}

// ========== Guest Exports ==========

pub mod exports_operations {
    use super::*;

    pub const INTERFACE_NAME: &str = "example:calculator/operations";

    #[allow(clippy::type_complexity)]
    pub fn get_calculate<T, E: backend::WasmEngine>(
        instance: &Instance,
        _store: &mut Store<T, E>,
    ) -> Result<TypedFunc<(Operation, f64, f64), Result<CalcResult, CalcError>>> {
        let interface = instance
            .exports()
            .instance(&INTERFACE_NAME.try_into().unwrap())
            .ok_or_else(|| anyhow!("Interface not found"))?;

        interface
            .func("calculate")
            .ok_or_else(|| anyhow!("Function 'calculate' not found"))?
            .typed::<(Operation, f64, f64), Result<CalcResult, CalcError>>()
    }

    #[allow(clippy::type_complexity)]
    pub fn get_get_history<T, E: backend::WasmEngine>(
        instance: &Instance,
        _store: &mut Store<T, E>,
    ) -> Result<TypedFunc<(), Vec<CalcResult>>> {
        let interface = instance
            .exports()
            .instance(&INTERFACE_NAME.try_into().unwrap())
            .ok_or_else(|| anyhow!("Interface not found"))?;

        interface
            .func("get-history")
            .ok_or_else(|| anyhow!("Function 'get-history' not found"))?
            .typed::<(), Vec<CalcResult>>()
    }

}

