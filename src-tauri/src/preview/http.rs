use std::{
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream},
    time::Duration,
};

const PREVIEW_UPSTREAM_CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const PREVIEW_UPSTREAM_IO_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_PREVIEW_UPSTREAM_RESPONSE_BYTES: u64 = 24 * 1024 * 1024;
const MAX_PREVIEW_UPSTREAM_HEADER_BYTES: usize = 64 * 1024;
const MAX_PREVIEW_UPSTREAM_PATH_BYTES: usize = 16 * 1024;

pub struct HttpResponse {
    pub raw: Vec<u8>,
    pub status_line: String,
    body_offset: usize,
}

impl HttpResponse {
    pub fn into_body(self) -> Vec<u8> {
        let HttpResponse {
            mut raw,
            body_offset,
            ..
        } = self;
        raw.drain(..body_offset);
        raw
    }
}

pub fn read_http_document(url: &str) -> Result<String, String> {
    let response = send_local_http_request(url)?;

    if !response.status_line.contains(" 200 ") {
        return Err(format!(
            "Preview-ul local a raspuns cu un status invalid: {}",
            response.status_line
        ));
    }

    String::from_utf8(response.into_body()).map_err(|error| {
        format!(
            "Documentul randat de preview nu este UTF-8 valid: {}",
            error
        )
    })
}

pub fn parse_local_http_url(url: &str) -> Result<(String, u16, String), String> {
    let trimmed = url.trim();
    let without_scheme = trimmed
        .strip_prefix("http://")
        .ok_or_else(|| "Preview-ul accepta doar URL-uri http:// locale.".to_string())?;
    let slash_index = without_scheme.find('/').unwrap_or(without_scheme.len());
    let authority = &without_scheme[..slash_index];
    let raw_path = &without_scheme[slash_index..];
    let (host, port) = authority
        .rsplit_once(':')
        .ok_or_else(|| "URL-ul preview-ului nu include port.".to_string())?;

    if host != "127.0.0.1" && host != "localhost" {
        return Err("Sunt permise doar preview-uri locale pe 127.0.0.1 sau localhost.".to_string());
    }

    let port = port
        .parse::<u16>()
        .map_err(|error| format!("Port preview invalid: {}", error))?;
    let path = if raw_path.is_empty() { "/" } else { raw_path };

    Ok((host.to_string(), port, path.to_string()))
}

pub fn send_local_http_request(url: &str) -> Result<HttpResponse, String> {
    let (host, port, path) = parse_local_http_url(url)?;
    if path.len() > MAX_PREVIEW_UPSTREAM_PATH_BYTES || path.contains('\r') || path.contains('\n') {
        return Err(format!(
            "Path-ul preview depășește contractul bounded de {} bytes sau conține delimitatori HTTP.",
            MAX_PREVIEW_UPSTREAM_PATH_BYTES
        ));
    }
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let mut stream = TcpStream::connect_timeout(&address, PREVIEW_UPSTREAM_CONNECT_TIMEOUT)
        .map_err(|error| {
            format!(
                "Nu am putut conecta la preview-ul local {}:{}: {}",
                host, port, error
            )
        })?;
    stream
        .set_read_timeout(Some(PREVIEW_UPSTREAM_IO_TIMEOUT))
        .map_err(|error| format!("Nu am putut seta timeout-ul de citire preview: {error}"))?;
    stream
        .set_write_timeout(Some(PREVIEW_UPSTREAM_IO_TIMEOUT))
        .map_err(|error| format!("Nu am putut seta timeout-ul de scriere preview: {error}"))?;
    let request = format!(
        "GET {} HTTP/1.0\r\nHost: {}:{}\r\nConnection: close\r\n\r\n",
        path, host, port
    );

    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("Nu am putut cere documentul de preview: {}", error))?;

    let raw = read_bounded_http_response(&mut stream)?;
    parse_http_response(raw)
}

fn parse_http_response(raw: Vec<u8>) -> Result<HttpResponse, String> {
    let header_end = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
        .ok_or_else(|| "Raspuns HTTP invalid de la preview.".to_string())?;
    if header_end > MAX_PREVIEW_UPSTREAM_HEADER_BYTES {
        return Err(format!(
            "Headerul răspunsului preview depășește limita de {} bytes.",
            MAX_PREVIEW_UPSTREAM_HEADER_BYTES
        ));
    }

    let header_text = String::from_utf8_lossy(&raw[..header_end]).to_string();
    let status_line = header_text.lines().next().unwrap_or_default().to_string();
    Ok(HttpResponse {
        raw,
        status_line,
        body_offset: header_end,
    })
}

fn read_bounded_http_response(reader: &mut impl Read) -> Result<Vec<u8>, String> {
    let mut raw = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut header_complete = false;

    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("Nu am putut citi raspunsul preview-ului: {error}"))?;
        if read == 0 {
            break;
        }
        raw.extend_from_slice(&buffer[..read]);

        if raw.len() as u64 > MAX_PREVIEW_UPSTREAM_RESPONSE_BYTES {
            return Err(format!(
                "Răspunsul preview depășește limita de {} bytes.",
                MAX_PREVIEW_UPSTREAM_RESPONSE_BYTES
            ));
        }

        if !header_complete {
            match raw.windows(4).position(|window| window == b"\r\n\r\n") {
                Some(index) => {
                    let header_end = index + 4;
                    if header_end > MAX_PREVIEW_UPSTREAM_HEADER_BYTES {
                        return Err(format!(
                            "Headerul răspunsului preview depășește limita de {} bytes.",
                            MAX_PREVIEW_UPSTREAM_HEADER_BYTES
                        ));
                    }
                    header_complete = true;
                }
                None if raw.len() > MAX_PREVIEW_UPSTREAM_HEADER_BYTES => {
                    return Err(format!(
                        "Headerul răspunsului preview depășește limita de {} bytes.",
                        MAX_PREVIEW_UPSTREAM_HEADER_BYTES
                    ));
                }
                None => {}
            }
        }
    }

    Ok(raw)
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{
        parse_http_response, parse_local_http_url, read_bounded_http_response,
        send_local_http_request, MAX_PREVIEW_UPSTREAM_HEADER_BYTES,
    };

    #[test]
    fn local_http_url_rejects_non_loopback_and_http_delimiters() {
        assert!(parse_local_http_url("https://127.0.0.1:1111/").is_err());
        assert!(parse_local_http_url("http://example.test:1111/").is_err());
        assert!(send_local_http_request("http://127.0.0.1:1/ok\r\nInjected: yes").is_err());
    }

    #[test]
    fn oversized_upstream_header_is_rejected() {
        let response = vec![b'a'; MAX_PREVIEW_UPSTREAM_HEADER_BYTES + 1];
        let error = read_bounded_http_response(&mut Cursor::new(response))
            .err()
            .expect("oversized header must fail");
        assert!(error.contains("Headerul răspunsului"), "{error}");
    }

    #[test]
    fn parsed_response_keeps_a_single_raw_allocation_and_extracts_body_on_consumption() {
        let raw = b"HTTP/1.0 200 OK\r\nContent-Type: text/plain\r\n\r\nbody".to_vec();
        let response = parse_http_response(raw).unwrap();
        assert_eq!(response.into_body(), b"body");
    }
}
