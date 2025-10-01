wit_bindgen::generate!({
    path: "wit/component.wit",
    exports: {
        "test:guest/message": Message
    }
});

struct Message;

use exports::test::guest::message::Guest;
use test::guest::host_logger::host_log;

impl Guest for Message {
    fn process_message(input: String) -> String {
        // Call the host function with "world"
        host_log("world");
        
        // Return a greeting combining the input and our response
        format!("{} world", input)
    }
}