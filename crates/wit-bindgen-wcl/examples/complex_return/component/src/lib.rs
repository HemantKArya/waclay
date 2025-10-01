wit_bindgen::generate!({
    world: "guest",
    path: "wit/component.wit"
});

use exports::test::guest::exports::*;

struct Component;

impl Guest for Component {
    fn get_complex_data() -> ComplexData {
        ComplexData {
            id: 42,
            name: "complex-object".to_string(),
            values: vec![1.1, 2.2, 3.3, 4.4, 5.5],
            metadata: Some("metadata-value".to_string()),
            status: Ok("success".to_string()),
        }
    }
}

export!(Component);
