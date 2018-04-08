#![deny(missing_debug_implementations)]

extern crate base64;
extern crate chrono;
#[macro_use]
extern crate failure;
extern crate hex;
extern crate hmac;
extern crate http;
extern crate num_traits;
extern crate reqwest;
extern crate rust_decimal;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate sha2;
extern crate url;

#[path = "http.rs"]
mod _http;
pub use _http::HttpClient;
use _http::Query;

pub mod liqui;
pub mod binance;
pub mod exmo;
