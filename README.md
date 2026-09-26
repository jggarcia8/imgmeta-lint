# imgmeta-lint

Photos carry more than pixels. A JPEG straight off a phone usually embeds
GPS coordinates, the camera's make and model, and a capture timestamp in
its EXIF block. That's fine for a private photo library and a problem the
moment the same file gets attached to a support ticket, posted to a forum,
or uploaded to a site that doesn't strip metadata on the way in.

`imgmeta-lint` reads a JPEG's EXIF data or a PNG's text chunks and reports
anything worth a second look before the file goes out the door: embedded
location data, a missing or malformed capture date, an out-of-range
orientation value, absent copyright information. Findings are reported the
way a code linter reports them, with a location you can go look at.

## Usage

```
cargo run -- photo.jpg
```

```
photo.jpg:-: warning: file embeds GPS coordinates; strip before sharing publicly
photo.jpg:3: error: DateTime value '2024-13-40 99:99:99' does not match 'YYYY:MM:DD HH:MM:SS'
photo.jpg:-: info: no Copyright tag present
```

The number after the filename is the field's position in the metadata
listing, not a byte offset or a line in a text editor. Run with `--dump`
to see that listing directly:

```
cargo run -- --dump photo.jpg
```

```
   1 Make: Canon
   2 Model: Canon EOS 90D
   3 DateTime: 2024-13-40 99:99:99
   4 Orientation: 1

photo.jpg:-: warning: file embeds GPS coordinates; strip before sharing publicly
photo.jpg:3: error: DateTime value '2024-13-40 99:99:99' does not match 'YYYY:MM:DD HH:MM:SS'
photo.jpg:-: info: no Copyright tag present
```

A `-` location means the finding applies to the file as a whole rather than
to one field.

Exit code is non-zero if any finding is an error.

## Building

```
cargo build --release
```

No external crates. The parser and every lint rule are hand-rolled against
the JPEG and TIFF/EXIF specs, so the binary has no dependency tree to
audit.

## Current scope

- JPEG (no TIFF or HEIC yet), only IFD0: Make, Model, Orientation, Software,
  DateTime, ImageDescription, Artist, Copyright, plus a check for the
  presence of a GPS IFD.
- PNG tEXt, zTXt, and iTXt chunks, decompressed with a small built-in
  DEFLATE implementation. Only a Copyright presence check runs against
  them so far.
- XMP packets, in both formats: the APP1 block in JPEG and the
  `XML:com.adobe.xmp` iTXt chunk in PNG. Checked against the same GPS and
  date-quality rules as EXIF, using a small text scan rather than a full
  XML parser (`exif:GPSLatitude`/`GPSLongitude`, `exif:DateTimeOriginal`,
  `xmp:CreateDate`).
- No IPTC support yet, and no PNG eXIf chunk support (PNG can carry a
  full EXIF block of its own).

## License

MIT, see LICENSE.
