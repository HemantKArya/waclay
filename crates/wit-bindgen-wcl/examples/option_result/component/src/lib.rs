wit_bindgen::generate!({
    world: "guest",
    path: "wit/component.wit"
});

use exports::test::guest::run::*;

struct Component;

impl Guest for Component {
    fn start() {
        // Test option return
        let some_val = test::guest::host::result_option(true);
        test::guest::host::log(&format!("Option (Some): {:?}", some_val));
        
        let none_val = test::guest::host::result_option(false);
        test::guest::host::log(&format!("Option (None): {:?}", none_val));
        
        // Test result<T, E>
        let ok_val = test::guest::host::result_result(true);
        test::guest::host::log(&format!("Result<T,E> (Ok): {:?}", ok_val));
        
        let err_val = test::guest::host::result_result(false);
        test::guest::host::log(&format!("Result<T,E> (Err): {:?}", err_val));
        
        // Test result<T> (no error type)
        let ok_only = test::guest::host::result_result_ok(true);
        test::guest::host::log(&format!("Result<T> (Ok): {:?}", ok_only));
        
        let ok_only_err = test::guest::host::result_result_ok(false);
        test::guest::host::log(&format!("Result<T> (Err): {:?}", ok_only_err));
        
        // Test result<_, E> (no ok type)
        let err_only = test::guest::host::result_result_err(true);
        test::guest::host::log(&format!("Result<_,E> (Ok): {:?}", err_only));
        
        let err_only_val = test::guest::host::result_result_err(false);
        test::guest::host::log(&format!("Result<_,E> (Err): {:?}", err_only_val));
        
        // Test result (no types)
        let none_result = test::guest::host::result_result_none(true);
        test::guest::host::log(&format!("Result (Ok): {:?}", none_result));
        
        let none_result_err = test::guest::host::result_result_none(false);
        test::guest::host::log(&format!("Result (Err): {:?}", none_result_err));
    }
}

export!(Component);
