wit_bindgen::generate!({
    path: "wit/calculator.wit",
    world: "calculator",
    exports: {
        "example:calculator/operations": Operations
    }
});

use exports::example::calculator::operations::*;

struct Operations;

// Static storage for calculation history
static mut HISTORY: Vec<CalcResult> = Vec::new();

impl Guest for Operations {
    fn calculate(op: Operation, a: f64, b: f64) -> Result<CalcResult, CalcError> {
        // Log the operation
        example::calculator::logger::log(
            example::calculator::logger::LogLevel::Info,
            &format!("Calculating: {:?}({}, {})", op, a, b),
        );
        
        let value = match op {
            Operation::Add => {
                let result = a + b;
                if result.is_infinite() {
                    return Err(CalcError::Overflow);
                }
                result
            }
            Operation::Subtract => {
                let result = a - b;
                if result.is_infinite() {
                    return Err(CalcError::Overflow);
                }
                result
            }
            Operation::Multiply => {
                let result = a * b;
                if result.is_infinite() {
                    return Err(CalcError::Overflow);
                }
                result
            }
            Operation::Divide => {
                if b == 0.0 {
                    example::calculator::logger::log(
                        example::calculator::logger::LogLevel::Error,
                        "Division by zero attempted",
                    );
                    return Err(CalcError::DivisionByZero);
                }
                let result = a / b;
                if result.is_infinite() || result.is_nan() {
                    return Err(CalcError::Overflow);
                }
                result
            }
        };
        
        let result = CalcResult {
            value,
            operation: op,
            success: true,
        };
        
        // Store in history (unsafe due to static mut)
        unsafe {
            HISTORY.push(result.clone());
        }
        
        example::calculator::logger::log(
            example::calculator::logger::LogLevel::Info,
            &format!("Result: {}", value),
        );
        
        Ok(result)
    }
    
    fn get_history() -> Vec<CalcResult> {
        unsafe { HISTORY.clone() }
    }
}
