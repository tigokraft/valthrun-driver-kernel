use alloc::{
    format,
    string::{
        String,
        ToString,
    },
    vec::Vec,
};
use core::fmt::Debug;

use embedded_io::{
    Read,
    ReadExactError,
    Write,
};
use embedded_tls::blocking::{
    Aes256GcmSha384,
    NoVerify,
    TlsConfig,
    TlsConnection,
    TlsContext,
};
use httparse::Status;
use obfstr::obfstr;

use crate::{
    io::BufReader,
    metrics::HttpError,
    util::{
        TcpConnection,
        Win32Rng,
    },
    wsk::{
        sys::SOCKADDR_INET,
        WskInstance,
    },
};

pub struct HttpRequest<'a> {
    pub method: &'a str,
    pub target: &'a str,
    pub headers: HttpHeaders,
    pub payload: &'a [u8],
}

impl<'a> HttpRequest<'a> {
    fn emit_headers<E: Into<HttpError> + embedded_io::Error>(
        &self,
        output: &mut dyn Write<Error = E>,
    ) -> Result<(), HttpError> {
        let default_user_agent = format!("Valthrun/Kernel v{}", env!("CARGO_PKG_VERSION"));
        let user_agent = self
            .headers
            .find_header("User-Agent")
            .map_or(&default_user_agent, |header| &header.value);

        let connection = self
            .headers
            .find_header("Connection")
            .map_or("Close", |header| &header.value);

        let mut buffer = Vec::with_capacity(500);
        write!(&mut buffer, "{} {} HTTP/1.1\r\n", self.method, self.target)?;
        write!(&mut buffer, "User-Agent: {}\r\n", user_agent)?;
        write!(&mut buffer, "Connection: {}\r\n", connection)?;
        write!(&mut buffer, "Content-Length: {}\r\n", self.payload.len())?;

        for header in self.headers.headers.iter() {
            write!(&mut buffer, "{}: {}\r\n", header.name, header.value)?;
        }

        write!(&mut buffer, "\r\n")?;

        //log::debug!("Request: {}", String::from_utf8_lossy(&buffer));
        output
            .write_all(buffer.as_slice())
            .map_err(|err| err.into())?;

        Ok(())
    }

    fn emit_payload<E: Into<HttpError> + embedded_io::Error>(
        &self,
        output: &mut dyn Write<Error = E>,
    ) -> Result<(), HttpError> {
        output.write_all(self.payload).map_err(|err| err.into())
    }
}

#[derive(Debug, Default, Clone)]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Default, Clone)]
pub struct HttpHeaders {
    pub headers: Vec<HttpHeader>,
}

impl HttpHeaders {
    pub fn new() -> Self {
        Self {
            ..Default::default()
        }
    }

    pub fn add_header(&mut self, name: impl Into<String>, value: impl Into<String>) -> &mut Self {
        self.headers.push(HttpHeader {
            name: name.into(),
            value: value.into(),
        });
        self
    }

    pub fn find_header(&self, name: &str) -> Option<&HttpHeader> {
        let name_lower = name.to_lowercase();
        self.headers
            .iter()
            .find(|header| header.name.to_lowercase() == name_lower)
    }
}

#[derive(Default)]
pub struct HttpResponse {
    pub headers: HttpHeaders,
    pub status_code: u16,
    pub content: Vec<u8>,
}

impl HttpResponse {
    fn read_headers<E: Into<HttpError> + embedded_io::Error + Debug>(
        &mut self,
        reader: &mut dyn Read<Error = E>,
    ) -> Result<(), HttpError> {
        let mut buffer = Vec::with_capacity(512);
        loop {
            let mut current_byte = 0u8;
            match reader.read(core::slice::from_mut(&mut current_byte)) {
                Ok(0) => {
                    log::trace!("{}", obfstr!("EOF reading HTTP response header"));
                    return Err(HttpError::EOF);
                }
                Ok(_) => {}
                Err(err) => {
                    log::trace!("{}: {:?}", obfstr!("reading HTTP response header"), err);
                    return Err(err.into());
                }
            }

            buffer.push(current_byte);
            if current_byte == 0x0A {
                /* carrige return -> check for complete http header */
                if buffer.len() < 4 {
                    /* require more bytes */
                    continue;
                }

                if buffer[buffer.len() - 4..] == [0x0D, 0x0A, 0x0D, 0x0A] {
                    /* end of HTTP header */
                    break;
                }
            }

            if buffer.len() > 4096 {
                return Err(HttpError::ResponseHeadersTooLong);
            }
        }

        // log::debug!("Response headers: {}", String::from_utf8_lossy(&response_header_buffer));

        let mut headers = [httparse::EMPTY_HEADER; 64];
        let mut header = httparse::Response::new(&mut headers);
        let _header_count = match header.parse(&buffer) {
            Ok(Status::Complete(count)) => count,
            Ok(Status::Partial) => return Err(HttpError::ResponseHeadersUncomplete),
            Err(err) => return Err(HttpError::ResponseHeadersInvalid(err)),
        };

        self.status_code = header.code.unwrap_or_default();
        for header in header.headers {
            if header.name.is_empty() {
                continue;
            }

            self.headers.add_header(
                header.name.to_string(),
                String::from_utf8_lossy(header.value).to_string(),
            );
        }
        Ok(())
    }

