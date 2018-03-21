#![feature(associated_type_defaults)]
#![feature(fnbox)]
#![feature(crate_in_paths)]
#![feature(test)]
#![feature(try_from)]
#![allow(warnings)]

extern crate base64;
extern crate chrono;
#[macro_use]
extern crate failure;
extern crate hex;
extern crate hmac;
extern crate num_traits;
extern crate reqwest;
extern crate rust_decimal;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate sha2;
extern crate test;
extern crate tungstenite;
extern crate url;
extern crate uuid;

pub mod api;
pub mod future;

// pub mod exmo;
pub mod liqui;
pub mod binance;
// pub mod gemini;
// pub mod hitbtc;

mod model;
mod status;
pub use model::*;

use failure::Error;
use rust_decimal::Decimal as d128;
use std::collections::HashMap;
use crate as ccex;

/// The interface to an exchange.
pub trait Exchange {
    fn name(&self) -> &'static str;

    /// The maker fee as a percentage. `1.0` is equal to 100%.
    fn maker_fee(&self) -> d128;

    /// The taker fee as a percentage. `1.0` is equal to 100%.
    fn taker_fee(&self) -> d128;

    /// The minimum quantity allowed for trades of `product`.
    fn min_quantity(&self, product: ccex::CurrencyPair) -> Option<d128>;

    /// The number of decimal places supported.
    fn precision(&self) -> u32;

    /// Request the orderbooks for given products.
    fn get_orderbooks(
        &self,
        products: &[ccex::CurrencyPair],
    ) -> Result<HashMap<ccex::CurrencyPair, ccex::Orderbook>, Error>;

    /// Place a new order.
    fn place_order(&self, order: ccex::NewOrder) -> Result<ccex::Order, Error>;

    /// Get the account's balances available for trading.
    fn get_balances(&self) -> Result<HashMap<ccex::Currency, d128>, Error>;
}
