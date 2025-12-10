#[allow(warnings)]
mod bindings;

use bindings::exports::example::toplevel::Guest;
use bindings::example::toplevel::{log_message, multiply};

struct Component;

impl Guest for Component {
    fn add(a: f32, b: f32) -> f32 {
        log_message(&format!("Adding {} + {}", a, b));
        a + b
    }

    fn compute(x: f32, y: f32) -> f32 {
        log_message(&format!("Computing with {} and {}", x, y));
        // Use the host-provided multiply function
        let product = multiply(x, y);
        let sum = x + y;
        product + sum
    }
}

bindings::export!(Component with_types_in bindings);
