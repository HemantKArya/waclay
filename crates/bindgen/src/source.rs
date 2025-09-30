//! Utilities for generating Rust code from WIT types

use std::fmt;

/// A buffer for generating source code
#[derive(Default)]
pub struct Source {
    s: String,
}

impl Source {
    pub fn push_str(&mut self, s: &str) {
        self.s.push_str(s);
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.s)
    }
}

impl fmt::Write for Source {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push_str(s);
        Ok(())
    }
}
