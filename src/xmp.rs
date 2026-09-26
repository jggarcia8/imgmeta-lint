// XMP packets are RDF/XML, but the handful of properties this linter
// cares about (GPS presence, a creation date) always show up either as
// an attribute (`exif:GPSLatitude="..."`) or a simple element
// (`<exif:GPSLatitude>...</exif:GPSLatitude>`) on an rdf:Description
// node. Pulling in a real XML parser for that would be a lot of surface
// area for two lookups, so this just scans the packet as text.

pub struct XmpData {
    pub has_gps: bool,
    pub create_date: Option<String>,
}

pub fn parse(xml: &str) -> XmpData {
    XmpData {
        has_gps: has_property(xml, "exif:GPSLatitude") || has_property(xml, "exif:GPSLongitude"),
        create_date: find_property(xml, "exif:DateTimeOriginal")
            .or_else(|| find_property(xml, "xmp:CreateDate")),
    }
}

fn has_property(xml: &str, name: &str) -> bool {
    xml.contains(&format!("{}=\"", name)) || xml.contains(&format!("<{}", name))
}

fn find_property(xml: &str, name: &str) -> Option<String> {
    find_attr(xml, name).or_else(|| find_element(xml, name))
}

fn find_attr(xml: &str, name: &str) -> Option<String> {
    let needle = format!("{}=\"", name);
    let start = xml.find(&needle)? + needle.len();
    let end = start + xml.get(start..)?.find('"')?;
    Some(xml[start..end].to_string())
}

fn find_element(xml: &str, name: &str) -> Option<String> {
    let open = format!("<{}>", name);
    let close = format!("</{}>", name);
    let start = xml.find(&open)? + open.len();
    let end = start + xml.get(start..)?.find(&close)?;
    Some(xml[start..end].trim().to_string())
}

/// Loosely validates an XMP date: `YYYY-MM-DD`, optionally extended with
/// `THH:MM:SS` and whatever timezone suffix follows it.
pub fn looks_like_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() < 10 {
        return false;
    }
    let date_ok = is_digits(&b[0..4])
        && b[4] == b'-'
        && is_digits(&b[5..7])
        && b[7] == b'-'
        && is_digits(&b[8..10]);
    if !date_ok {
        return false;
    }
    if b.len() == 10 {
        return true;
    }
    b.len() >= 19
        && b[10] == b'T'
        && is_digits(&b[11..13])
        && b[13] == b':'
        && is_digits(&b[14..16])
        && b[16] == b':'
        && is_digits(&b[17..19])
}

fn is_digits(b: &[u8]) -> bool {
    b.iter().all(|c| c.is_ascii_digit())
}
