// Walks PNG chunks looking for tEXt, zTXt, and iTXt metadata. Fields are
// numbered in encounter order, same convention as jpeg.rs uses for EXIF
// fields, so `--dump` line numbers stay meaningful across formats.
//
// XMP packets travel in an iTXt chunk under the fixed keyword
// "XML:com.adobe.xmp" (see the XMP spec, part 3); that one is pulled out
// and parsed separately rather than dumped as a raw text field.

use crate::inflate;
use crate::xmp::{self, XmpData};

const XMP_KEYWORD: &str = "XML:com.adobe.xmp";

pub struct TextField {
    pub keyword: String,
    pub text: String,
    pub line: usize,
}

pub struct PngText {
    pub fields: Vec<TextField>,
    pub xmp: Option<XmpData>,
}

pub enum ParseError {
    NotAPng,
    Truncated,
}

const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

pub fn parse_png(data: &[u8]) -> Result<Option<PngText>, ParseError> {
    if data.len() < 8 || data[0..8] != SIGNATURE {
        return Err(ParseError::NotAPng);
    }

    let mut pos = 8usize;
    let mut fields = Vec::new();
    let mut xmp = None;
    let mut line = 0usize;

    while pos + 8 <= data.len() {
        let length = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let chunk_type = &data[pos + 4..pos + 8];
        let data_start = pos + 8;
        let data_end = match data_start.checked_add(length).and_then(|end| end.checked_add(4)) {
            Some(end_with_crc) if end_with_crc <= data.len() => data_start + length,
            _ => return Err(ParseError::Truncated),
        };
        let payload = &data[data_start..data_end];

        let decoded = match chunk_type {
            b"tEXt" => split_null(payload)
                .map(|(keyword, text)| (latin1_to_string(keyword), latin1_to_string(text))),
            b"zTXt" => split_null(payload).and_then(|(keyword, rest)| {
                if rest.first() != Some(&0) {
                    return None;
                }
                let decompressed = inflate::zlib_decompress(&rest[1..])?;
                Some((latin1_to_string(keyword), latin1_to_string(&decompressed)))
            }),
            b"iTXt" => parse_itxt(payload),
            b"IEND" => break,
            _ => None,
        };

        if let Some((keyword, text)) = decoded {
            if keyword == XMP_KEYWORD && xmp.is_none() {
                xmp = Some(xmp::parse(&text));
            } else {
                line += 1;
                fields.push(TextField { keyword, text, line });
            }
        }

        pos = data_end + 4; // skip the trailing CRC
    }

    if fields.is_empty() && xmp.is_none() {
        Ok(None)
    } else {
        Ok(Some(PngText { fields, xmp }))
    }
}

fn split_null(data: &[u8]) -> Option<(&[u8], &[u8])> {
    let idx = data.iter().position(|&b| b == 0)?;
    Some((&data[..idx], &data[idx + 1..]))
}

// tEXt/zTXt keywords and text are Latin-1, which maps to Unicode code
// points 0-255 one-to-one, so this can't fail the way UTF-8 decoding can.
fn latin1_to_string(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

fn parse_itxt(payload: &[u8]) -> Option<(String, String)> {
    let (keyword, rest) = split_null(payload)?;
    if rest.len() < 2 {
        return None;
    }
    let compression_flag = rest[0];
    let compression_method = rest[1];
    let rest = &rest[2..];
    let (_language_tag, rest) = split_null(rest)?;
    let (_translated_keyword, rest) = split_null(rest)?;

    let text = if compression_flag == 0 {
        String::from_utf8_lossy(rest).into_owned()
    } else {
        if compression_method != 0 {
            return None;
        }
        String::from_utf8_lossy(&inflate::zlib_decompress(rest)?).into_owned()
    };

    Some((latin1_to_string(keyword), text))
}
