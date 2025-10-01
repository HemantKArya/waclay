wit_bindgen::generate!({
    path: "wit/component.wit",
    exports: {
        "test:guest/message": Message
    }
});

struct Message;

impl exports::test::guest::message::Guest for Message {
    fn process_message(input: String) -> exports::test::guest::message::Response {
        // Process the input message and create a response
        let id = 42; // Some processing ID
        let reply = format!("Processed: {}", input.to_uppercase());

        exports::test::guest::message::Response {
            id,
            reply,
        }
    }
}