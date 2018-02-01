#![feature(try_from)]
#![feature(associated_type_defaults)]
#![feature(crate_in_paths)]
#![allow(warnings)]

#[macro_use] extern crate failure;
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
extern crate reqwest;
extern crate tungstenite;
extern crate uuid;

pub mod api;
pub mod exmo;
pub mod liqui;
mod model;
mod status;
pub use model::*;

use failure::{ResultExt, Error, Fail, Context, err_msg};
use decimal::d128;

pub trait RestExchange: std::fmt::Debug {
    fn place_order(&mut self, order: NewOrder) -> Result<Order, Error>;
    fn balances(&mut self) -> Result<Vec<Balance>, Error>;
    fn orders(&mut self, product: CurrencyPair) -> Result<Vec<Order>, Error>;
    fn orderbook(&mut self, product: CurrencyPair) -> Result<Orderbook, Error>;
    // fn exchange(&mut self) -> MutexGuard<Exchange>;
}

pub fn d128_from_f64(float: f64) -> Result<d128, Error> {
    use std::str::{FromStr};
    match d128::from_str(&float.to_string()) {
    	Ok(decimal) => Ok(decimal),
    	Err(_) => Err(err_msg("Couldn't convert f64 into d128"))
    }
}