    fn read_payload<E: Into<HttpError> + embedded_io::Error>(
        &mut self,
        stream: &mut dyn Read<Error = E>,
    ) -> Result<(), HttpError> {
        let content_length = if let Some(header) = self.headers.find_header("Content-Length") {
            header
                .value
                .parse::<usize>()
                .map_err(|_| HttpError::ResponseHeaderInvalid("Content-Length".to_string()))?
        } else {
            0
        };

        if content_length == 0 {
            return Ok(());
        }

        if content_length > 5 * 1024 * 1024 {
            /* too much data :) */
            return Err(HttpError::ResponseHeaderInvalid(
                "Content-Length".to_string(),
            ));
        }

        self.content.reserve(content_length);
        unsafe { self.content.set_len(content_length) };
        if let Err(error) = stream.read_exact(&mut self.content) {
            match error {
                ReadExactError::UnexpectedEof => return Err(HttpError::EOF),
                ReadExactError::Other(err) => return Err(err.into()),
            }
        }

        Ok(())
    }
}

pub fn execute_https_request(
    wsk: &WskInstance,
    server_address: &SOCKADDR_INET,
    request: &HttpRequest,
) -> Result<HttpResponse, HttpError> {
    let connection = match TcpConnection::connect(wsk, server_address) {
        Ok(connection) => connection,
        Err(err) => {
            log::trace!("{}: {:#}", obfstr!("connect"), err);
            return Err(HttpError::ConnectError(err));
        }
    };

    let mut read_record_buffer = Vec::new();
    read_record_buffer.resize(16640, 0u8);

    /* set the write buffer a little under the max record size to avoid any issues with other implementations */
    let mut write_record_buffer = Vec::new();
    write_record_buffer.resize(16000, 0u8);

    let server_name = &request
        .headers
        .find_header("Host")
        .ok_or(HttpError::MissingHostHeader)?
        .value;

    let config = TlsConfig::new().with_server_name(server_name);

    let mut tls: TlsConnection<'_, TcpConnection, Aes256GcmSha384> = TlsConnection::new(
        connection,
        &mut read_record_buffer,
        &mut write_record_buffer,
    );

    tls.open::<_, NoVerify>(TlsContext::new(&config, &mut Win32Rng::new()))?;

    if let Err(error) = request.emit_headers(&mut tls) {
        log::trace!("{}: {:#}", obfstr!("send headers"), error);
        return Err(error);
    }
    if let Err(error) = request.emit_payload(&mut tls) {
        log::trace!("{}: {:#}", obfstr!("send payload"), error);
        return Err(error);
    }
    tls.flush()
        .inspect_err(|err| log::trace!("flush: {:?}", err))?;

    let mut reader = BufReader::new(tls);
    let mut response = HttpResponse::default();

    response
        .read_headers(&mut reader)
        .inspect_err(|err| log::trace!("read headers: {:#}", err))?;

    response
        .read_payload(&mut reader)
        .inspect_err(|err| log::trace!("read content: {:#}", err))?;

    // log::debug!("Request succeeded -> {}", response.status_code);
    // log::debug!("Response content length: {}", response.content.len());
    // log::debug!("{}", String::from_utf8_lossy(response.content.as_slice()));
    Ok(response)
}

pub fn execute_http_request(
    wsk: &WskInstance,
    server_address: &SOCKADDR_INET,
    request: &HttpRequest,
) -> Result<HttpResponse, HttpError> {
    let mut connection = match TcpConnection::connect(wsk, server_address) {
        Ok(connection) => connection,
        Err(err) => {
            log::trace!("{}: {:#}", obfstr!("connect"), err);
            return Err(HttpError::ConnectError(err));
        }
    };

    if let Err(error) = request.emit_headers(&mut connection) {
        log::trace!("{}: {:#}", obfstr!("send headers"), error);
        return Err(error);
    }
    if let Err(error) = request.emit_payload(&mut connection) {
        log::trace!("{}: {:#}", obfstr!("send payload"), error);
        return Err(error);
    }
    connection.flush()?;

    let mut reader = BufReader::new(connection);
    let mut response = HttpResponse::default();

    response.read_headers(&mut reader)?;
    response.read_payload(&mut reader)?;

    // log::debug!("Request succeeded -> {}", response.status_code);
    // log::debug!("Response content length: {}", response.content.len());
    // log::debug!("{}", String::from_utf8_lossy(response.content.as_slice()));
    Ok(response)
}
