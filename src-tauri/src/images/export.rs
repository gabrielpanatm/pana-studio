use image::codecs::webp::WebPEncoder;
use std::io::Cursor;

const MAX_DATA_URL_HEADER_BYTES: usize = 512;
const MAX_CANVAS_EDGE: u32 = 8_192;
const MAX_CANVAS_PIXELS: u64 = 33_554_432;
const MAX_CANVAS_DECODE_ALLOC_BYTES: u64 = 256 * 1024 * 1024;

fn maximum_base64_payload_bytes(max_decoded_bytes: usize) -> Result<usize, String> {
    max_decoded_bytes
        .checked_add(2)
        .and_then(|value| value.checked_div(3))
        .and_then(|value| value.checked_mul(4))
        .ok_or_else(|| "Limita base64 nu poate fi reprezentată pe această platformă.".to_string())
}

fn maximum_base64_input_bytes(max_decoded_bytes: usize) -> Result<usize, String> {
    maximum_base64_payload_bytes(max_decoded_bytes)?
        .checked_add(MAX_DATA_URL_HEADER_BYTES)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| "Limita data URL nu poate fi reprezentată pe această platformă.".to_string())
}

fn base64_value(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn decode_base64_payload(input: &str, max_decoded_bytes: usize) -> Result<Vec<u8>, String> {
    let maximum_payload = maximum_base64_payload_bytes(max_decoded_bytes)?;
    if input.len() > maximum_payload {
        return Err(format!(
            "Payload-ul base64 depășește limita de {} MiB decodați.",
            max_decoded_bytes / (1024 * 1024)
        ));
    }

    let bytes = input.as_bytes();
    if bytes.iter().any(|byte| byte.is_ascii_whitespace()) {
        return Err("Payload-ul base64 nu poate conține whitespace.".to_string());
    }
    if bytes.len() % 4 != 0 {
        return Err("Payload-ul base64 trebuie să aibă lungimea multiplu de 4.".to_string());
    }

    let padding = bytes.iter().rev().take_while(|byte| **byte == b'=').count();
    if padding > 2 {
        return Err("Padding-ul base64 poate conține cel mult două caractere '='.".to_string());
    }
    let data_length = bytes.len() - padding;
    if bytes[..data_length].contains(&b'=') {
        return Err("Padding-ul base64 este permis numai la finalul payload-ului.".to_string());
    }
    if bytes[..data_length]
        .iter()
        .any(|byte| base64_value(*byte).is_none())
    {
        return Err("Payload-ul base64 conține caractere invalide.".to_string());
    }

    if padding == 1 {
        let final_value = base64_value(bytes[data_length - 1])
            .ok_or_else(|| "Payload-ul base64 are padding invalid.".to_string())?;
        if final_value & 0b0000_0011 != 0 {
            return Err("Payload-ul base64 are biți de padding necanonici.".to_string());
        }
    } else if padding == 2 {
        let final_value = base64_value(bytes[data_length - 1])
            .ok_or_else(|| "Payload-ul base64 are padding invalid.".to_string())?;
        if final_value & 0b0000_1111 != 0 {
            return Err("Payload-ul base64 are biți de padding necanonici.".to_string());
        }
    }

    let decoded_length = bytes
        .len()
        .checked_div(4)
        .and_then(|quartets| quartets.checked_mul(3))
        .and_then(|length| length.checked_sub(padding))
        .ok_or_else(|| "Lungimea payload-ului base64 este invalidă.".to_string())?;
    if decoded_length > max_decoded_bytes {
        return Err(format!(
            "Payload-ul base64 depășește limita de {} MiB decodați.",
            max_decoded_bytes / (1024 * 1024)
        ));
    }

    // Alocarea se face numai după validarea structurii și a limitei exacte.
    let mut output = Vec::with_capacity(decoded_length);
    for quartet in bytes.chunks_exact(4) {
        let first = base64_value(quartet[0])
            .ok_or_else(|| "Payload-ul base64 conține caractere invalide.".to_string())?;
        let second = base64_value(quartet[1])
            .ok_or_else(|| "Payload-ul base64 conține caractere invalide.".to_string())?;
        output.push((first << 2) | (second >> 4));

        if quartet[2] != b'=' {
            let third = base64_value(quartet[2])
                .ok_or_else(|| "Payload-ul base64 conține caractere invalide.".to_string())?;
            output.push((second << 4) | (third >> 2));

            if quartet[3] != b'=' {
                let fourth = base64_value(quartet[3])
                    .ok_or_else(|| "Payload-ul base64 conține caractere invalide.".to_string())?;
                output.push((third << 6) | fourth);
            }
        }
    }

    debug_assert_eq!(output.len(), decoded_length);
    Ok(output)
}

fn is_mime_subtype_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric()
        || matches!(
            byte,
            b'!' | b'#'
                | b'$'
                | b'%'
                | b'&'
                | b'\''
                | b'*'
                | b'+'
                | b'-'
                | b'.'
                | b'^'
                | b'_'
                | b'`'
                | b'|'
                | b'~'
        )
}

