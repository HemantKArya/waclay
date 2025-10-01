wit_bindgen::generate!({
    world: "guest",
    path: "wit/component.wit"
});

use exports::test::guest::run::*;
use test::guest::host::*;

struct Component;

impl Guest for Component {
    fn start() {
        // Test param-list
        param_list(&vec![-10, 20, -30, 40]);
        
        // Test param-record with variants
        param_record(Event::Open);
        param_record(Event::Close(12345));
        param_record(Event::Click(ClickType::Up));
        param_record(Event::Click(ClickType::Press(42)));
        param_record(Event::Click(ClickType::Down));
        
        // Test param-option
        param_option(Some(100));
        param_option(None);
        
        // Test result parameters
        param_result_all(Ok(10));
        param_result_all(Err(20));
        
        param_result_ok(Ok(30));
        param_result_ok(Err(()));
        
        param_result_err(Ok(()));
        param_result_err(Err(40));
        
        param_result_none(Ok(()));
        param_result_none(Err(()));
        
        // Test param-mult with multiple complex parameters
        param_mult(
            &vec!["hello".to_string(), "world".to_string()],
            Event::Click(ClickType::Press(99)),
            Some("optional-data"),
            Ok("success")
        );
    }
}

export!(Component);
