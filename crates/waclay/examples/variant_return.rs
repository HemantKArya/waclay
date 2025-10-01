use anyhow::*;
use waclay::*;

// The bytes of the component.
const WASM: &[u8] = include_bytes!("variant_return/component.wasm");

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
                    VariantCase::new(
                        "completed",
                        Some(ValueType::Result(ResultType::new(
                            Some(ValueType::String),
                            Some(ValueType::String),
                        ))),
                    ),
                    VariantCase::new("failed", Some(ValueType::String)),
                ],
            )
            .unwrap(),
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
                    if let Some(Value::String(s)) = payload {
                        Ok(Status::Running(s.to_string()))
                    } else {
                        bail!("Expected string payload for running case")
                    }
                }
                "completed" => {
                    if let Some(Value::Result(res)) = payload {
                        let result = match res.as_ref() {
                            std::result::Result::Ok(Some(Value::String(s))) => {
                                std::result::Result::<String, String>::Ok(s.to_string())
                            }
                            std::result::Result::Err(Some(Value::String(s))) => {
                                std::result::Result::<String, String>::Err(s.to_string())
                            }
                            _ => bail!("Invalid result structure in completed case"),
                        };
                        Ok(Status::Completed(result))
                    } else {
                        bail!("Expected result payload for completed case")
                    }
                }
                "failed" => {
                    if let Some(Value::String(s)) = payload {
                        Ok(Status::Failed(s.to_string()))
                    } else {
                        bail!("Expected string payload for failed case")
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
                VariantCase::new(
                    "completed",
                    Some(ValueType::Result(ResultType::new(
                        Some(ValueType::String),
                        Some(ValueType::String),
                    ))),
                ),
                VariantCase::new("failed", Some(ValueType::String)),
            ],
        )
        .unwrap();

        let (discriminant, payload) = match self {
            Status::Pending => (0, None),
            Status::Running(msg) => (1, Some(Value::String(msg.into()))),
            Status::Completed(result) => {
                let result_value = match result {
                    std::result::Result::Ok(s) => ResultValue::new(
                        ResultType::new(Some(ValueType::String), Some(ValueType::String)),
                        std::result::Result::Ok(Some(Value::String(s.into()))),
                    )?,
                    std::result::Result::Err(s) => ResultValue::new(
                        ResultType::new(Some(ValueType::String), Some(ValueType::String)),
                        std::result::Result::Err(Some(Value::String(s.into()))),
                    )?,
                };
                (2, Some(Value::Result(result_value)))
            }
            Status::Failed(msg) => (3, Some(Value::String(msg.into()))),
        };

        let variant = Variant::new(variant_type, discriminant, payload)?;
        Ok(Value::Variant(variant))
    }
}

impl UnaryComponentType for Status {}

pub fn main() {
    println!("=== Variant Return Example ===");

    // Create a new engine for instantiating a component.
    let engine = Engine::new(wasmi_runtime_layer::Engine::default());

    // Create a store for managing WASM data and any custom user-defined state.
    let mut store = Store::new(&engine, ());

    // Parse the component bytes and load its imports and exports.
    let component = Component::new(&engine, WASM).unwrap();

    // Create a linker that will be used to resolve the component's imports.
    let linker = Linker::default();

    // Create an instance of the component using the linker.
    let instance = linker.instantiate(&mut store, &component).unwrap();

    // Get the interface that the component exports.
    let interface = instance
        .exports()
        .instance(&"test:guest/exports".try_into().unwrap())
        .unwrap();

    // Get the get-status function from the guest component.
    let get_status = interface
        .func("get-status")
        .unwrap()
        .typed::<(), (Status,)>()
        .unwrap();

    // Call the guest function and receive the variant status
    let (result,) = get_status.call(&mut store, ()).unwrap();

    println!("[Host] Received variant status:");
    match &result {
        Status::Pending => println!("  └ Status: Pending"),
        Status::Running(msg) => println!("  └ Status: Running - {}", msg),
        Status::Completed(res) => match res {
            std::result::Result::Ok(msg) => println!("  └ Status: Completed - {}", msg),
            std::result::Result::Err(err) => println!("  └ Status: Completed with error - {}", err),
        },
        Status::Failed(msg) => println!("  └ Status: Failed - {}", msg),
    }

    // Demonstrate manual variant handling as well
    println!("\n=== Manual Variant Handling ===");

    let get_status_manual = interface.func("get-status").unwrap();

    let mut results = vec![Value::Bool(false)]; // dummy value to be overwritten
    get_status_manual
        .call(
            &mut store,
            &[], // no parameters
            &mut results,
        )
        .unwrap();

    if let Value::Variant(variant) = &results[0] {
        println!("[Host] Manual variant handling:");
        let discriminant = variant.discriminant();
        let variant_ty = variant.ty();
        let case = &variant_ty.cases()[discriminant];
        let case_name = case.name();
        let payload = variant.value();

        match case_name {
            "pending" => println!("  └ Status: Pending"),
            "running" => {
                if let Some(Value::String(s)) = payload {
                    println!("  └ Status: Running - {}", s);
                }
            }
            "completed" => {
                if let Some(Value::Result(res)) = payload {
                    match res.as_ref() {
                        std::result::Result::Ok(Some(Value::String(s))) => {
                            println!("  └ Status: Completed - {}", s)
                        }
                        std::result::Result::Err(Some(Value::String(s))) => {
                            println!("  └ Status: Completed with error - {}", s)
                        }
                        _ => println!("  └ Status: Invalid result structure"),
                    }
                }
            }
            "failed" => {
                if let Some(Value::String(s)) = payload {
                    println!("  └ Status: Failed - {}", s);
                }
            }
            _ => println!("  └ Status: Unknown case '{}'", case_name),
        }
    }
}
