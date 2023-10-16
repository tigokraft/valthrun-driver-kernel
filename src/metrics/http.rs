use alloc::{
    string::{
        String,
        ToString,
    },
    vec::Vec,
};

use embedded_io::{
    Read,
    ReadExactError,
    Write,
};
use httparse::Status;
use obfstr::obfstr;
use winapi::shared::ntdef::UNICODE_STRING;

use crate::{
    io::BufReader,
    kapi::UnicodeStringEx,
    metrics::HttpError,
    util::TcpConnection,
    wsk::{
        sys::{
            AF_INET,
            AF_INET6,
            SOCKADDR_INET,
        },
        SocketAddrInetEx,
        WskInstance,
    },
};

const METRICS_DEFAULT_PORT: u16 = 80;
fn resolve_metrics_target(wsk: &WskInstance) -> Result<(String, SOCKADDR_INET), HttpError> {
    let target_host = if let Some(override_value) = option_env!("METRICS_HOST") {
        String::from(override_value)
            .encode_utf16()
            .collect::<Vec<_>>()
    } else {
        obfstr::wide!("metrics.valth.run")
            .iter()
            .cloned()
            .collect::<Vec<_>>()
    };
    let utarget_domain = UNICODE_STRING::from_bytes_unchecked(&target_host);

    let target_address = wsk
        .get_address_info(Some(&utarget_domain), None)
        .map_err(HttpError::DnsLookupFailure)?
        .iterate_results()
        .filter(|address| {
            address.ai_family == AF_INET as i32 || address.ai_family == AF_INET6 as i32
        })
        .next()
        .ok_or(HttpError::DnsNoResults)?
        .clone();

    let mut inet_addr = unsafe { *(target_address.ai_addr as *mut SOCKADDR_INET).clone() };
    if let Some(port) = option_env!("METRICS_PORT") {
        *inet_addr.port_mut() = match port.parse::<u16>() {
            Ok(port) => port.swap_bytes(),
            Err(_) => {
                log::warn!(
                    "{}",
                    obfstr!("Failed to parse custom metrics port. Using default port.")
                );
                METRICS_DEFAULT_PORT.swap_bytes()
            }
        };
    } else {
        *inet_addr.port_mut() = METRICS_DEFAULT_PORT.swap_bytes();
    }

    log::trace!(
        "{}: {}",
        obfstr!("Successfully resolved metrics target to"),
        inet_addr.to_string()
    );
    Ok((String::from_utf16_lossy(&target_host), inet_addr))
}

pub struct HttpResponse {
    pub headers: Vec<(String, String)>,
    pub status_code: u16,
    pub content: Vec<u8>,
}

pub fn send_report(wsk: &WskInstance, target: &str, raw_data: &str) -> Result<HttpResponse, HttpError> {
    let (server_host, server_address) = resolve_metrics_target(wsk)?;
    let mut connection = match TcpConnection::connect(wsk, &server_address) {
        Ok(connection) => connection,
        Err(err) => {
            log::trace!("{}: {:#}", obfstr!("Failed to connect"), err);
            return Err(HttpError::ConnectError(err));
        }
    };

    let mut buffer = Vec::with_capacity(500);
    write!(&mut buffer, "POST {} HTTP/1.1\r\n", target).map_err(HttpError::WriteFmtError)?;
    write!(
        &mut buffer,
        "User-Agent: Valthrun/Kernel v{}\r\n",
        env!("CARGO_PKG_VERSION")
    )
    .map_err(HttpError::WriteFmtError)?;
    write!(&mut buffer, "Connection: Close\r\n").map_err(HttpError::WriteFmtError)?;
    write!(&mut buffer, "Host: {}\r\n", server_host).map_err(HttpError::WriteFmtError)?;
    write!(&mut buffer, "Content-Type: application/json\r\n").map_err(HttpError::WriteFmtError)?;
    write!(&mut buffer, "Content-Length: {}\r\n", raw_data.len())
        .map_err(HttpError::WriteFmtError)?;
    write!(&mut buffer, "\r\n").map_err(HttpError::WriteFmtError)?;

    if let Err(error) = connection.write_all(buffer.as_slice()) {
        log::trace!("{}: {:#}", obfstr!("Failed to send headers"), error);
        return Err(HttpError::IoError(error));
    }
    if let Err(error) = connection.write_all(raw_data.as_bytes()) {
        log::trace!("{}: {:#}", obfstr!("Failed to send payload"), error);
        return Err(HttpError::IoError(error));
    }

    let mut reader = BufReader::new(connection);
    let mut response_header_buffer = Vec::with_capacity(512);
    loop {
        let mut current_byte = 0u8;
        match reader.read(core::slice::from_mut(&mut current_byte)) {
            Ok(0) => {
                log::trace!("{}", obfstr!("EOF reading HTTP response header"));
                return Err(HttpError::EOF);
            }
            Ok(_) => {}
            Err(err) => {
                log::trace!("{}: {}", obfstr!("reading HTTP response header"), err);
                return Err(HttpError::IoError(err));
            }
        }

        response_header_buffer.push(current_byte);
        if current_byte == 0x0A {
            /* carrige return -> check for complete http header */
            if response_header_buffer.len() < 4 {
                /* require more bytes */
                continue;
            }

            if response_header_buffer[response_header_buffer.len() - 4..]
                == [0x0D, 0x0A, 0x0D, 0x0A]
            {
                /* end of HTTP header */
                break;
            }
        }

        if response_header_buffer.len() > 4096 {
            return Err(HttpError::ResponseHeadersTooLong);
        }
    }

    // log::debug!("Response headers: {}", String::from_utf8_lossy(&response_header_buffer));

    let mut response_headers = [httparse::EMPTY_HEADER; 64];
    let mut response_header = httparse::Response::new(&mut response_headers);
    let response_header_count = match response_header.parse(&response_header_buffer) {
        Ok(Status::Complete(count)) => count,
        Ok(Status::Partial) => return Err(HttpError::ResponseHeadersUncomplete),
        Err(err) => return Err(HttpError::ResponseHeadersInvalid(err)),
    };

    let content_length = if let Some(header) = {
        response_header
            .headers
            .iter()
            .find(|header| header.name.to_lowercase() == "content-length")
    } {
        String::from_utf8_lossy(header.value)
            .parse::<usize>()
            .map_err(|_| HttpError::ResponseHeaderInvalid("Content-Length".to_string()))?
    } else {
        0
    };

    if content_length > 5 * 1024 * 1024 {
        /* too much data :) */
        return Err(HttpError::ResponseHeaderInvalid(
            "Content-Length".to_string(),
        ));
    }

    let mut content = Vec::with_capacity(content_length);
    unsafe { content.set_len(content_length) };
    if let Err(error) = reader.read_exact(&mut content) {
        match error {
            ReadExactError::UnexpectedEof => return Err(HttpError::EOF),
            ReadExactError::Other(err) => return Err(HttpError::IoError(err)),
        }
    }

    let mut response = HttpResponse {
        content,
        status_code: response_header.code.unwrap_or_default(),
        headers: Default::default()
    };

    for header in response_header.headers {
        if header.name.is_empty() {
            continue;
        }

        response.headers.push((
            header.name.to_string(),
            String::from_utf8_lossy(header.value).to_string(),
        ));
    }

    log::debug!("Request succeeded -> {}", response_header_count);
    log::debug!("Response content length: {}", content_length);
    log::debug!("{}", String::from_utf8_lossy(response.content.as_slice()));
    Ok(response)
}
