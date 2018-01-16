#![feature(crate_in_paths)]

#[macro_use] extern crate serde_derive;
extern crate base64;
extern crate chrono;
extern crate decimal;
extern crate hex;
extern crate hmac;
extern crate serde;
extern crate serde_json;
extern crate sha2;
extern crate url;
// extern crate api;

pub mod api;
pub mod gemini;
pub mod gdax;
mod model;

pub use model::*;

use url::Url;

pub type Header = (&'static str, String);

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Method {
	Post,
	Get,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Request {
    pub address: Url,
    pub headers: Option<Vec<Header>>,
    pub method: Method,
    pub payload: Option<String>,
}