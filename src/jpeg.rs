// Walks JPEG segments looking for an APP1/Exif block and an APP1/XMP
// block, then decodes the handful of IFD0 tags the linter currently
// understands plus the XMP properties in xmp.rs. Each decoded EXIF tag
// is numbered in the order it appears in the IFD so that findings can
// point back at it (see `--dump`).

use crate::xmp::XmpData;

const XMP_SIGNATURE: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";

pub struct Field {
    pub tag: u16,
    pub name: &'static str,
    pub value: String,
    pub line: usize,
}

pub struct ExifData {
    pub fields: Vec<Field>,
    pub has_gps: bool,
    pub xmp: Option<XmpData>,
}

pub enum ParseError {
    NotAJpeg,
    Truncated,
}

pub fn parse_jpeg(data: &[u8]) -> Result<Option<ExifData>, ParseError> {
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return Err(ParseError::NotAJpeg);
    }

    let mut pos = 2usize;
    let mut fields = Vec::new();
    let mut has_gps = false;
    let mut xmp = None;
    let mut found_exif = false;

    while pos + 2 <= data.len() {
        if data[pos] != 0xFF {
            pos += 1; // resync past stray fill bytes
            continue;
        }
        let marker = data[pos + 1];

        // Markers with no payload: SOI, TEM, RSTn.
        if marker == 0xD8 || marker == 0x01 || (0xD0..=0xD7).contains(&marker) {
            pos += 2;
            continue;
        }
        if marker == 0xD9 {
            break; // EOI
        }
        if pos + 4 > data.len() {
            return Err(ParseError::Truncated);
        }

        let seg_len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
        if seg_len < 2 || pos + 2 + seg_len > data.len() {
            return Err(ParseError::Truncated);
        }
        let payload = &data[pos + 4..pos + 2 + seg_len];

        if marker == 0xE1 && !found_exif && payload.len() >= 6 && &payload[0..6] == b"Exif\0\0" {
            found_exif = true;
            if let Some((f, gps)) = parse_exif(&payload[6..]) {
                fields = f;
                has_gps = gps;
            }
        } else if marker == 0xE1
            && xmp.is_none()
            && payload.len() >= XMP_SIGNATURE.len()
            && &payload[..XMP_SIGNATURE.len()] == XMP_SIGNATURE
        {
            let xml = String::from_utf8_lossy(&payload[XMP_SIGNATURE.len()..]);
            xmp = Some(crate::xmp::parse(&xml));
        }

        if marker == 0xDA {
            break; // start of scan: compressed data follows, nothing left to read
        }
        pos += 2 + seg_len;
    }

    if fields.is_empty() && xmp.is_none() {
        Ok(None)
    } else {
        Ok(Some(ExifData { fields, has_gps, xmp }))
    }
}

fn parse_exif(tiff: &[u8]) -> Option<(Vec<Field>, bool)> {
    if tiff.len() < 8 {
        return None;
    }
    let le = match &tiff[0..2] {
        b"II" => true,
        b"MM" => false,
        _ => return None,
    };
    if read_u16(&tiff[2..4], le) != 42 {
        return None;
    }

    let ifd0_offset = read_u32(&tiff[4..8], le) as usize;
    if ifd0_offset + 2 > tiff.len() {
        return None;
    }
    let entry_count = read_u16(&tiff[ifd0_offset..ifd0_offset + 2], le) as usize;

    let mut fields = Vec::new();
    let mut has_gps = false;
    let mut line = 0usize;

    for i in 0..entry_count {
        let entry_off = ifd0_offset + 2 + i * 12;
        if entry_off + 12 > tiff.len() {
            break;
        }
        let entry = &tiff[entry_off..entry_off + 12];
        let tag = read_u16(&entry[0..2], le);
        let typ = read_u16(&entry[2..4], le);
        let count = read_u32(&entry[4..8], le) as usize;
        let raw = &entry[8..12];

        if tag == 0x8825 {
            has_gps = true; // GPS IFD pointer: coordinates live elsewhere in the file
            continue;
        }

        let name = match tag {
            0x010E => "ImageDescription",
            0x010F => "Make",
            0x0110 => "Model",
            0x0112 => "Orientation",
            0x0131 => "Software",
            0x0132 => "DateTime",
            0x013B => "Artist",
            0x8298 => "Copyright",
            _ => continue,
        };

        let value = decode_value(tiff, typ, count, raw, le);
        line += 1;
        fields.push(Field { tag, name, value, line });
    }

    Some((fields, has_gps))
}

fn decode_value(tiff: &[u8], typ: u16, count: usize, raw: &[u8], le: bool) -> String {
    let elem_size = match typ {
        2 => 1, // ASCII
        3 => 2, // SHORT
        4 => 4, // LONG
        _ => return String::from("(unsupported type)"),
    };
    let total = elem_size * count;

    let bytes: Vec<u8> = if total <= 4 {
        raw.get(..total).unwrap_or(&[]).to_vec()
    } else {
        let offset = read_u32(raw, le) as usize;
        match tiff.get(offset..offset.saturating_add(total)) {
            Some(b) => b.to_vec(),
            None => return String::from("(out of range)"),
        }
    };

    match typ {
        2 => {
            let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
            String::from_utf8_lossy(bytes.get(..end).unwrap_or(&[])).trim().to_string()
        }
        3 => bytes
            .get(0..2)
            .map(|b| read_u16(b, le).to_string())
            .unwrap_or_default(),
        4 => bytes
            .get(0..4)
            .map(|b| read_u32(b, le).to_string())
            .unwrap_or_default(),
        _ => unreachable!(),
    }
}

fn read_u16(b: &[u8], le: bool) -> u16 {
    if le {
        u16::from_le_bytes([b[0], b[1]])
    } else {
        u16::from_be_bytes([b[0], b[1]])
    }
}

fn read_u32(b: &[u8], le: bool) -> u32 {
    if b.len() < 4 {
        return 0;
    }
    if le {
        u32::from_le_bytes([b[0], b[1], b[2], b[3]])
    } else {
        u32::from_be_bytes([b[0], b[1], b[2], b[3]])
    }
}
