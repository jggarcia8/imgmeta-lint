use crate::jpeg::ExifData;
use crate::png::PngText;

pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    pub fn label(&self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }
}

pub struct Finding {
    /// Field line number from the metadata dump, or 0 for file-level findings.
    pub line: usize,
    pub severity: Severity,
    pub message: String,
}

pub fn check(exif: &ExifData) -> Vec<Finding> {
    let mut findings = Vec::new();

    if exif.has_gps {
        findings.push(Finding {
            line: 0,
            severity: Severity::Warning,
            message: "file embeds GPS coordinates; strip before sharing publicly".to_string(),
        });
    }

    let mut has_datetime = false;
    let mut has_copyright = false;

    for field in &exif.fields {
        match field.tag {
            0x0132 => {
                has_datetime = true;
                if !looks_like_exif_datetime(&field.value) {
                    findings.push(Finding {
                        line: field.line,
                        severity: Severity::Error,
                        message: format!(
                            "DateTime value '{}' does not match 'YYYY:MM:DD HH:MM:SS'",
                            field.value
                        ),
                    });
                }
            }
            0x8298 => has_copyright = true,
            0x0112 => {
                if let Ok(v) = field.value.parse::<u32>() {
                    if v == 0 || v > 8 {
                        findings.push(Finding {
                            line: field.line,
                            severity: Severity::Error,
                            message: format!(
                                "Orientation value {} is out of the valid range 1-8",
                                v
                            ),
                        });
                    }
                }
            }
            _ => {}
        }
    }

    if !has_datetime {
        findings.push(Finding {
            line: 0,
            severity: Severity::Info,
            message: "no DateTime tag present".to_string(),
        });
    }
    if !has_copyright {
        findings.push(Finding {
            line: 0,
            severity: Severity::Info,
            message: "no Copyright tag present".to_string(),
        });
    }

    findings
}

pub fn check_png(text: &PngText) -> Vec<Finding> {
    let mut findings = Vec::new();

    let has_copyright = text
        .fields
        .iter()
        .any(|f| f.keyword.eq_ignore_ascii_case("copyright"));
    if !has_copyright {
        findings.push(Finding {
            line: 0,
            severity: Severity::Info,
            message: "no Copyright text chunk present".to_string(),
        });
    }

    findings
}

fn looks_like_exif_datetime(s: &str) -> bool {
    let bytes = s.as_bytes();
    if bytes.len() != 19 {
        return false;
    }
    let pattern = b"XXXX:XX:XX XX:XX:XX";
    for (i, &p) in pattern.iter().enumerate() {
        let c = bytes[i];
        if p == b'X' {
            if !c.is_ascii_digit() {
                return false;
            }
        } else if c != p {
            return false;
        }
    }
    true
}
