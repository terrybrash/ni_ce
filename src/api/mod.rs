use std::io;
use std::collections::HashMap;
use url::Url;
use std::fmt;

pub type Headers = HashMap<String, Vec<String>>;
pub type Query = Vec<(String, String)>;

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
        }
    }
}
pub trait Api {
    type Reply;
    type Body: io::Read;
    type Error;

    fn method(&self) -> Method;

    fn path(&self) -> String;

    fn query(&self) -> Query {
        Query::new()
    }

    fn headers(&self) -> Headers {
        Headers::new()
    }

    fn body(&self) -> Self::Body;

    fn parse<R>(&self, response: &mut R) -> Result<Self::Reply, Self::Error> where R: HttpResponse;
}

pub trait HttpResponse {
    type Body: io::Read;

    fn status(&self) -> u16;

    fn reason(&self) -> &str;

    fn headers(&self) -> Headers;

    fn body(&self) -> &mut Self::Body;
}

pub trait HttpClient {
    type Error;

    fn send<U, Request>(&mut self, url: U, request: Request) -> Result<Request::Reply, Self::Error>
        where U: Into<Url>, Request: Api;
}

pub trait WebsocketClient {
    // fn connect(&mut self, )
}