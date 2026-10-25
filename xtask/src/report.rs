//! Collected errors and warnings, printed as `path:line: message`.

use std::fmt;

#[derive(Default)]
pub struct Report {
    pub errors: Vec<Finding>,
    pub warnings: Vec<Finding>,
}

pub struct Finding {
    pub path: String,
    pub line: Option<usize>,
    pub message: String,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.line {
            Some(line) => write!(f, "{}:{}: {}", self.path, line, self.message),
            None => write!(f, "{}: {}", self.path, self.message),
        }
    }
}

impl Report {
    pub fn error(&mut self, path: &str, line: Option<usize>, message: impl Into<String>) {
        self.errors.push(Finding {
            path: path.to_string(),
            line,
            message: message.into(),
        });
    }

    pub fn warning(&mut self, path: &str, line: Option<usize>, message: impl Into<String>) {
        self.warnings.push(Finding {
            path: path.to_string(),
            line,
            message: message.into(),
        });
    }

    /// Prints every finding; returns an error if there were any errors.
    pub fn finish(&self) -> Result<(), String> {
        for warning in &self.warnings {
            eprintln!("warning: {warning}");
        }
        for error in &self.errors {
            eprintln!("error: {error}");
        }
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(format!("{} error(s)", self.errors.len()))
        }
    }

    #[cfg(test)]
    pub fn messages(&self) -> Vec<String> {
        self.errors
            .iter()
            .map(|f| format!("error: {f}"))
            .chain(self.warnings.iter().map(|f| format!("warning: {f}")))
            .collect()
    }
}
