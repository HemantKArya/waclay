// AUTO-GENERATED WIT BINDINGS for wasm-component-layer
// DO NOT EDIT - Regenerate using wit-bindgen-wcl

#![allow(dead_code, unused_imports, ambiguous_glob_reexports)]

use anyhow::*;
use wasm_component_layer::*;
use wasm_runtime_layer::backend;

// ========== Type Definitions ==========

#[derive(Debug, Clone)]
pub struct ComplexData {
    pub id: u32,
    pub name: String,
    pub values: Vec<f64>,
    pub metadata: Option<String>,
    pub status: Result<String, String>,
}

impl ComponentType for ComplexData {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("id", ValueType::U32),
                    ("name", ValueType::String),
                    ("values", ValueType::List(ListType::new(ValueType::F64))),
                    (
                        "metadata",
                        ValueType::Option(OptionType::new(ValueType::String)),
                    ),
                    (
                        "status",
                        ValueType::Result(ResultType::new(
                            Some(ValueType::String),
                            Some(ValueType::String),
                        )),
                    ),
                ],
            )
            .unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let id = record
                .field("id")
                .ok_or_else(|| anyhow!("Missing 'id' field"))?;
            let name = record
                .field("name")
                .ok_or_else(|| anyhow!("Missing 'name' field"))?;
            let values = record
                .field("values")
                .ok_or_else(|| anyhow!("Missing 'values' field"))?;
            let metadata = record
                .field("metadata")
                .ok_or_else(|| anyhow!("Missing 'metadata' field"))?;
            let status = record
                .field("status")
                .ok_or_else(|| anyhow!("Missing 'status' field"))?;

            let id = if let Value::U32(x) = id {
                x
            } else {
                bail!("Expected u32")
            };
            let name = if let Value::String(s) = name {
                s.to_string()
            } else {
                bail!("Expected string")
            };
            let values = Vec::<f64>::from_value(&values)?;
            let metadata = Option::<String>::from_value(&metadata)?;
            let status = Result::<String, String>::from_value(&status)?;

            Ok(ComplexData {
                id,
                name,
                values,
                metadata,
                status,
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
                    ("name", ValueType::String),
                    ("values", ValueType::List(ListType::new(ValueType::F64))),
                    (
                        "metadata",
                        ValueType::Option(OptionType::new(ValueType::String)),
                    ),
                    (
                        "status",
                        ValueType::Result(ResultType::new(
                            Some(ValueType::String),
                            Some(ValueType::String),
                        )),
                    ),
                ],
            )
            .unwrap(),
            [
                ("id", Value::U32(self.id)),
                ("name", Value::String(self.name.into())),
                ("values", self.values.into_value()?),
                ("metadata", self.metadata.into_value()?),
                ("status", self.status.into_value()?),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for ComplexData {}

// ========== Guest Exports ==========

pub mod exports_exports {
    use super::*;

    pub const INTERFACE_NAME: &str = "test:guest/exports";

    #[allow(clippy::type_complexity)]
    pub fn get_get_complex_data<T, E: backend::WasmEngine>(
        instance: &Instance,
        _store: &mut Store<T, E>,
    ) -> Result<TypedFunc<(), ComplexData>> {
        let interface = instance
            .exports()
            .instance(&INTERFACE_NAME.try_into().unwrap())
            .ok_or_else(|| anyhow!("Interface not found"))?;

        interface
            .func("get-complex-data")
            .ok_or_else(|| anyhow!("Function 'get-complex-data' not found"))?
            .typed::<(), ComplexData>()
    }
}
