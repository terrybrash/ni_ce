use std::io::{Read};
use url::Url;
use failure::{Error};
use base64;
use std::fmt::{self, Display, Formatter};
use serde::ser::Serialize;
use status::StatusCode;

use reqwest;
use tungstenite;

// pub type Headers = HashMap<String, String>;
pub type Headers = Vec<Header>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Header {
    pub name: String,
    pub values: Vec<String>,
}

impl Header {
    pub fn new<N, V>(name: N, value: V) -> Self 
    where N: Into<String>, V: Into<String> {
        Header {
            name: name.into(),
            values: vec![value.into()],
        }
    }

    pub fn from_vec<N, V>(name: N, values: Vec<V>) -> Self 
    where N: Into<String>, V: Into<String> {
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
        match self {
            &Payload::Text(ref payload) => payload.as_bytes(),
            &Payload::Binary(ref payload) => payload.as_slice(),
        }
    }
}

impl Display for Payload {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        match self {
            &Payload::Text(ref body) => write!(f, "{}", body),
            &Payload::Binary(ref body) => write!(f, "(binary) {}", base64::encode(body)),
        }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Query {
    pub params: Vec<(String, String)>,
    char_len: usize,
}

impl Query {
    pub fn from_vec<K, V>(params: Vec<(K, V)>) -> Self 
    where K: Into<String>, V: Into<String> {
        // let char_len = params.iter().fold(0, |acc, &(ref name, ref value)| acc + name.len() + value.len());
        let char_len = 0;
        let params = params.into_iter().map(|(k, v)| (k.into(), v.into())).collect();        
        Query {
            params,
            char_len,
        }
    }

    pub fn to_string(&self) -> String {
        if self.params.is_empty() {
            String::new()
        } else {
            let params: Vec<String> = self.params.iter().map(|&(ref name, ref value)| [name.as_str(), value.as_str()].join("=")).collect();
            let query = ["?", &params.as_slice().join("&")].concat();
            query
        }
    }
}

#[derive(Debug, Default)]
pub struct QueryBuilder {
    params: Vec<(String, String)>,
}

impl QueryBuilder {
    pub fn with_capacity(len: usize) -> Self {
        QueryBuilder {
            params: Vec::with_capacity(len),
        }
    }

    pub fn param<K, V>(mut self, key: K, value: V) -> Self 
    where K: Into<String>, V: Into<String> {
        self.params.push((key.into(), value.into()));
        self
    }

