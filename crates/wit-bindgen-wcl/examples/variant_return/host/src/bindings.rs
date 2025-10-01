// AUTO-GENERATED WIT BINDINGS for wasm-component-layer
// DO NOT EDIT - Regenerate using wit-bindgen-wcl

#![allow(dead_code, unused_imports, ambiguous_glob_reexports)]

use anyhow::*;
use waclay::*;
use wasm_runtime_layer::{backend};


// ========== Type Definitions ==========


#[derive(Debug, Clone)]
pub enum Status {
    Pending,
    Running(String),
    Completed(Result<String, String>),
    Failed(String),
}

impl ComponentType for Status {
    fn ty() -> ValueType {
        ValueType::Variant(
            VariantType::new(
                None,
                [
                    VariantCase::new("pending", None),
                    VariantCase::new("running", Some(ValueType::String)),
                    VariantCase::new("completed", Some(ValueType::Result(ResultType::new(Some(ValueType::String), Some(ValueType::String))))),
                    VariantCase::new("failed", Some(ValueType::String)),
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
                "pending" => Ok(Status::Pending),
                "running" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::String(s) = payload_value { s.to_string() } else { bail!("Expected string") };
                        Ok(Status::Running(converted))
                    } else {
                        bail!("Expected payload for running case")
                    }
                }
                "completed" => {
                    if let Some(payload_value) = payload {
                        let converted = Result::<String, String>::from_value(&payload_value)?;
                        Ok(Status::Completed(converted))
                    } else {
                        bail!("Expected payload for completed case")
                    }
                }
                "failed" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::String(s) = payload_value { s.to_string() } else { bail!("Expected string") };
                        Ok(Status::Failed(converted))
                    } else {
                        bail!("Expected payload for failed case")
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
                VariantCase::new("pending", None),
                VariantCase::new("running", Some(ValueType::String)),
                VariantCase::new("completed", Some(ValueType::Result(ResultType::new(Some(ValueType::String), Some(ValueType::String))))),
                VariantCase::new("failed", Some(ValueType::String)),
            ],
        ).unwrap();

        let (discriminant, payload) = match self {
            Status::Pending => (0, None),
            Status::Running(val) => (1, Some(Value::String(val.into()))),
            Status::Completed(val) => (2, Some(val.into_value()?)),
            Status::Failed(val) => (3, Some(Value::String(val.into()))),
        };

        let variant = Variant::new(variant_type, discriminant, payload)?;
        Ok(Value::Variant(variant))
    }
}

impl UnaryComponentType for Status {}

// ========== Guest Exports ==========

pub mod exports_exports {
    use super::*;

    pub const INTERFACE_NAME: &str = "test:guest/exports";

    #[allow(clippy::type_complexity)]
    pub fn get_get_status<T, E: backend::WasmEngine>(
        instance: &Instance,
        _store: &mut Store<T, E>,
    ) -> Result<TypedFunc<(), Status>> {
        let interface = instance
            .exports()
            .instance(&INTERFACE_NAME.try_into().unwrap())
            .ok_or_else(|| anyhow!("Interface not found"))?;

        interface
            .func("get-status")
            .ok_or_else(|| anyhow!("Function 'get-status' not found"))?
            .typed::<(), Status>()
    }

}

