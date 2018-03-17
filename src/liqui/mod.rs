use crate as ccex;
use {Exchange};
use api::{Header, Headers, HttpClient, HttpRequest, HttpResponse, Method, NeedsAuthentication,
          Payload, PrivateRequest, Query, RestResource};
use chrono::Utc;
use failure::{Error, ResultExt};
use hex;
use hmac::{Hmac, Mac};
use num_traits::*;
use rust_decimal::Decimal as d128;
use serde::de::DeserializeOwned;
use serde_json;
use sha2::Sha512;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;
use std::time::Duration;
use url::Url;
use std::cell::RefCell;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::ops::Deref;

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
struct PrivateResponse<T> {
    success: i32,
    #[serde(rename = "return")]
    ok: Option<T>,
    error: Option<String>,
    code: Option<u32>,
}

#[derive(Debug, Fail)]
pub enum PrivateError {
    #[fail(display = "({}) {}", _0, _1)]
    InvalidOrder(u32, String),

    #[fail(display = "({}) {}", _0, _1)]
    InsufficientFunds(u32, String),

    #[fail(display = "({}) {}", _0, _1)]
    OrderNotFound(u32, String),

    #[fail(display = "({:?}) {}", _0, _1)]
    Unregistered(Option<u32>, String),
}

impl<T> PrivateResponse<T> {
    pub fn is_ok(&self) -> bool {
        self.success == 1
    }

    pub fn into_result(self) -> Result<T, PrivateError> {
        if self.is_ok() {
            Ok(self.ok.unwrap())
        } else {
            let error = match self.code {
                Some(code @ 803) | Some(code @ 804) | Some(code @ 805) | Some(code @ 806)
                | Some(code @ 807) => PrivateError::InvalidOrder(code, self.error.unwrap()),

                Some(code @ 831) | Some(code @ 832) => {
                    PrivateError::InsufficientFunds(code, self.error.unwrap())
                }

                Some(code @ 833) => PrivateError::OrderNotFound(code, self.error.unwrap()),

                code => PrivateError::Unregistered(code, self.error.unwrap()),
            };

            Err(error)
        }
    }
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Buy,
    Sell,
}

impl Display for Side {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        match *self {
            Side::Buy => write!(f, "buy"),
            Side::Sell => write!(f, "sell"),
        }
    }
}

impl From<ccex::Side> for Side {
    fn from(side: ccex::Side) -> Self {
        match side {
            ccex::Side::Bid => Side::Buy,
            ccex::Side::Ask => Side::Sell,
        }
    }
}

