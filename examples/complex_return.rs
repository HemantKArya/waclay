use anyhow::*;
use wasm_component_layer::*;

// The bytes of the component.
const WASM: &[u8] = include_bytes!("complex_return/component.wasm");

#[derive(Debug, Clone)]
struct ComplexData {
    id: u32,
    name: String,
    values: Vec<f64>,
    metadata: Option<String>,
    status: Result<String, String>,
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
                    ("metadata", ValueType::Option(OptionType::new(ValueType::String))),
                    ("status", ValueType::Result(ResultType::new(Some(ValueType::String), Some(ValueType::String)))),
                ],
            )
            .unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let id = record.field("id").ok_or_else(|| anyhow!("Missing 'id' field"))?;
            let name = record.field("name").ok_or_else(|| anyhow!("Missing 'name' field"))?;
            let values = record.field("values").ok_or_else(|| anyhow!("Missing 'values' field"))?;
            let metadata = record.field("metadata").ok_or_else(|| anyhow!("Missing 'metadata' field"))?;
            let status = record.field("status").ok_or_else(|| anyhow!("Missing 'status' field"))?;

            let id = if let Value::U32(x) = id { x } else { bail!("'id' field is not U32") };
            let name = if let Value::String(s) = name { s.to_string() } else { bail!("'name' field is not String") };
            let values = Vec::<f64>::from_value(&values)?;
            let metadata = Option::<String>::from_value(&metadata)?;
            let status = Result::<String, String>::from_value(&status)?;

            Ok(ComplexData { id, name, values, metadata, status })
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
                    ("metadata", ValueType::Option(OptionType::new(ValueType::String))),
                    ("status", ValueType::Result(ResultType::new(Some(ValueType::String), Some(ValueType::String)))),
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

pub fn main() {
    println!("=== Complex Return Example ===");

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

    // Get the get-complex-data function from the guest component.
    let get_complex_data = interface
        .func("get-complex-data")
        .unwrap()
        .typed::<(), (ComplexData,)>()
        .unwrap();

    // Call the guest function and receive the complex data
    let (result,) = get_complex_data.call(&mut store, ()).unwrap();

    println!("[Host] Received complex data:");
    println!("  └ id: {}", result.id);
    println!("  └ name: '{}'", result.name);
    println!("  └ values: {:?}", result.values);
    println!("  └ metadata: {:?}", result.metadata);
    println!("  └ status: {:?}", result.status);

    // Demonstrate manual record handling as well
    println!("\n=== Manual Complex Data Handling ===");

    let get_complex_data_manual = interface.func("get-complex-data").unwrap();

    let mut results = vec![Value::Bool(false)]; // dummy value to be overwritten
    get_complex_data_manual.call(
        &mut store,
        &[], // no parameters
        &mut results
    ).unwrap();

    if let Value::Record(record) = &results[0] {
        println!("[Host] Manual handling of complex record:");
        if let Some(Value::U32(id)) = record.field("id") {
            println!("  └ id: {}", id);
        }
        if let Some(Value::String(name)) = record.field("name") {
            println!("  └ name: '{}'", name);
        }
        if let Some(Value::List(values_list)) = record.field("values") {
            let values: Vec<f64> = Vec::<f64>::from_value(&Value::List(values_list.clone())).unwrap_or_default();
            println!("  └ values: {:?}", values);
        }
        if let Some(Value::Option(opt)) = record.field("metadata") {
            match opt.as_ref() {
                Some(Value::String(s)) => println!("  └ metadata: Some('{}')", s),
                None => println!("  └ metadata: None"),
                _ => println!("  └ metadata: Invalid type"),
            }
        }
        if let Some(Value::Result(res)) = record.field("status") {
            match res.as_ref() {
                std::result::Result::Ok(Some(Value::String(s))) => println!("  └ status: Ok('{}')", s),
                std::result::Result::Err(Some(Value::String(s))) => println!("  └ status: Err('{}')", s),
                _ => println!("  └ status: Invalid structure"),
            }
        }
    }
}