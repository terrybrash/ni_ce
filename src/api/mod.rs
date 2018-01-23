use std::io::{self, Read};
use std::collections::HashMap;
use url::Url;
use std::fmt;
use std::borrow::Cow;
use std::io::Cursor;
use failure::{Fail, Error};
use std::string::FromUtf8Error; 

use reqwest;
use tungstenite;

pub type Headers = HashMap<String, String>;
pub type Query = Vec<(String, String)>;

/// Specifies that a request first needs to be authenticated before becoming a valid request.
pub trait NeedsAuthentication<C>: Sized + fmt::Debug where C: fmt::Debug {
    fn authenticate(self, credential: C) -> PrivateRequest<Self, C> {
        PrivateRequest {
            credential: credential,
            request: self,
        }
    }
}

/// Wrapper for requests that require authentication.
#[derive(Debug)]
pub struct PrivateRequest<R, C> where R: fmt::Debug, C: fmt::Debug {
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
    // type Error: Fail;

    fn method(&self) -> Method;

    fn path(&self) -> String;

    fn query(&self) -> Query {
        Query::new()
    }

    fn headers(&self) -> Result<Headers, Error> {
        Ok(Headers::new())
    }

    fn body(&self) -> Result<Vec<u8>, Error> {
        Ok(Vec::new())
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

// pub trait HttpResponse {
//     type Body: io::Read;

//     fn status(&self) -> u16;

//     fn reason(&self) -> Option<&str>;

//     fn headers(&self) -> Headers;

//     fn body(&mut self) -> &mut Self::Body;
// }

pub trait HttpClient {
    type Error: fmt::Debug;

    fn send<Request>(&mut self, url: Url, request: Request) -> Result<Request::Response, Self::Error> where Request: RestResource;
}

// --------------------------------------

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

// impl HttpResponse for reqwest::Response {
//     type Body = Self;

//     fn status(&self) -> u16 {
//         self.status().as_u16()
//     }

//     fn reason(&self) -> Option<&str> {
//         self.status().canonical_reason()
//     }

//     fn headers(&self) -> Headers {
//         Headers::new()
//     }

//     fn body(&mut self) -> &mut Self::Body {
//         self
//     }
// }

#[derive(Debug, Clone)]
pub struct HttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
    pub headers: Headers,
}

impl HttpResponse {
    fn to_string(&self) -> Result<String, FromUtf8Error> {
        String::from_utf8(self.body.clone())
    }
}

impl From<reqwest::Response> for HttpResponse {
    fn from(mut response: reqwest::Response) -> Self {
        let mut body = Vec::with_capacity(1024);
        response.read_to_end(&mut body).unwrap();
        HttpResponse {
            status: response.status().as_u16(),
            headers: Headers::new(),
            body: body,
        }
    }
}

impl HttpClient for reqwest::Client {
    type Error = Error;

    fn send<Resource>(&mut self, mut url: Url, request: Resource) -> Result<Resource::Response, Self::Error> where Resource: RestResource {
        let mut headers = reqwest::header::Headers::new();
        for (name, value) in request.headers()? {
            headers.set_raw(name.to_owned(), value.to_owned());
        }

        url = url.join(&request.path())?;
        url.query_pairs_mut().extend_pairs(request.query());
        
        let response: HttpResponse = 
            self.request(request.method().into(), url)
            .headers(headers)
            .body(reqwest::Body::new(Cursor::new(request.body().unwrap())))
            .send()
            .unwrap()
            .into();

        println!("Response");
        println!("  Code: {}", response.status);
        println!("  Body: {}", response.to_string().unwrap());
        Ok(request.deserialize(&response)?)
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
        for (name, value) in request.headers() {
            tungstenite_request.add_header(Cow::from(name), Cow::from(value));
        }

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