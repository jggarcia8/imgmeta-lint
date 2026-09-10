mod jpeg;
mod lint;

use std::env;
use std::fs;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    let mut dump = false;
    let mut path: Option<String> = None;

    for arg in &args[1..] {
        match arg.as_str() {
            "--dump" => dump = true,
            "-h" | "--help" => {
                print_usage();
                return ExitCode::SUCCESS;
            }
            other => path = Some(other.to_string()),
        }
    }

    let path = match path {
        Some(p) => p,
        None => {
            print_usage();
            return ExitCode::FAILURE;
        }
    };

    let data = match fs::read(&path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{}: {}", path, e);
            return ExitCode::FAILURE;
        }
    };

    let exif = match jpeg::parse_jpeg(&data) {
        Ok(Some(exif)) => exif,
        Ok(None) => {
            println!("{}: no EXIF metadata found", path);
            return ExitCode::SUCCESS;
        }
        Err(jpeg::ParseError::NotAJpeg) => {
            eprintln!("{}: not a JPEG file", path);
            return ExitCode::FAILURE;
        }
        Err(jpeg::ParseError::Truncated) => {
            eprintln!("{}: truncated or malformed JPEG segment", path);
            return ExitCode::FAILURE;
        }
    };

    if dump {
        for field in &exif.fields {
            println!("{:4} {}: {}", field.line, field.name, field.value);
        }
        println!();
    }

    let findings = lint::check(&exif);
    if findings.is_empty() {
        println!("{}: no issues found", path);
        return ExitCode::SUCCESS;
    }

    let mut had_error = false;
    for finding in &findings {
        if matches!(finding.severity, lint::Severity::Error) {
            had_error = true;
        }
        let loc = if finding.line == 0 {
            "-".to_string()
        } else {
            finding.line.to_string()
        };
        println!(
            "{}:{}: {}: {}",
            path,
            loc,
            finding.severity.label(),
            finding.message
        );
    }

    if had_error {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn print_usage() {
    eprintln!("usage: imgmeta-lint [--dump] <file.jpg>");
}
