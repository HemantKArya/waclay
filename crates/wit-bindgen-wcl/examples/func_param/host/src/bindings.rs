// AUTO-GENERATED WIT BINDINGS for wasm-component-layer
// DO NOT EDIT - Regenerate using wit-bindgen-wcl

#![allow(dead_code, unused_imports, ambiguous_glob_reexports)]

use anyhow::*;
use wasm_component_layer::*;
use wasm_runtime_layer::{backend};


// ========== Type Definitions ==========


#[derive(Debug, Clone)]
pub struct Rect {
    pub label: String,
    pub sides: Vec<i16>,
}

impl ComponentType for Rect {
    fn ty() -> ValueType {
        ValueType::Record(
            RecordType::new(
                None,
                [
                    ("label", ValueType::String),
                    ("sides", ValueType::List(ListType::new(ValueType::S16))),
                ],
            ).unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let label = record
                .field("label")
                .ok_or_else(|| anyhow!("Missing 'label' field"))?;
            let sides = record
                .field("sides")
                .ok_or_else(|| anyhow!("Missing 'sides' field"))?;

            let label = if let Value::String(s) = label { s.to_string() } else { bail!("Expected string") };
            let sides = Vec::<i16>::from_value(&sides)?;

            Ok(Rect {
                label,
                sides,
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
                    ("label", ValueType::String),
                    ("sides", ValueType::List(ListType::new(ValueType::S16))),
                ],
            ).unwrap(),
            [
                ("label", Value::String(self.label.into())),
                ("sides", self.sides.into_value()?),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for Rect {}

#[derive(Debug, Clone)]
pub enum ClickType {
    Up,
    Press(u8),
    Down,
}

impl ComponentType for ClickType {
    fn ty() -> ValueType {
        ValueType::Variant(
            VariantType::new(
                None,
                [
                    VariantCase::new("up", None),
                    VariantCase::new("press", Some(ValueType::U8)),
                    VariantCase::new("down", None),
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
                "up" => Ok(ClickType::Up),
                "press" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::U8(x) = payload_value { x } else { bail!("Expected u8") };
                        Ok(ClickType::Press(converted))
                    } else {
                        bail!("Expected payload for press case")
                    }
                }
                "down" => Ok(ClickType::Down),
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
                VariantCase::new("up", None),
                VariantCase::new("press", Some(ValueType::U8)),
                VariantCase::new("down", None),
            ],
        ).unwrap();

        let (discriminant, payload) = match self {
            ClickType::Up => (0, None),
            ClickType::Press(val) => (1, Some(Value::U8(val))),
            ClickType::Down => (2, None),
        };

        let variant = Variant::new(variant_type, discriminant, payload)?;
        Ok(Value::Variant(variant))
    }
}

impl UnaryComponentType for ClickType {}

#[derive(Debug, Clone)]
pub enum Event {
    Open,
    Close(u64),
    Click(ClickType),
}

impl ComponentType for Event {
    fn ty() -> ValueType {
        ValueType::Variant(
            VariantType::new(
                None,
                [
                    VariantCase::new("open", None),
                    VariantCase::new("close", Some(ValueType::U64)),
                    VariantCase::new("click", Some(ClickType::ty())),
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
                "open" => Ok(Event::Open),
                "close" => {
                    if let Some(payload_value) = payload {
                        let converted = if let Value::U64(x) = payload_value { x } else { bail!("Expected u64") };
                        Ok(Event::Close(converted))
                    } else {
                        bail!("Expected payload for close case")
                    }
                }
                "click" => {
                    if let Some(payload_value) = payload {
                        let converted = ClickType::from_value(&payload_value)?;
                        Ok(Event::Click(converted))
                    } else {
                        bail!("Expected payload for click case")
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
                VariantCase::new("open", None),
                VariantCase::new("close", Some(ValueType::U64)),
                VariantCase::new("click", Some(ClickType::ty())),
            ],
        ).unwrap();

        let (discriminant, payload) = match self {
            Event::Open => (0, None),
            Event::Close(val) => (1, Some(Value::U64(val))),
            Event::Click(val) => (2, Some(val.into_value()?)),
        };

        let variant = Variant::new(variant_type, discriminant, payload)?;
        Ok(Value::Variant(variant))
    }
}

impl UnaryComponentType for Event {}









// ========== Host Imports ==========

/// Host trait for interface: test:guest/host
pub trait HostHost {
    fn param_list(&mut self, param_s16: Vec<i16>) -> ();
    fn param_record(&mut self, param_record: Event) -> ();
    fn param_option(&mut self, param_option: Option<u16>) -> ();
    fn param_result_all(&mut self, result_all: Result<u8, u8>) -> ();
    fn param_result_ok(&mut self, result_ok: Result<u8, ()>) -> ();
    fn param_result_err(&mut self, result_err: Result<(), u8>) -> ();
    fn param_result_none(&mut self, result_none: Result<(), ()>) -> ();
    fn param_mult(&mut self, param_list: Vec<String>, param_record: Event, param_option: Option<String>, result_all: Result<String, String>) -> ();
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
                "param-list",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [ValueType::List(ListType::new(ValueType::S16)), ],
                        [],
                    ),
                    |mut ctx, params, _results| {
                        let param_s16 = Vec::<i16>::from_value(&params[0])?;
                        ctx.data_mut().param_list(param_s16);
                        Ok(())
                    },
                ),
            )
            .context("Failed to define param-list function")?;

        host_interface
            .define_func(
                "param-record",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [Event::ty(), ],
                        [],
                    ),
                    |mut ctx, params, _results| {
                        let param_record = Event::from_value(&params[0])?;
                        ctx.data_mut().param_record(param_record);
                        Ok(())
                    },
                ),
            )
            .context("Failed to define param-record function")?;

        host_interface
            .define_func(
                "param-option",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [ValueType::Option(OptionType::new(ValueType::U16)), ],
                        [],
                    ),
                    |mut ctx, params, _results| {
                        let param_option = Option::<u16>::from_value(&params[0])?;
                        ctx.data_mut().param_option(param_option);
                        Ok(())
                    },
                ),
            )
            .context("Failed to define param-option function")?;

        host_interface
            .define_func(
                "param-result-all",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [ValueType::Result(ResultType::new(Some(ValueType::U8), Some(ValueType::U8))), ],
                        [],
                    ),
                    |mut ctx, params, _results| {
                        let result_all = Result::<u8, u8>::from_value(&params[0])?;
                        ctx.data_mut().param_result_all(result_all);
                        Ok(())
                    },
                ),
            )
            .context("Failed to define param-result-all function")?;

        host_interface
            .define_func(
                "param-result-ok",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [ValueType::Result(ResultType::new(Some(ValueType::U8), None)), ],
                        [],
                    ),
                    |mut ctx, params, _results| {
                        let result_ok = Result::<u8, ()>::from_value(&params[0])?;
                        ctx.data_mut().param_result_ok(result_ok);
                        Ok(())
                    },
                ),
            )
            .context("Failed to define param-result-ok function")?;

        host_interface
            .define_func(
                "param-result-err",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [ValueType::Result(ResultType::new(None, Some(ValueType::U8))), ],
                        [],
                    ),
                    |mut ctx, params, _results| {
                        let result_err = Result::<(), u8>::from_value(&params[0])?;
                        ctx.data_mut().param_result_err(result_err);
                        Ok(())
                    },
                ),
            )
            .context("Failed to define param-result-err function")?;

        host_interface
            .define_func(
                "param-result-none",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [ValueType::Result(ResultType::new(None, None)), ],
                        [],
                    ),
                    |mut ctx, params, _results| {
                        let result_none = Result::<(), ()>::from_value(&params[0])?;
                        ctx.data_mut().param_result_none(result_none);
                        Ok(())
                    },
                ),
            )
            .context("Failed to define param-result-none function")?;

        host_interface
            .define_func(
                "param-mult",
                Func::new(
                    &mut *store,
                    FuncType::new(
                        [ValueType::List(ListType::new(ValueType::String)), Event::ty(), ValueType::Option(OptionType::new(ValueType::String)), ValueType::Result(ResultType::new(Some(ValueType::String), Some(ValueType::String))), ],
                        [],
                    ),
                    |mut ctx, params, _results| {
                        let param_list = Vec::<String>::from_value(&params[0])?;
                        let param_record = Event::from_value(&params[1])?;
                        let param_option = Option::<String>::from_value(&params[2])?;
                        let result_all = Result::<String, String>::from_value(&params[3])?;
                        ctx.data_mut().param_mult(param_list, param_record, param_option, result_all);
                        Ok(())
                    },
                ),
            )
            .context("Failed to define param-mult function")?;

        Ok(())
    }

}

// ========== Guest Exports ==========

pub mod exports_run {
    use super::*;

    pub const INTERFACE_NAME: &str = "test:guest/run";

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

