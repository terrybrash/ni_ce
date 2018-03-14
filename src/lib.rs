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

pub mod exmo;
// pub mod liqui;
// pub mod gemini;

mod model;
mod status;
pub use model::*;

use failure::Error;
use rust_decimal::Decimal as d128;
use std::boxed::FnBox;
use std::sync::mpsc;
use api::HttpClient;
use std::sync::{Arc, Condvar, Mutex};
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

// trait SyncExchangeRestClient: Send {
//     fn balances(&mut self) -> Result<Vec<Balance>, Error>;
//     fn orderbook(&mut self, product: CurrencyPair) -> Result<Orderbook, Error>;
//     fn orders(&mut self, product: CurrencyPair) -> Result<Vec<Order>, Error>;
//     fn place_order(&mut self, order: NewOrder) -> Result<Order, Error>;
// }
//
// trait AsyncExchangeRestClient {
// 	fn balances<'a>(&'a self) -> Future<Result<Vec<Balance>, Error>>;
// 	fn orderbook<'a>(&'a self, product: CurrencyPair) -> Future<Result<Orderbook, Error>>;
// 	fn orders<'a>(&'a self, product: CurrencyPair) -> Future<Result<Vec<Order>, Error>>;
// 	fn place_order<'a>(&'a self, order: NewOrder) -> Future<Result<Order, Error>>;
// }

enum FutureStatus<T> {
    Returned(T),
    Dropped,
}

/// A handle to a value to be returned at a later time.
///
/// `Future` is the receiver of a value sent by [`FutureLock`].
///
/// [`FutureLock`]: struct.FutureLock.html
pub struct Future<T> {
    result: Arc<(Mutex<Option<FutureStatus<T>>>, Condvar)>,
}

impl<T> Future<T> {
    /// Create a `Future` and its corresponding [`FutureLock`].
    ///
    /// The [`FutureLock`] is meant to be sent to a separate thread where a return value will be
    /// created and sent back using the lock.
    ///
    /// [`FutureLock`]: struct.FutureLock.html
    pub fn await() -> (Future<T>, FutureLock<T>) {
        let future = Future {
            result: Arc::new((Mutex::new(None), Condvar::new())),
        };

        let lock = FutureLock::new(future.result.clone());

        (future, lock)
    }

    /// Wait for the paired [`FutureLock`] to either return a value with [`FutureLock::send`] or
    /// drop.
    ///
    /// [`FutureLock`]: struct.FutureLock.html
    /// [`FutureLock::send`]: struct.FutureLock.html#method.send
    pub fn wait(self) -> Result<T, &'static str> {
        let (ref lock, ref cvar) = *self.result;
        let mut lock = lock.lock().unwrap();

        // 1. Check if the result is immediately available.
        match lock.take() {
            Some(FutureStatus::Returned(result)) => return Ok(result),
            Some(FutureStatus::Dropped) => return Err("The future was dropped"),
            None => {
                // `None` is fine here. It means `wait` was called
                // before a result could be returned from the lock.
            }
        }

        // 2. The result wasn't immediately available, so we have to wait.
        match cvar.wait(lock).unwrap().take() {
            Some(FutureStatus::Returned(result)) => Ok(result),
            Some(FutureStatus::Dropped) => Err("The future was dropped"),
            None => {
                // Shouldn't be possible
                unreachable!()
            }
        }
    }
}

/// Created from [`Future::await`]. Used to return a value to a [`Future`].
///
/// `FutureLock` is meant to be sent to a different thread than the one it was created on. Once on
/// a separate thread, work can be done and sent back to the original thread, using `FutureLock`.
///
/// A call to [`Future::wait`] will block until either [`send`] is called or the
/// `FutureLock` is dropped.
///
/// [`Future`] and `FutureLock` can be thought of as a one-time channel, where [`Future`] is the
/// receiver and `FutureLock` is the sender.
///
/// [`Future`]: struct.Future.html
/// [`Future::wait`]: struct.Future.html#method.wait
/// [`Future::await`]: struct.Future.html#method.await
/// [`send`]: #method.send
pub struct FutureLock<T> {
    value: Arc<(Mutex<Option<FutureStatus<T>>>, Condvar)>,
    has_responded: bool,
}

impl<T> FutureLock<T> {
    fn new(value: Arc<(Mutex<Option<FutureStatus<T>>>, Condvar)>) -> Self {
        FutureLock {
            value: value,
            has_responded: false,
        }
    }

    /// Consumes the lock and returns a value to the [`Future`](struct.Future.html) that was created with this lock.
    pub fn send(mut self, result: T) {
        self.has_responded = true;
        let (ref value, ref cvar) = *self.value;
        let mut value = value.lock().unwrap();
        *value = Some(FutureStatus::Returned(result));
        cvar.notify_one();
    }
}

impl<T> Drop for FutureLock<T> {
    fn drop(&mut self) {
        // If the `FutureLock` hasn't been used to send a result, it needs to signal that it's been
        // dropped or else the `Future` will `await` forever.
        if !self.has_responded {
            self.has_responded = true;
            let (ref value, ref cvar) = *self.value;
            let mut value = value.lock().unwrap();
            *value = Some(FutureStatus::Dropped);
            cvar.notify_one()
        }
    }
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
