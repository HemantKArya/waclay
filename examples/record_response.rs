use anyhow::*;
use wasm_component_layer::*;

// The bytes of the component.
const WASM: &[u8] = include_bytes!("record_response/component.wasm");

#[derive(Debug, Clone)]
struct Response {
    id: u32,
    reply: String,
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
            )
            .unwrap(),
        )
    }

    fn from_value(value: &Value) -> Result<Self> {
        if let Value::Record(record) = value {
            let id = record.field("id").ok_or_else(|| anyhow!("Missing 'id' field"))?;
            let reply = record.field("reply").ok_or_else(|| anyhow!("Missing 'reply' field"))?;

            let id = if let Value::U32(x) = id { x } else { bail!("'id' field is not U32") };
            let reply = if let Value::String(s) = reply { s.to_string() } else { bail!("'reply' field is not String") };

            Ok(Response { id, reply })
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
            )
            .unwrap(),
            [
                ("id", Value::U32(self.id)),
                ("reply", Value::String(self.reply.into())),
            ],
        )?;
        Ok(Value::Record(record))
    }
}

impl UnaryComponentType for Response {}

pub fn main() {
    println!("=== Record Response Example ===");

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
        .instance(&"test:guest/message".try_into().unwrap())
        .unwrap();

    // Get the process-message function from the guest component.
    let process_message = interface
        .func("process-message")
        .unwrap()
        .typed::<(String,), (Response,)>()
        .unwrap();

    // Call the guest function with a message and receive the record response
    let input_message = "Hello from host!".to_string();
    let (result,) = process_message.call(&mut store, (input_message.clone(),)).unwrap();

    println!("[Host] Input message: '{}'", input_message);
    println!("[Host] Received record response:");
    println!("  └ id: {}", result.id);
    println!("  └ reply: '{}'", result.reply);

    // Demonstrate manual record handling as well
    println!("\n=== Manual Record Handling ===");

    let process_message_manual = interface.func("process-message").unwrap();

    let mut results = vec![Value::Bool(false)]; // dummy value to be overwritten
    process_message_manual.call(
        &mut store,
        &[Value::String("Another message".into())],
        &mut results
    ).unwrap();

    if let Value::Record(record) = &results[0] {
        let id_field = record.field("id").unwrap();
        let reply_field = record.field("reply").unwrap();

        let id = if let Value::U32(x) = id_field { x } else { panic!("id field is not U32") };
        let reply = if let Value::String(s) = reply_field { s.to_string() } else { panic!("reply field is not String") };

        println!("[Host] Manual record handling:");
        println!("  └ id: {}", id);
        println!("  └ reply: '{}'", reply);
    }
}