fn validate_image_data_url_header(header: &str) -> Result<(), String> {
    let metadata = header
        .strip_prefix("data:")
        .ok_or_else(|| "Data URL invalid: lipsește prefixul canonical 'data:'.".to_string())?;
    let media_type = metadata.strip_suffix(";base64").ok_or_else(|| {
        "Data URL invalid: headerul trebuie să se termine cu markerul canonical ';base64'."
            .to_string()
    })?;
    if media_type.contains(';') {
        return Err(
            "Data URL invalid: parametrii media nu sunt acceptați în headerul canonical."
                .to_string(),
        );
    }
    let subtype = media_type.strip_prefix("image/").ok_or_else(|| {
        "Data URL invalid: este acceptat numai un media type din familia image/*.".to_string()
    })?;
    if subtype.is_empty() || !subtype.bytes().all(is_mime_subtype_token_byte) {
        return Err("Data URL invalid: subtype-ul media image/* nu este valid.".to_string());
    }
    Ok(())
}

pub fn decode_data_url_bounded(
    data_url: &str,
    max_decoded_bytes: usize,
) -> Result<Vec<u8>, String> {
    let maximum_input = maximum_base64_input_bytes(max_decoded_bytes)?;
    if data_url.len() > maximum_input {
        return Err(format!(
            "Data URL depășește limita de {} MiB decodați.",
            max_decoded_bytes / (1024 * 1024)
        ));
    }
    let (header, payload) = data_url
        .split_once(',')
        .ok_or_else(|| "Data URL invalid: lipsește separatorul de payload.".to_string())?;
    if header.len() > MAX_DATA_URL_HEADER_BYTES {
        return Err("Headerul data URL este prea mare.".to_string());
    }
    validate_image_data_url_header(header)?;
    decode_base64_payload(payload, max_decoded_bytes)
}

fn validate_canvas_dimensions(width: u32, height: u32) -> Result<(), String> {
    if width == 0 || height == 0 || width > MAX_CANVAS_EDGE || height > MAX_CANVAS_EDGE {
        return Err(format!(
            "Randarea canvasului depășește limita de {MAX_CANVAS_EDGE}px pe latură."
        ));
    }
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or_else(|| "Dimensiunile canvasului depășesc reprezentarea sigură.".to_string())?;
    if pixels > MAX_CANVAS_PIXELS {
        return Err(format!(
            "Randarea canvasului depășește limita de {MAX_CANVAS_PIXELS} pixeli."
        ));
    }
    pixels
        .checked_mul(4)
        .filter(|bytes| *bytes <= MAX_CANVAS_DECODE_ALLOC_BYTES)
        .ok_or_else(|| {
            "Randarea canvasului depășește bugetul de memorie pentru RGBA.".to_string()
        })?;
    Ok(())
}

