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

pub mod exmo;
// pub mod liqui;
// pub mod gemini;

mod model;
mod status;
pub use model::*;

use failure::Error;
use rust_decimal::Decimal as d128;
use std::sync::mpsc;
use std::collections::HashMap;
use crate as ccex;


// /// The interface to an exchange.
// pub trait Exchange<C> where C: HttpClient {
// 	fn name(&self) -> &'static str;
//
//     /// The maker fee as a percentage. `1.0` is equal to 100%.
// 	fn maker_fee(&self) -> d128;
//
//     /// The taker fee as a percentage. `1.0` is equal to 100%.
//     fn taker_fee(&self) -> d128;
//
//     /// The minimum quantity allowed for trades of `product`.
// 	fn min_quantity(&self, product: CurrencyPair) -> Option<d128>;
//
// 	/// The number of decimal places supported.
// 	fn precision(&self) -> u32;
//
//     /// Non-blocking request for a new orderbook for a given `product`.
//     fn orderbook(&mut self, product: CurrencyPair) -> Future<Result<Orderbook, Error>>;
//
//     /// Non-blocking request to place a new order.
//     fn place_order(&mut self, order: NewOrder) -> Future<Result<Order, Error>>;
//
//     /// Non-blocking request for current currency balances.
//     fn balances(&mut self) -> Future<Result<Vec<Balance>, Error>>;
// }

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

    /// Non-blocking request for a new orderbook for a given `product`.
    fn get_orderbooks(
        &mut self,
        products: &[ccex::CurrencyPair],
    ) -> Result<HashMap<ccex::CurrencyPair, ccex::Orderbook>, Error>;

    /// Non-blocking request to place a new order.
    fn place_order(
        &mut self,
        credential: &ccex::Credential,
        order: ccex::NewOrder,
    ) -> Result<ccex::Order, Error>;

    /// Non-blocking request for current currency balances.
    fn get_balances(&mut self, credential: &ccex::Credential) -> Result<Vec<ccex::Balance>, Error>;
}

fn dual_channel<A, B>() -> (
    (mpsc::Sender<A>, mpsc::Receiver<B>),
    (mpsc::Sender<B>, mpsc::Receiver<A>),
) {
    let (sender_a, receiver_a) = mpsc::channel();
    let (sender_b, receiver_b) = mpsc::channel();
    let channel_ab = (sender_a, receiver_b);
    let channel_ba = (sender_b, receiver_a);
    (channel_ab, channel_ba)
}
