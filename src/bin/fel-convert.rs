//! Convert a single FEL or CEL expression supplied as text.
// Rust guideline compliant 2026-02-21

use fel_core::cel::{
    Language, MAX_SOURCE_BYTES, NativeOptions, convert, convert_literal, convert_native,
};
use std::error::Error;
use std::io::{self, Read, Write};
use std::process::ExitCode;

const USAGE: &str = "Usage: fel-convert {to-fel|to-cel} [--syntax [--literal] | --native [--bind name[:type]=$field]... [--snapshot]] [--] [EXPRESSION|-]
Read one expression from the argument or standard input. The default preserves editable syntax.
Use --native for ordinary target expressions. Binding types are string?, string, int, bool and list.";

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os()
        .skip(1)
        .map(|s| s.into_string().map_err(|_| "arguments must be UTF-8"));
    let from = match args.next().transpose()?.as_deref() {
        Some("to-cel") => Language::Fel,
        Some("to-fel") => Language::Cel,
        Some("-h" | "--help") => {
            println!("{USAGE}");
            return Ok(());
        }
        _ => return Err(USAGE.into()),
    };
    let mut options = NativeOptions::default();
    let mut mode = None;
    let mut literal = false;
    let mut has_native_option = false;
    let mut positional = false;
    let mut input = None;
    while let Some(arg) = args.next().transpose()? {
        match arg.as_str() {
            "--" if !positional => positional = true,
            "-h" | "--help" if !positional => {
                println!("{USAGE}");
                return Ok(());
            }
            "--syntax" | "--native" if !positional => {
                let native = arg == "--native";
                if mode
                    .replace(native)
                    .is_some_and(|previous| previous != native)
                {
                    return Err("choose one conversion mode".into());
                }
            }
            "--literal" if !positional => literal = true,
            "--snapshot" if !positional => {
                has_native_option = true;
                options = options.with_snapshot();
            }
            "--bind" if !positional => {
                let binding = args
                    .next()
                    .transpose()?
                    .ok_or("--bind requires name[:type]=$field")?;
                let (declaration, field) = binding
                    .split_once('=')
                    .ok_or("--bind requires name[:type]=$field")?;
                let (name, ty) = declaration
                    .split_once(':')
                    .unwrap_or((declaration, "string?"));
                options = options.with_binding(name, ty.parse()?, field);
                has_native_option = true;
            }
            unknown if !positional && unknown.starts_with("--") => {
                return Err(format!("unknown option {unknown}").into());
            }
            _ => {
                if input.replace(arg).is_some() {
                    return Err("expected one expression".into());
                }
            }
        }
    }
    let native = mode.unwrap_or(false);
    if native && literal {
        return Err("--literal requires syntax conversion".into());
    }
    if !native && has_native_option {
        return Err("--bind and --snapshot require --native".into());
    }
    let mut source = input.unwrap_or_else(|| "-".to_owned());
    if source == "-" {
        source.clear();
        io::stdin()
            .lock()
            .take(MAX_SOURCE_BYTES as u64 + 3)
            .read_to_string(&mut source)?;
        // The command appends a line ending to stdout. A full-size expression
        // must also work when the next converter reads it from a pipe.
        if source.len() > MAX_SOURCE_BYTES && source.ends_with('\n') {
            source.pop();
            if source.ends_with('\r') {
                source.pop();
            }
        }
    }
    let output = if native {
        convert_native(&source, from, &options)?
    } else if literal {
        convert_literal(&source, from)?
    } else {
        convert(&source, from)?
    };
    writeln!(io::stdout().lock(), "{output}")?;
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e)
            if e.downcast_ref::<io::Error>()
                .is_some_and(|e| e.kind() == io::ErrorKind::BrokenPipe) =>
        {
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("fel-convert: {e}");
            ExitCode::FAILURE
        }
    }
}
