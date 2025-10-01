wit_bindgen::generate!({
    path: "wit/component.wit",
    exports: {
        "test:guest/exports": Exports
    }
});

struct Exports;

impl exports::test::guest::exports::Guest for Exports {
    fn get_status() -> exports::test::guest::exports::Status {
        exports::test::guest::exports::Status::Completed(Ok("Task completed successfully".to_string()))
    }
}