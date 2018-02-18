#![feature(associated_type_defaults)]
#![feature(fnbox)]
#![feature(crate_in_paths)]
#![feature(test)]
#![feature(try_from)]

#![allow(warnings)]

#[macro_use] extern crate failure;
#[macro_use] extern crate serde_derive;
extern crate base64;
extern crate chrono;
extern crate hex;
extern crate hmac;
extern crate num_traits;
extern crate reqwest;
extern crate rust_decimal;
extern crate serde;
extern crate serde_json;
extern crate sha2;
extern crate test;
extern crate tungstenite;
extern crate url;
extern crate uuid;

pub mod api;
pub mod exmo;
pub mod liqui;
pub mod gemini;
mod model;
mod status;
pub use model::*;

use failure::{Error};
use rust_decimal::Decimal as d128;
use std::boxed::FnBox;
use std::sync::mpsc;
use api::HttpClient;

pub trait Exchange<C> where C: HttpClient {
	fn name(&self) -> &'static str;
	fn maker_fee(&self) -> d128;
	fn taker_fee(&self) -> d128;
	fn min_quantity(&self, product: CurrencyPair) -> Option<d128>;
	/// The number of decimal places supported.
	fn orderbook_cooldown(&self) -> std::time::Duration;
	fn precision(&self) -> u32;
	fn sync_rest_client(&self) -> Box<SyncExchangeRestClient>;
	fn async_rest_client(&self) -> Box<AsyncExchangeRestClient>;
}

pub trait SyncExchangeRestClient: Send {
    fn balances(&mut self) -> Result<Vec<Balance>, Error>;
    fn orderbook(&mut self, product: CurrencyPair) -> Result<Orderbook, Error>;
    fn orders(&mut self, product: CurrencyPair) -> Result<Vec<Order>, Error>;
    fn place_order(&mut self, order: NewOrder) -> Result<Order, Error>;
}

pub trait AsyncExchangeRestClient {
	fn balances<'a>(&'a self) -> Future<'a, Result<Vec<Balance>, Error>>;
	fn orderbook<'a>(&'a self, product: CurrencyPair) -> Future<'a, Result<Orderbook, Error>>;
	fn orders<'a>(&'a self, product: CurrencyPair) -> Future<'a, Result<Vec<Order>, Error>>;
	fn place_order<'a>(&'a self, order: NewOrder) -> Future<'a, Result<Order, Error>>;
}

pub struct Future<'a, T> {
	closure: Box<FnBox() -> T + 'a>,
	awaited: bool,
}

impl<'a, T> Future<'a, T> {
	fn new<F>(closure: F) -> Self
	where F: FnBox() -> T + 'a {
		Future {
			closure: Box::new(closure),
			awaited: false,
		}
	}

	pub fn await(mut self) -> T {
		self.awaited = true;
		(self.closure)()
	}
}

// impl<'a, T> Drop for Future<'a, T> {
// 	fn drop(&mut self) {
// 		if !self.awaited {
// 			// The future's been dropped without awaiting a value. This
// 			// results in an object being left in the channel that will never
// 			// get taken out, so we have to take it out manually.
// 			// TODO
// 		}
// 	}
// }

fn dual_channel<A, B>() -> ((mpsc::Sender<A>, mpsc::Receiver<B>), (mpsc::Sender<B>, mpsc::Receiver<A>)) {
	let (sender_a, receiver_a) = mpsc::channel();
	let (sender_b, receiver_b) = mpsc::channel();
	let channel_ab = (sender_a, receiver_b);
	let channel_ba = (sender_b, receiver_a);
	(channel_ab, channel_ba)
}