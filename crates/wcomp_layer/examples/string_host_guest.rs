use wasm_component_layer::*;

// The bytes of the component.
const WASM: &[u8] = include_bytes!("string_host_guest/component.wasm");

pub fn main() {
    println!("=== String Host-Guest Communication Demo ===");

    // Create a new engine for instantiating a component.
    let engine = Engine::new(wasmi_runtime_layer::Engine::default());

    // Create a store for managing WASM data and any custom user-defined state.
    let mut store = Store::new(&engine, ());

    // Parse the component bytes and load its imports and exports.
    let component = Component::new(&engine, WASM).unwrap();

    // Create a linker that will be used to resolve the component's imports.
    let mut linker = Linker::default();

    // Create a host interface for the logger that the guest can call.
    let host_interface = linker
        .define_instance("test:guest/host-logger".try_into().unwrap())
        .unwrap();

    // Define the host function that the guest can call.
    host_interface
        .define_func(
            "host-log",
            Func::new(
                &mut store,
                FuncType::new([ValueType::String], []),
                move |_, params, _results| {
                    let message = match &params[0] {
                        Value::String(s) => s,
                        _ => panic!("Unexpected parameter type"),
                    };

                    println!("[Host Function Called] Received from guest: '{}'", message);
                    Ok(())
                },
            ),
        )
        .unwrap();

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
        .typed::<String, String>()
        .unwrap();

    // Call the guest function with "hello" - this will trigger the guest to call back to host
    let result = process_message
        .call(&mut store, "hello".to_string())
        .unwrap();

    println!("[Host] Guest returned: '{}'", result);
}
