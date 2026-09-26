mod inflate;
mod jpeg;
mod lint;
mod png;
mod xmp;

use std::env;
use std::fs;
use std::process::ExitCode;

const PNG_SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

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

    if data.len() >= 8 && data[0..8] == PNG_SIGNATURE {
        return run_png(&path, &data, dump);
    }
    run_jpeg(&path, &data, dump)
}

fn run_jpeg(path: &str, data: &[u8], dump: bool) -> ExitCode {
    let exif = match jpeg::parse_jpeg(data) {
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

    report(path, &lint::check(&exif))
}

fn run_png(path: &str, data: &[u8], dump: bool) -> ExitCode {
    let text = match png::parse_png(data) {
        Ok(Some(text)) => text,
        Ok(None) => {
            println!("{}: no text metadata found", path);
            return ExitCode::SUCCESS;
        }
        Err(png::ParseError::NotAPng) => {
            eprintln!("{}: not a PNG file", path);
            return ExitCode::FAILURE;
        }
        Err(png::ParseError::Truncated) => {
            eprintln!("{}: truncated or malformed PNG chunk", path);
            return ExitCode::FAILURE;
        }
    };

    if dump {
        for field in &text.fields {
            println!("{:4} {}: {}", field.line, field.keyword, field.text);
        }
        println!();
    }

    report(path, &lint::check_png(&text))
}

fn report(path: &str, findings: &[lint::Finding]) -> ExitCode {
    if findings.is_empty() {
        println!("{}: no issues found", path);
        return ExitCode::SUCCESS;
    }

    let mut had_error = false;
    for finding in findings {
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
    eprintln!("usage: imgmeta-lint [--dump] <file.jpg|file.png>");
}
