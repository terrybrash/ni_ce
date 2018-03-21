use std::io::Read;
use url::{self, Url};
use failure::Error;
use base64;
use std::fmt::{self, Display, Formatter};
use serde::ser::Serialize;
use status::StatusCode;

use reqwest;
use tungstenite;

pub type Headers = Vec<Header>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    pub name: String,
    pub values: Vec<String>,
}

impl Header {
    pub fn new<N, V>(name: N, value: V) -> Self
    where
        N: Into<String>,
        V: Into<String>, {
        Header {
            name: name.into(),
            values: vec![value.into()],
        }
    }

    pub fn from_vec<N, V>(name: N, values: Vec<V>) -> Self
    where
        N: Into<String>,
        V: Into<String>, {
        Header {
            name: name.into(),
            values: values.into_iter().map(Into::into).collect(),
        }
    }
}

impl Display for Header {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "{}: {}", self.name, self.values.as_slice().join("; "))
    }
}

/// This is a useful abstraction over the blob of bytes that eventually gets
/// sent out to HTTP calls. It allows passing around the payload as a string
/// for as long as possible, which is useful in debugging or intermediary
/// steps that need to work with *strings* and not just binary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Payload {
    Text(String),
    Binary(Vec<u8>),
}

impl Payload {
    pub fn as_bytes(&self) -> &[u8] {
        match *self {
            Payload::Text(ref payload) => payload.as_bytes(),
            Payload::Binary(ref payload) => payload.as_slice(),
        }
    }
}

impl Display for Payload {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        match *self {
            Payload::Text(ref body) => write!(f, "{}", body),
            Payload::Binary(ref body) => write!(f, "(binary) {}", base64::encode(body)),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Query {
    pub params: Vec<(String, String)>,
}

impl Query {
    pub fn with_capacity(capacity: usize) -> Self {
        Query {
            params: Vec::with_capacity(capacity),
        }
    }

    pub fn append_param<K, V>(&mut self, key: K, value: V)
    where
        K: Into<String>,
        V: Into<String>, {
        self.params.push((key.into(), value.into()));
    }

    pub fn to_string(&self) -> String {
        if self.params.is_empty() {
            String::new()
        } else {
            self.params
                .iter()
                .map(|&(ref name, ref value)| [name.as_str(), "=", value.as_str()])
                .collect::<Vec<[&str; 3]>>()
                .join(&"&")
                .into_iter()
                .collect()
        }
    }
}

/// Specifies that a request first needs to be authenticated before becoming a valid request.
pub trait NeedsAuthentication<C>: Serialize + Sized + fmt::Debug
where C: fmt::Debug {
    fn authenticate(self, credential: C) -> PrivateRequest<Self, C> {
        PrivateRequest {
            credential,
            request: self,
        }
    }
}

/// Wrapper for requests that require authentication.
#[derive(Debug)]
pub struct PrivateRequest<R, C>
where
    R: Serialize + fmt::Debug,
    C: fmt::Debug, {
    pub request: R,
    pub credential: C,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Method {
    Get,
    Head,
    Post,
    Put,
    Delete,
    Patch,
    Options,
    Trace,
    Connect,
    Extension(String),
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Method::Get => write!(f, "GET"),
            Method::Head => write!(f, "HEAD"),
            Method::Post => write!(f, "POST"),
            Method::Put => write!(f, "PUT"),
            Method::Delete => write!(f, "DELETE"),
            Method::Patch => write!(f, "PATCH"),
            Method::Options => write!(f, "OPTIONS"),
            Method::Trace => write!(f, "TRACE"),
            Method::Connect => write!(f, "CONNECT"),
            Method::Extension(ref method) => write!(f, "{}", method),
        }
    }
}

pub trait RestResource {
    type Response;

    fn method(&self) -> Method;

    fn path(&self) -> String;

    fn query(&self) -> Query {
        Query::default()
    }

