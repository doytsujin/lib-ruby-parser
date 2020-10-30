use regex::Regex;
use std::error::Error;
use std::fmt;

lazy_static! {
    static ref ENCODING_RE: Regex = Regex::new(
        r"(?x)
        [\s\#](en)?coding\s*[:=]\s*
        (
            # Special-case: there's a UTF8-MAC encoding.
            (?P<a>utf8-mac)
            |
            # Chew the suffix; it's there for emacs compat.
            (?P<b>[A-Za-z0-9_-]+?)(-unix|-dos|-mac)
            |
            (?P<c>[A-Za-z0-9_-]+)
        )
    "
    )
    .expect("ENCODING_RE regex is invalid");
}

#[derive(Debug)]
pub enum InputError {
    UnableToRecognizeEncoding,
    UnsupportdEncoding(String),
    UnknownEncoding,
    EncodingError(String),
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for InputError {}

fn recognize_encoding(source: &[u8]) -> Result<String, InputError> {
    if source.is_empty() {
        return Err(InputError::UnableToRecognizeEncoding);
    }

    let mut lines = source.split(|byte| *byte == b'\n');
    let first_line = lines.next().unwrap_or(&[] as &[u8]);
    let second_line = lines.next().unwrap_or(&[] as &[u8]);

    let encoding_line: &[u8];

    if first_line.starts_with(r"\xef\xbb\xbf".as_bytes()) {
        return Ok("utf-8".to_owned());
    } else if first_line.starts_with("#!".as_bytes()) {
        encoding_line = second_line;
    } else {
        encoding_line = first_line;
    }

    if !encoding_line.starts_with("#".as_bytes()) {
        return Err(InputError::UnableToRecognizeEncoding);
    }

    let encoding_line = String::from(String::from_utf8_lossy(encoding_line));

    let captures = ENCODING_RE
        .captures(&encoding_line)
        .ok_or(InputError::UnableToRecognizeEncoding)?;

    captures
        .name("a")
        .or_else(|| captures.name("b"))
        .or_else(|| captures.name("c"))
        .map(|m| m.as_str().to_owned())
        .ok_or(InputError::UnableToRecognizeEncoding)
}

fn decode(input: &[u8], enc: &str) -> Result<String, InputError> {
    let enc: encoding::EncodingRef = match &enc.to_uppercase()[..] {
        "ASCII-8BIT" | "BINARY" => {
            return Ok(String::from_utf8_lossy(input).into_owned());
        }
        "UTF-8" => encoding::all::UTF_8,
        "KOI8-R" => encoding::all::KOI8_R,
        _ => return Err(InputError::UnsupportdEncoding(enc.to_owned())),
    };

    enc.decode(input, encoding::DecoderTrap::Ignore)
        .map_err(|err| InputError::EncodingError(err.into_owned()))
}

pub fn decode_input(input: &[u8], enc: Option<String>) -> Result<(String, String), InputError> {
    if let Some(enc) = enc {
        return Ok((decode(input, &enc)?, enc));
    }

    let enc = recognize_encoding(input).unwrap_or_else(|_| "utf-8".to_owned());
    Ok((decode(input, &enc)?, enc))
}
