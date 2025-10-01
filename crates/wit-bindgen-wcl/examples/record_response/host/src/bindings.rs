// AUTO-GENERATED WIT BINDINGS for wasm-component-layer
// DO NOT EDIT - Regenerate using wit-bindgen-wcl

#![allow(dead_code, unused_imports, ambiguous_glob_reexports)]

use anyhow::*;
use waclay::*;
use wasm_runtime_layer::{backend};


// ========== Type Definitions ==========

#[derive(Debug, Clone)]
pub struct Response {
    pub id: u32,
    pub reply: String,
}

impl ComponentType for Response {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("id", ValueType::U32),
                    ("reply", ValueType::String),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let id = record
                .field("id")
                .ok_or_else(|| anyhow!("Missing 'id' field"))?;
            let reply = record
                .field("reply")
                .ok_or_else(|| anyhow!("Missing 'reply' field"))?;

            let id = if let Value::U32(x) = id { x } else { bail!("Expected u32") };
            let reply = if let Value::String(s) = reply { s.to_string() } else { bail!("Expected string") };

            Ok(Response {
                id,
                reply,
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
                    ("id", ValueType::U32),
                    ("reply", ValueType::String),
                ],
            ).unwrap(),
            [
                ("id", Value::U32(self.id)),
                ("reply", Value::String(self.reply.into())),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for Response {}

// ========== Guest Exports ==========

pub mod exports_message {
    use super::*;

    pub const INTERFACE_NAME: &str = "test:guest/message";

    #[allow(clippy::type_complexity)]
    pub fn get_process_message<T, E: backend::WasmEngine>(
        instance: &Instance,
        _store: &mut Store<T, E>,
    ) -> Result<TypedFunc<String, Response>> {
        let interface = instance
            .exports()
            .instance(&INTERFACE_NAME.try_into().unwrap())
            .ok_or_else(|| anyhow!("Interface not found"))?;

        interface
            .func("process-message")
            .ok_or_else(|| anyhow!("Function 'process-message' not found"))?
            .typed::<String, Response>()
    }

}