    fn headers(&self) -> Result<Headers, Error> {
        Ok(Headers::new())
    }

    fn body(&self) -> Result<Option<Payload>, Error> {
        Ok(None)
    }

    fn deserialize(&self, response: &HttpResponse) -> Result<Self::Response, Error>;
}

pub trait WebsocketResource: fmt::Debug {
    type Message: fmt::Debug;
    type Error: fmt::Debug;

    fn method(&self) -> Method;

    fn path(&self) -> String;

    fn headers(&self) -> Headers {
        Headers::new()
    }

    fn serialize(message: Self::Message) -> Result<WebsocketMessage, Self::Error>;

    fn deserialize(message: WebsocketMessage) -> Result<Self::Message, Self::Error>;
}

pub enum WebsocketMessage {
    Text(String),
    Binary(Vec<u8>),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
}

pub trait HttpClient: 'static + Clone + Send + Sized + fmt::Debug {
    fn new() -> Self;
    fn send(&mut self, request: &HttpRequest) -> Result<HttpResponse, Error>;
}

impl From<Method> for reqwest::Method {
    fn from(method: Method) -> reqwest::Method {
        match method {
            Method::Get => reqwest::Method::Get,
            Method::Head => reqwest::Method::Head,
            Method::Post => reqwest::Method::Post,
            Method::Put => reqwest::Method::Put,
            Method::Delete => reqwest::Method::Delete,
            Method::Patch => reqwest::Method::Patch,
            Method::Options => reqwest::Method::Options,
            Method::Trace => reqwest::Method::Trace,
            Method::Connect => reqwest::Method::Connect,
            Method::Extension(m) => reqwest::Method::Extension(m),
        }
    }
}

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: StatusCode,
    pub body: Option<Payload>,
    pub headers: Headers,
}

impl From<reqwest::Response> for HttpResponse {
    fn from(mut response: reqwest::Response) -> Self {
        let mut body = Vec::with_capacity(1024);
        response.read_to_end(&mut body).unwrap();

        let body = if body.is_empty() {
            None
        } else {
            match String::from_utf8(body) {
                Ok(body) => Some(Payload::Text(body)),
                Err(body) => Some(Payload::Binary(body.into_bytes())),
            }
        };

        let headers = response
            .headers()
            .iter()
            .map(|header| Header::new(header.name(), header.value_string()))
            .collect();

        HttpResponse {
            status: StatusCode::try_from(response.status().as_u16()).unwrap(),
            headers,
            body,
        }
    }
}

impl Display for HttpResponse {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        writeln!(f, "{}", self.status)?;

        for header in &self.headers {
            writeln!(f, "{}", header)?;
        }

        if let Some(ref body) = self.body {
            writeln!(f, "Body: {}", body)?;
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct HttpRequest<'a> {
    pub method: Method,
    pub path: &'a str,
    pub host: &'a str,
    pub headers: Option<Headers>,
    pub body: Option<Payload>,
    pub query: Option<&'a str>,
}

impl<'a> HttpRequest<'a> {
    pub fn url(&self) -> Result<Url, url::ParseError> {
        match self.query {
            Some(query) => {
                Url::parse(self.host)?
                    .join(self.path)?
                    .join(format!("?{}", query.to_string()).as_str())
            }
            None => Url::parse(self.host)?.join(self.path),
        }
    }
}

impl<'a> Display for HttpRequest<'a> {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        writeln!(f, "{} {:?}", self.method, self.url())?;

        if let Some(query) = self.query {
            writeln!(f, "Query: {}", query.to_string())?;
        }

        if let Some(ref headers) = self.headers {
            for header in headers {
                writeln!(f, "{}", header)?;
            }
        }

        if let Some(ref body) = self.body {
            writeln!(f, "Body: {}", body)?;
        }

        Ok(())
    }
}

impl HttpClient for reqwest::Client {
    fn new() -> Self {
        reqwest::Client::new()
    }