impl From<Side> for ccex::Side {
    fn from(side: Side) -> Self {
        match side {
            Side::Buy => ccex::Side::Bid,
            Side::Sell => ccex::Side::Ask,
        }
    }
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct Currency(String);
impl TryFrom<ccex::Currency> for Currency {
    type Error = Error;
    fn try_from(currency: ccex::Currency) -> Result<Self, Self::Error> {
        match currency {
            ccex::Currency::USDT => Ok(Currency(String::from("usdt"))),
            ccex::Currency::ETH => Ok(Currency(String::from("eth"))),
            ccex::Currency::BTC => Ok(Currency(String::from("btc"))),
            currency => Err(format_err!("{} isn't supported", currency)),
        }
    }
}
impl TryFrom<Currency> for ccex::Currency {
    type Error = Error;
    fn try_from(Currency(currency): Currency) -> Result<Self, Self::Error> {
        match currency.to_uppercase().as_str() {
            "USDT" => Ok(ccex::Currency::USDT),
            "ETH" => Ok(ccex::Currency::ETH),
            "BTC" => Ok(ccex::Currency::BTC),
            currency => Err(format_err!("{} isn't supported", currency)),
        }
    }
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct CurrencyPair(String);
impl TryFrom<ccex::CurrencyPair> for CurrencyPair {
    type Error = Error;
    fn try_from(ccex::CurrencyPair(base, quote): ccex::CurrencyPair) -> Result<Self, Self::Error> {
        let Currency(base) = base.try_into()?;
        let Currency(quote) = quote.try_into()?;
        Ok(CurrencyPair(format!("{}_{}", base, quote)))
    }
}
impl TryFrom<CurrencyPair> for ccex::CurrencyPair {
    type Error = Error;
    fn try_from(CurrencyPair(currency_pair): CurrencyPair) -> Result<Self, Self::Error> {
        let currencies: Vec<&str> = currency_pair.split('_').collect();
        let base = Currency(currencies[0].to_owned());
        let quote = Currency(currencies[1].to_owned());
        let currency_pair = ccex::CurrencyPair(base.try_into()?, quote.try_into()?);
        Ok(currency_pair)
    }
}
impl Display for CurrencyPair {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        f.write_str(self.0.as_str())
    }
}

#[derive(Debug, PartialEq, PartialOrd, Clone, Deserialize, Serialize)]
pub struct Orderbook {
    pub bids: Vec<(f64, f64)>,
    pub asks: Vec<(f64, f64)>,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct Info {
    /// Your account balance available for trading. Doesn’t include funds on
    /// your open orders.
    pub funds: HashMap<Currency, f64>,

    /// The privileges of the current API key. At this time the privilege to
    /// withdraw is not used anywhere.
    pub rights: Rights,

    /// The number of your open orders.
    pub open_orders: i64,

    /// Server time (UTC).
    pub server_time: i64,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct Rights {
    pub info: bool,
    pub trade: bool,
    pub withdraw: bool,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct OrderPlacement {
    /// The amount of currency bought/sold.
    received: f64,

    /// The remaining amount of currency to be bought/sold (and the initial
    /// order amount).
    remains: f64,

    /// Is equal to 0 if the request was fully “matched” by the opposite
    /// orders, otherwise the ID of the executed order will be returned.
    order_id: i64,

    /// Balance after the request.
    funds: HashMap<Currency, f64>,
}

// pub struct GetActiveOrders {
// 	pair: CurrencyPair,
// 	nonce: u32,
// }

#[derive(Debug, PartialEq, PartialOrd, Clone, Deserialize, Serialize)]
pub struct Order {
    pub status: i32,
    pub pair: CurrencyPair,
    #[serde(rename = "type")]
    pub side: Side,
    pub amount: f64,
    pub rate: f64,
    pub timestamp_created: u64,
}

pub type OrderId = String;
// impl<'a> NeedsAuthentication<&'a Credential> for GetActiveOrders {}
// impl<'a> RestResource for PrivateRequest<GetActiveOrders, &'a Credential> {
// 	type Response = HashMap<OrderId, Order>;
//
// 	fn method(&self) -> Method {
// 		Method::Post
// 	}
//
// 	fn path(&self) -> String {
// 		"/tapi".to_owned()
// 	}
//
// 	fn body(&self) -> Result<Option<Payload>, Error> {
// 		let body = QueryBuilder::with_capacity(3)
// 			.param("method", "ActiveOrders")
// 			.param("nonce", self.request.nonce.to_string())
// 			.param("pair", self.request.pair.to_string())
// 			.build();
//
// 		Ok(Some(Payload::Text(body.to_string())))
// 	}
//
// 	fn headers(&self) -> Result<Headers, Error> {
// 		private_headers(self, &self.credential)
// 	}
//
// 	fn deserialize(&self, response: &HttpResponse) -> Result<Self::Response, Error> {
// 		deserialize_private_response(response)
// 	}
// }

#[derive(Deserialize, Serialize)]
struct ErrorResponse {
    pub success: i64,
    pub error: String,
}

pub struct Liqui<Client: HttpClient> {
    pub host: Url,
    pub http_client: RefCell<Client>,
    pub credential: ccex::Credential,
}

impl<Client: HttpClient> Liqui<Client> {
    pub fn new(credential: &ccex::Credential) -> Self {
        Liqui {
            host: Url::parse("https://api.liqui.io").unwrap(),
            http_client: RefCell::new(Client::new()),
            credential: credential.clone(),
        }
    }

    fn deserialize_public_response<T>(response: &HttpResponse) -> Result<T, Error>
    where
        T: DeserializeOwned,
    {
        let response: serde_json::Value = match response.body {
            Some(Payload::Text(ref body)) => serde_json::from_str(body)?,
            Some(Payload::Binary(ref body)) => serde_json::from_slice(body)?,
            None => return Err(format_err!("body is empty")),
        };

        let is_success = response
            .as_object()
            .and_then(|obj| obj.get("success"))
            .and_then(|obj| obj.as_u64())
            .map_or(true, |obj| if obj == 0 { false } else { true });

        if is_success {
            let response: T = serde_json::from_value(response)?;
            Ok(response)
        } else {
            let response: ErrorResponse = serde_json::from_value(response)?;
            Err(format_err!("The server returned: {}", response.error))
        }
    }

    fn deserialize_private_response<T>(response: &HttpResponse) -> Result<T, Error>
    where
        T: DeserializeOwned,
    {
        let response = match response.body {
            Some(Payload::Text(ref body)) => body,
            Some(Payload::Binary(ref body)) => {
                return Err(format_err!(
                    "the response body doesn't contain valid utf8 text: {:?}",
                    body
                ))
            }
            None => return Err(format_err!("the body is empty")),
        };

        let response: PrivateResponse<T> = serde_json::from_str(&response)
            .with_context(|e| format!("failed to deserialize: \"{}\"", response))?;

        response
            .into_result()
            .map_err(|e| format_err!("the server returned \"{}\"", e))
    }

    fn private_headers(
        credential: &ccex::Credential,
        body: Option<&str>,
    ) -> Result<Headers, Error> {
        let mut mac =
            Hmac::<Sha512>::new(credential.secret.as_bytes()).map_err(|e| format_err!("{:?}", e))?;
        if let Some(body) = body {
            mac.input(body.as_bytes());
        }
        let signature = hex::encode(mac.result().code().to_vec());

        let headers = vec![
            Header::new("Key", credential.key.clone()),
            Header::new("Sign", signature),
        ];
        Ok(headers)
    }

    fn nonce() -> u32 {
        // TODO: switch to a cached nonce at some point. this has the limitations
        // of 1) only allowing one request per millisecond and 2) expiring after
        // ~50 days
        let now = Utc::now();
        (now.timestamp() as u32 - 1521186749u32) * 1000 + now.timestamp_subsec_millis()
    }

    fn get_info(&self) -> Result<Info, Error> {
        // Liqui encodes its body data as an http query.
        let body = {
            let mut query = Query::with_capacity(2);
            query.insert_param("method", "getInfo");
            query.insert_param("nonce", Self::nonce().to_string());
            query.to_string()
        };
        let headers = Self::private_headers(&self.credential, Some(&body))?;

        let http_request = HttpRequest {
            method: Method::Post,
            host: self.host.as_str(),
            path: "/tapi",
            body: Some(Payload::Text(body)),
            headers: Some(headers),
            query: None,
        };
        let http_response = self.http_client.borrow_mut().send(&http_request)?;
        Self::deserialize_private_response(&http_response)
    }
}

impl<Client: HttpClient> Exchange for Liqui<Client> {
    fn get_balances(&self) -> Result<HashMap<ccex::Currency, d128>, Error> {
        let user_info = self.get_info()?;

        user_info.funds.into_iter()
        	// If a currency can't be converted, it means it's been newly
        	// added to Liqui and hasn't been added to the `Currency` enum. In
        	// that case, ignoring it is fine.
            .filter_map(|(currency, balance)| {
                match ccex::Currency::try_from(currency) {
                    Ok(currency) => Some((currency, balance)),
                    Err(_) => None,
                }
            })
            .map(|(currency, balance)| {
                let balance = d128::from_f64(balance)
                    .ok_or_else(|| format_err!("Couldn't convert {} into a decimal", balance))?;
                Ok((currency, balance))
            })
            .collect()
    }

    fn get_orderbooks(
        &self,
        products: &[ccex::CurrencyPair],
    ) -> Result<HashMap<ccex::CurrencyPair, ccex::Orderbook>, Error> {
        let products: Vec<String> = products
            .iter()
            .map(|product| {
                let product = CurrencyPair::try_from(*product)?;
                Ok(product.to_string())
            })
            .collect::<Result<Vec<String>, Error>>()?;
        let path = ["/api/3/depth/", products.join("-").as_str()].concat();
        let http_request = HttpRequest {
            method: Method::Get,
            host: self.host.as_str(),
            path: path.as_str(),
            headers: None,
            body: None,
            query: None,
        };

        let http_response = self.http_client.borrow_mut().send(&http_request)?;

        let orderbook: HashMap<CurrencyPair, Orderbook> =
            Self::deserialize_public_response(&http_response)?;
        orderbook
            .into_iter()
            .filter_map(|(product, orderbook)| {
                let product: Result<ccex::CurrencyPair, Error> = product.try_into();
                match product {
                    Ok(product) => Some((product, orderbook)),
                    Err(_) => {
                        // If we get a product back that we don't support, just silently filter it.
                        None
                    }
                }
            })
            .map(|(product, orderbook)| {
                let asks: Result<ccex::Asks, Error> = orderbook
                    .asks
                    .iter()
                    .map(|&(price, amount)| {
                        let price = d128::from_f64(price).ok_or_else(|| {
                            format_err!("Couldn't convert {} into a decimal", price)
                        })?;
                        let amount = d128::from_f64(amount).ok_or_else(|| {
                            format_err!("Couldn't convert {} into a decimal", amount)
                        })?;
                        Ok(ccex::Offer::new(price, amount))
                    })
                    .collect();
                let bids: Result<ccex::Bids, Error> = orderbook
                    .bids
                    .iter()
                    .map(|&(price, amount)| {
                        let price = d128::from_f64(price).ok_or_else(|| {
                            format_err!("Couldn't convert {} into a decimal", price)
                        })?;
                        let amount = d128::from_f64(amount).ok_or_else(|| {
                            format_err!("Couldn't convert {} into a decimal", amount)
                        })?;
                        Ok(ccex::Offer::new(price, amount))
                    })
                    .collect();
                Ok((product, ccex::Orderbook::new(asks?, bids?)))
            })
            .collect()
    }

    fn place_order(&self, order: ccex::NewOrder) -> Result<ccex::Order, Error> {
        let (price, quantity) = match order.instruction {
            ccex::NewOrderInstruction::Limit {
                price, quantity, ..
            } => (price, quantity),
            instruction => unimplemented!("liqui doesn't support {:?}", instruction),
        };
        let product: CurrencyPair = order.product.try_into()?;
        let side: Side = order.side.into();
        let body = {
            let mut query = Query::with_capacity(6);
            query.insert_param("nonce", Self::nonce().to_string());
            query.insert_param("method", "trade");
            query.insert_param("pair", product.to_string());
            query.insert_param("type", side.to_string());
            query.insert_param("rate", price.to_string());
            query.insert_param("amount", quantity.to_string());
            query.to_string()
        };
        let headers = Self::private_headers(&self.credential, Some(body.as_str()))?;
        let http_request = HttpRequest {
            method: Method::Post,
            host: self.host.as_str(),
            path: "/tapi",
            body: Some(Payload::Text(body)),
            headers: Some(headers),
            query: None,
        };

        let http_response = self.http_client.borrow_mut().send(&http_request)?;

        let placed_order: OrderPlacement = Self::deserialize_private_response(&http_response)?;
        let placed_order = ccex::Order {
            id: None,        //Some(placed_order.order_id),
            server_id: None, //Some(placed_order.order_id.to_string()),
            side: order.side,
            product: order.product,
            status: ccex::OrderStatus::Open,
            instruction: ccex::OrderInstruction::Limit {
                price: price,
                original_quantity: d128::from_f64(placed_order.received).unwrap()
                    + d128::from_f64(placed_order.remains).unwrap(),
                remaining_quantity: d128::from_f64(placed_order.remains).unwrap(),
                time_in_force: ccex::TimeInForce::GoodTillCancelled,
            },
        };
        Ok(placed_order)
    }

    // fn orders(&mut self, product: ccex::CurrencyPair) -> Result<Vec<ccex::Order>, Error> {
    // 	let request = GetActiveOrders {
    // 		pair: product.try_into()?,
    // 		nonce: nonce(),
    // 	};
    // 	let request = request.authenticate(&self.credential);
    // 	let response = self.client.send(&self.host, request)?;
    //
    // 	let mut orders = Vec::with_capacity(response.len());
    // 	for (id, order) in response.into_iter() {
    // 		let order = ccex::Order {
    // 			id: None,
    // 			server_id: Some(id),
    // 			side: order.side.into(),
    // 			product: order.pair.parse::<CurrencyPair>()?.try_into()?,
    // 			status: ccex::OrderStatus::Open,
    // 			instruction: ccex::OrderInstruction::Limit {
    // 				price: d128::from_f64(order.rate).unwrap(),
    // 				original_quantity: d128::zero(),
    // 				remaining_quantity: d128::from_f64(order.amount).unwrap(),
    // 				time_in_force: ccex::TimeInForce::GoodTillCancelled,
    // 			}
    // 		};
    // 		orders.push(order);
    // 	}
    // 	Ok(orders)
    // }

    fn name(&self) -> &'static str {
        "Liqui"
    }

    fn maker_fee(&self) -> d128 {
        // 0.001 (0.01%)
        d128::new(1, 3)
    }

    fn taker_fee(&self) -> d128 {
        // 0.0025 (0.025%)
        d128::new(25, 4)
    }

    fn precision(&self) -> u32 {
        8
    }

    fn min_quantity(&self, product: ccex::CurrencyPair) -> Option<d128> {
        match product {
            ccex::CurrencyPair(ccex::Currency::LTC, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::STEEM, ccex::Currency::BTC) => Some(d128::new(1, 3)),
            ccex::CurrencyPair(ccex::Currency::SBD, ccex::Currency::BTC) => Some(d128::new(1, 3)),
            ccex::CurrencyPair(ccex::Currency::DASH, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::ANS, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::DCT, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::ICN, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::ETH, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::XZC, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::GOLOS, ccex::Currency::BTC) => Some(d128::new(1, 3)),
            ccex::CurrencyPair(ccex::Currency::GBG, ccex::Currency::BTC) => Some(d128::new(1, 3)),
            ccex::CurrencyPair(ccex::Currency::GNT, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::WINGS, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::PLU, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::ROUND, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::VSL, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::WAVES, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::INCNT, ccex::Currency::BTC) => Some(d128::new(1, 4)),
            ccex::CurrencyPair(ccex::Currency::MLN, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::TIME, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::GNT, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::LTC, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::DASH, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::ICN, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::MLN, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::ROUND, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::WAVES, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::TIME, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::VSL, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::PLU, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::INCNT, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::LTC, ccex::Currency::USDT) => Some(d128::new(1, 3)),
            ccex::CurrencyPair(ccex::Currency::BTC, ccex::Currency::USDT) => Some(d128::new(1, 5)),
            ccex::CurrencyPair(ccex::Currency::DASH, ccex::Currency::USDT) => Some(d128::new(1, 4)),
            ccex::CurrencyPair(ccex::Currency::ETH, ccex::Currency::USDT) => Some(d128::new(1, 4)),
            ccex::CurrencyPair(ccex::Currency::ICN, ccex::Currency::USDT) => Some(d128::new(1, 2)),
            ccex::CurrencyPair(ccex::Currency::GNT, ccex::Currency::USDT) => Some(d128::new(1, 2)),
            ccex::CurrencyPair(ccex::Currency::ROUND, ccex::Currency::USDT) => Some(d128::one()),
            ccex::CurrencyPair(ccex::Currency::VSL, ccex::Currency::USDT) => Some(d128::new(1, 1)),
            ccex::CurrencyPair(ccex::Currency::WAVES, ccex::Currency::USDT) => {
                Some(d128::new(1, 2))
            }
            ccex::CurrencyPair(ccex::Currency::MLN, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::TIME, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::REP, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::EDG, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::REP, ccex::Currency::ETH) => Some(d128::new(1, 7)),
            ccex::CurrencyPair(ccex::Currency::EDG, ccex::Currency::ETH) => Some(d128::new(1, 7)),
            ccex::CurrencyPair(ccex::Currency::REP, ccex::Currency::USDT) => Some(d128::new(1, 7)),
            ccex::CurrencyPair(ccex::Currency::EDG, ccex::Currency::USDT) => Some(d128::new(1, 7)),
            ccex::CurrencyPair(ccex::Currency::RLC, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::RLC, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::RLC, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::TRST, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::TRST, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::TRST, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::WINGS, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::WINGS, ccex::Currency::USDT) => {
                Some(d128::new(1, 8))
            }
            ccex::CurrencyPair(ccex::Currency::PLU, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::INCNT, ccex::Currency::USDT) => {
                Some(d128::new(1, 8))
            }
            ccex::CurrencyPair(ccex::Currency::GNO, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::GNO, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::GNO, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::GUP, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::GUP, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::GUP, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::TAAS, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::TAAS, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::TAAS, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::LUN, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::LUN, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::LUN, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::TKN, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::TKN, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::TKN, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::HMQ, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::HMQ, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::HMQ, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::BCAP, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::BCAP, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::BCAP, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::ANT, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::ANT, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::ANT, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::BAT, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::BAT, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::BAT, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::QRL, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::QRL, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::QRL, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::BNT, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::BNT, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::BNT, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::MGO, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::MGO, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::MGO, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::MYST, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::MYST, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::MYST, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::SNGLS, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::SNGLS, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::SNGLS, ccex::Currency::USDT) => {
                Some(d128::new(1, 8))
            }
            ccex::CurrencyPair(ccex::Currency::PTOY, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::PTOY, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::PTOY, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::CFI, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::CFI, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::CFI, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::SNM, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::SNM, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::SNM, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::SNT, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::SNT, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::SNT, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::MCO, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::MCO, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::MCO, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::STORJ, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::STORJ, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::STORJ, ccex::Currency::USDT) => {
                Some(d128::new(1, 8))
            }
            ccex::CurrencyPair(ccex::Currency::ADX, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::ADX, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::ADX, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::EOS, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::EOS, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::EOS, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::PAY, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::PAY, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::PAY, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::XID, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::XID, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::XID, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::OMG, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::OMG, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::OMG, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::SAN, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::SAN, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::SAN, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::QTUM, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::QTUM, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::QTUM, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::CVC, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::CVC, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::CVC, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::NET, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::NET, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::NET, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::DGD, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::DGD, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::DGD, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::OAX, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::OAX, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::OAX, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::BCH, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::BCH, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::BCH, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::DNT, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::DNT, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::DNT, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::STX, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::STX, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::STX, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::ZRX, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::ZRX, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::ZRX, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::TNT, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::TNT, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::TNT, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::AE, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::AE, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::AE, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::VEN, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::VEN, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::VEN, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::BMC, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::BMC, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::BMC, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::MANA, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::MANA, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::MANA, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::PRO, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::PRO, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::PRO, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::KNC, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::KNC, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::KNC, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::SALT, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::SALT, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::SALT, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::IND, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::IND, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::IND, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::TRX, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::TRX, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::TRX, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::ENG, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::ENG, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::ENG, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::AST, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::AST, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::AST, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::REQ, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::REQ, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::REQ, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::NEU, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::NEU, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::NEU, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::SRN, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::SRN, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::SRN, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::INS, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::INS, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::INS, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::AION, ccex::Currency::BTC) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::AION, ccex::Currency::ETH) => Some(d128::new(1, 8)),
            ccex::CurrencyPair(ccex::Currency::AION, ccex::Currency::USDT) => Some(d128::new(1, 8)),
            _ => None,
        }
    }
}
