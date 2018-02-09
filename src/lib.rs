#![feature(try_from)]
#![feature(associated_type_defaults)]
#![feature(test)]
#![feature(crate_in_paths)]
#![allow(warnings)]

#[macro_use] extern crate failure;
#[macro_use] extern crate serde_derive;
// #[macro_use] extern crate decimal;
#[macro_use] extern crate rust_decimal;

extern crate test;
extern crate num_traits;
extern crate base64;
extern crate chrono;
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
// use decimal::d128;
use rust_decimal::Decimal as d128;

pub trait RestExchange: std::fmt::Debug {
	fn name(&self) -> &'static str;
	fn orderbook_cooldown(&self) -> std::time::Duration;
	fn maker_fee(&self) -> d128;
	fn taker_fee(&self) -> d128;
	fn min_quantity(&self, product: CurrencyPair) -> Option<d128>;

	/// The number of decimal places supported.
	fn precision(&self) -> u32;

    fn place_order(&mut self, order: NewOrder) -> Result<Order, Error>;
    fn balances(&mut self) -> Result<Vec<Balance>, Error>;
    fn orders(&mut self, product: CurrencyPair) -> Result<Vec<Order>, Error>;
    fn orderbook(&mut self, product: CurrencyPair) -> Result<Orderbook, Error>;
    // fn exchange(&mut self) -> MutexGuard<Exchange>;
}