pub fn encode_canvas_data_url_as_webp(
    data_url: &str,
    max_input_bytes: usize,
    max_output_bytes: usize,
) -> Result<Vec<u8>, String> {
    let bytes = decode_data_url_bounded(data_url, max_input_bytes)?;
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| format!("Nu am putut detecta randarea canvasului: {error}"))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_CANVAS_EDGE);
    limits.max_image_height = Some(MAX_CANVAS_EDGE);
    limits.max_alloc = Some(MAX_CANVAS_DECODE_ALLOC_BYTES);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|error| format!("Nu am putut decoda randarea canvasului: {}", error))?;
    validate_canvas_dimensions(image.width(), image.height())?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    let mut encoded = Vec::new();

    WebPEncoder::new_lossless(&mut encoded)
        .encode(rgba.as_raw(), width, height, image::ColorType::Rgba8.into())
        .map_err(|error| format!("Nu am putut encoda WebP în Rust: {}", error))?;

    if encoded.len() > max_output_bytes {
        return Err(format!(
            "WebP-ul rezultat depășește limita de {} MiB.",
            max_output_bytes / (1024 * 1024)
        ));
    }

    Ok(encoded)
}

#[cfg(test)]
mod tests {
    use super::{decode_data_url_bounded, maximum_base64_input_bytes, validate_canvas_dimensions};

    #[test]
    fn data_url_limit_is_checked_before_decode_allocation() {
        let max = 4usize;
        let input_limit = maximum_base64_input_bytes(max).unwrap();
        let oversized = format!("data:image/png;base64,{}", "A".repeat(input_limit));
        let error = decode_data_url_bounded(&oversized, max).unwrap_err();
        assert!(error.contains("depășește limita"));
    }

    #[test]
    fn data_url_decode_enforces_decoded_byte_limit() {
        let error = decode_data_url_bounded("data:image/png;base64,QUJDREVG", 4).unwrap_err();
        assert!(error.contains("depășește limita"));
        assert_eq!(
            decode_data_url_bounded("data:image/png;base64,QUJDRA==", 4).unwrap(),
            b"ABCD"
        );
    }

    #[test]
    fn data_url_decode_accepts_canonical_base64_variants() {
        assert_eq!(
            decode_data_url_bounded("data:image/png;base64,TWFu", 3).unwrap(),
            b"Man"
        );
        assert_eq!(
            decode_data_url_bounded("data:image/webp;base64,TWE=", 2).unwrap(),
            b"Ma"
        );
        assert_eq!(
            decode_data_url_bounded("data:image/svg+xml;base64,TQ==", 1).unwrap(),
            b"M"
        );
    }

    #[test]
    fn data_url_decode_rejects_data_after_padding() {
        assert!(decode_data_url_bounded("data:image/png;base64,TQ==AAAA", 16).is_err());
    }

    #[test]
    fn data_url_decode_rejects_interior_or_excessive_padding() {
        assert!(decode_data_url_bounded("data:image/png;base64,T=Q=", 16).is_err());
        assert!(decode_data_url_bounded("data:image/png;base64,T===", 16).is_err());
    }

    #[test]
    fn data_url_decode_rejects_invalid_length_and_whitespace() {
        assert!(decode_data_url_bounded("data:image/png;base64,TQ=", 16).is_err());
        assert!(decode_data_url_bounded("data:image/png;base64,T Q==", 16).is_err());
        assert!(decode_data_url_bounded("data:image/png;base64,TQ==\n", 16).is_err());
    }

    #[test]
    fn data_url_decode_requires_canonical_image_header() {
        assert!(decode_data_url_bounded("data:text/plain;base64,TQ==", 16).is_err());
        assert!(decode_data_url_bounded("data:image/png;base64evil,TQ==", 16).is_err());
        assert!(decode_data_url_bounded("data:image/png;charset=utf-8;base64,TQ==", 16).is_err());
    }

    #[test]
    fn data_url_decode_rejects_noncanonical_padding_bits() {
        assert!(decode_data_url_bounded("data:image/png;base64,TR==", 16).is_err());
        assert!(decode_data_url_bounded("data:image/png;base64,TWF=", 16).is_err());
    }

    #[test]
    fn canvas_dimensions_are_bounded_by_edge_and_pixel_count() {
        validate_canvas_dimensions(7_680, 4_320).unwrap();
        assert!(validate_canvas_dimensions(8_193, 1).is_err());
        assert!(validate_canvas_dimensions(8_192, 8_192).is_err());
        assert!(validate_canvas_dimensions(0, 10).is_err());
    }
}