    pub fn build(mut self) -> Query {
        Query::from_vec(self.params)
    }
}

/// Specifies that a request first needs to be authenticated before becoming a valid request.
pub trait NeedsAuthentication<C>: Serialize + Sized + fmt::Debug where C: fmt::Debug {
    fn authenticate(self, credential: C) -> PrivateRequest<Self, C> {
        PrivateRequest {
            credential: credential,
            request: self,
        }
    }
}

/// Wrapper for requests that require authentication.
#[derive(Debug)]
pub struct PrivateRequest<R, C>
where R: Serialize + fmt::Debug, C: fmt::Debug {
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
    Extension(String)
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

pub trait HttpClient: 'static + Clone + Send + fmt::Debug {
    fn new() -> Self;
    fn send<Request>(&mut self, url: &Url, request: Request) -> Result<Request::Response, Error> where Request: RestResource;
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
            Method::Extension(m) => reqwest::Method::Extension(m)
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

        let body = match body.is_empty() {
            true => None,
            false => match String::from_utf8(body) {
                Ok(body) => Some(Payload::Text(body)),
                Err(body) => Some(Payload::Binary(body.into_bytes())),
            },
        };

        let headers = response.headers().iter().map(|header| {
            Header::new(header.name(), header.value_string())
        }).collect();

        HttpResponse {
            status: StatusCode::try_from(response.status().as_u16()).unwrap(),
            headers: headers,
            body: body,
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

struct HttpRequest {
    pub host: Url,
    pub method: Method,
    pub body: Option<Payload>,
    pub headers: Headers,
    pub url: Url,
}

impl HttpRequest {
    pub fn new<Resource>(host: Url, resource: &Resource) -> Result<Self, Error> 
    where Resource: RestResource {
        let query = resource.query().to_string();
        let path = resource.path();
        let request = HttpRequest {
            url: host.join(&path)?.join(&query)?,
            host: host,
            method: resource.method(),
            body: resource.body()?,
            headers: resource.headers()?,
        };
        Ok(request)
    }
}

impl Display for HttpRequest {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        writeln!(f, "{} {}", self.method, self.url)?;

        for header in &self.headers {
            writeln!(f, "{}", header)?;
        }

        if let Some(ref body) = self.body {
            writeln!(f, "Body: {}", body)?;
        }

        Ok(())
    }
}


// todo: make an enum for body payloads where the available tags are
//       Text/Binary/Json and other things (lookup the RFCs)
// todo: remove HttpRequest and make a Log trait that gets implemented on
//       reqwest::Request and reqwest::Response.
// todo: impl common headers in RestResource (like Accept-Encoding)

impl HttpClient for reqwest::Client {
    fn new() -> Self {
        reqwest::Client::new()
    }

    fn send<Resource>(&mut self, host: &Url, resource: Resource) -> Result<Resource::Response, Error> where Resource: RestResource {
        let request = HttpRequest::new(host.clone(), &resource)?;
        // println!("{}", request);

        let mut request_builder = self.request(request.method.into(), request.url);

        let mut reqwest_headers = reqwest::header::Headers::new();
        for header in request.headers {
            reqwest_headers.set_raw(header.name.clone(), header.values.as_slice().join("; "));
        }
        request_builder.headers(reqwest_headers);

        match request.body {
            Some(Payload::Text(body)) => {request_builder.body(body);},
            Some(Payload::Binary(body)) => {request_builder.body(body);},
            None => (),
        }

        let request = request_builder.build()?;
        let response: HttpResponse = self.execute(request)?.into();
        // println!("{}", response);

        Ok(resource.deserialize(&response)?)
    }
}



pub struct TungsteniteClient<R> where R: WebsocketResource {
    pub client: tungstenite::protocol::WebSocket<tungstenite::client::AutoStream>,
    pub _resource: ::std::marker::PhantomData<R>,
}

pub trait WebsocketClient<R>: Sized where R: WebsocketResource {
    type Error;

    fn connect(url: Url, request: R) -> Result<Self, Self::Error>;
    fn recv(&mut self) -> Result<R::Message, Self::Error>;
    fn send(&mut self, message: R::Message) -> Result<(), Self::Error>; 
}

impl<R> WebsocketClient<R> for TungsteniteClient<R> where R: WebsocketResource {
    type Error = tungstenite::error::Error;

    fn connect(url: Url, request: R) -> Result<Self, tungstenite::error::Error> {
        use tungstenite::handshake::client::{Request};

        let mut tungstenite_request = Request::from(url);
        // for header in request.headers() {
        //     match header.value {
        //         HeaderValue::String(value) => tungstenite_request.add_header(Cow::from(header.name), Cow::from(value)),
        //         HeaderValue::Bytes(value) => tungstenite_request.add_header(Cow::from(header.name), Cow::from(value)),
        //     }
        // }

        let (client, response) = tungstenite::connect(tungstenite_request).unwrap();
        if response.code != 101 {
            panic!("[tungstenite] server returned {}: {:?}", response.code, response.headers);
        }

        Ok(TungsteniteClient {
            client: client,
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
            tungstenite::protocol::Message::Text(text)      => WebsocketMessage::Text(text),
            tungstenite::protocol::Message::Binary(bytes)   => WebsocketMessage::Binary(bytes),
            tungstenite::protocol::Message::Ping(bytes)     => WebsocketMessage::Ping(bytes),
            tungstenite::protocol::Message::Pong(bytes)     => WebsocketMessage::Pong(bytes),
        }
    }
}

impl From<WebsocketMessage> for tungstenite::protocol::Message {
    fn from(message: WebsocketMessage) -> Self {
        match message {
            WebsocketMessage::Text(text)    => tungstenite::protocol::Message::Text(text),
            WebsocketMessage::Binary(bytes) => tungstenite::protocol::Message::Binary(bytes),
            WebsocketMessage::Ping(bytes)   => tungstenite::protocol::Message::Ping(bytes),
            WebsocketMessage::Pong(bytes)   => tungstenite::protocol::Message::Pong(bytes),
        }
    }
}