    fn send(&mut self, request: &HttpRequest) -> Result<HttpResponse, Error> {
        let mut request_builder = self.request(
            request.method.clone().into(),
            request.url().map_err(|e| format_err!("{}", e))?,
        );

        if let Some(ref headers) = request.headers {
            let mut reqwest_headers = reqwest::header::Headers::new();
            for header in headers {
                reqwest_headers.set_raw(header.name.clone(), header.values.as_slice().join("; "));
            }
            request_builder.headers(reqwest_headers);
        }

        match request.body {
            Some(Payload::Text(ref body)) => {
                request_builder.body(body.clone());
            }
            Some(Payload::Binary(ref body)) => {
                request_builder.body(body.clone());
            }
            None => {}
        }

        let request = request_builder.build()?;
        let response = self.execute(request)?.into();

        Ok(response)
    }
}

pub struct TungsteniteClient<R>
where R: WebsocketResource {
    pub client: tungstenite::protocol::WebSocket<tungstenite::client::AutoStream>,
    pub _resource: ::std::marker::PhantomData<R>,
}

pub trait WebsocketClient<R>: Sized
where R: WebsocketResource {
    type Error;

    fn connect(url: Url, request: R) -> Result<Self, Self::Error>;
    fn recv(&mut self) -> Result<R::Message, Self::Error>;
    fn send(&mut self, message: R::Message) -> Result<(), Self::Error>;
}

impl<R> WebsocketClient<R> for TungsteniteClient<R>
where R: WebsocketResource
{
    type Error = tungstenite::error::Error;

    fn connect(url: Url, _request: R) -> Result<Self, tungstenite::error::Error> {
        use tungstenite::handshake::client::Request;

        let tungstenite_request = Request::from(url);
        // for header in request.headers() {
        //     match header.value {
        //         HeaderValue::String(value) =>
        //         tungstenite_request.add_header(Cow::from(header.name), Cow::from(value)),
        //         HeaderValue::Bytes(value) =>
        //         tungstenite_request.add_header(Cow::from(header.name), Cow::from(value)),
        //     }
        // }

        let (client, response) = tungstenite::connect(tungstenite_request).unwrap();
        if response.code != 101 {
            panic!(
                "[tungstenite] server returned {}: {:?}",
                response.code, response.headers
            );
        }

        Ok(TungsteniteClient {
            client,
            _resource: ::std::marker::PhantomData::default(),
        })
    }

    fn recv(&mut self) -> Result<R::Message, Self::Error> {
        let message = self.client.read_message()?;
        Ok(R::deserialize(message.into()).unwrap())
    }

    fn send(&mut self, message: R::Message) -> Result<(), Self::Error> {
        let message = R::serialize(message).unwrap();
        self.client.write_message(message.into())
    }
}

impl From<tungstenite::protocol::Message> for WebsocketMessage {
    fn from(message: tungstenite::protocol::Message) -> Self {
        match message {
            tungstenite::protocol::Message::Text(text) => WebsocketMessage::Text(text),
            tungstenite::protocol::Message::Binary(bytes) => WebsocketMessage::Binary(bytes),
            tungstenite::protocol::Message::Ping(bytes) => WebsocketMessage::Ping(bytes),
            tungstenite::protocol::Message::Pong(bytes) => WebsocketMessage::Pong(bytes),
        }
    }
}

impl From<WebsocketMessage> for tungstenite::protocol::Message {
    fn from(message: WebsocketMessage) -> Self {
        match message {
            WebsocketMessage::Text(text) => tungstenite::protocol::Message::Text(text),
            WebsocketMessage::Binary(bytes) => tungstenite::protocol::Message::Binary(bytes),
            WebsocketMessage::Ping(bytes) => tungstenite::protocol::Message::Ping(bytes),
            WebsocketMessage::Pong(bytes) => tungstenite::protocol::Message::Pong(bytes),
        }
    }
}
