//! Enrich fuzz regression expressions with parse/eval oracles (JSONL on stdout).
//!
//! Input: one JSON object per line with at least `id` and `expression`.
//! Output: same object plus `mustParse` and optional `displayOracle`.

use std::io::{self, BufRead, Write};

use fel_core::{MapEnvironment, evaluate, parse};
use serde_json::{Map, Value};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for (line_no, line) in stdin.lock().lines().enumerate() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let mut obj: Map<String, Value> =
            serde_json::from_str(&line).map_err(|e| format!("line {}: {e}", line_no + 1))?;
        let expression = obj
            .get("expression")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("line {}: missing expression", line_no + 1))?
            .to_string();
        match parse(&expression) {
            Ok(expr) => {
                obj.insert("mustParse".to_string(), Value::Bool(true));
                let result = evaluate(&expr, &MapEnvironment::new());
                obj.insert(
                    "displayOracle".to_string(),
                    Value::String(format!("{}", result.value)),
                );
            }
            Err(_) => {
                obj.insert("mustParse".to_string(), Value::Bool(false));
                obj.remove("displayOracle");
            }
        }
        writeln!(stdout, "{}", serde_json::to_string(&obj)?)?;
    }
    Ok(())
}
