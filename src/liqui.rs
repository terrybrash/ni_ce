use api::{
	Header,
	Headers,
	HttpClient,
	HttpResponse,
	Method,
	NeedsAuthentication,
	Payload,
	PrivateRequest,
	Query,
	QueryBuilder,
	RestResource,
};
use crate as ccex;
use chrono::{Utc};
use decimal::{d128};
use failure::{Error, ResultExt};
use hex;
use hmac::{Hmac, Mac};
use serde::de::{DeserializeOwned};
use serde_json;
use sha2::{Sha512};
use std::fmt::{self, Display, Formatter};
use std::str::{FromStr};
use url::Url;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct Credential {
	pub key: String,
	pub secret: String,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
struct PrivateResponse<T> {
	success: i32,
	#[serde(rename="return")]
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
				Some(code @ 803)
				| Some(code @ 804)
				| Some(code @ 805)
				| Some(code @ 806)
				| Some(code @ 807)
				=> PrivateError::InvalidOrder(code, self.error.unwrap()),

				Some(code @ 831)
				| Some(code @ 832)
				=> PrivateError::InsufficientFunds(code, self.error.unwrap()),

				Some(code @ 833)
				=> PrivateError::OrderNotFound(code, self.error.unwrap()),

				code
				=> PrivateError::Unregistered(code, self.error.unwrap()),
			};

			Err(error)
		}
	}
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all="lowercase")]
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

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Copy, Deserialize, Serialize)]
pub enum Currency {
	BTC,
	ETH,
	USDT,
	LTC,
	BCH,
}

impl TryFrom<ccex::Currency> for Currency {
	type Error = Error;
	fn try_from(currency: ccex::Currency) -> Result<Self, Self::Error> {
		match currency {
			ccex::Currency::BTC => Ok(Currency::BTC),
			ccex::Currency::ETH => Ok(Currency::ETH),
			ccex::Currency::USDT => Ok(Currency::USDT),
			ccex::Currency::LTC => Ok(Currency::LTC),
			ccex::Currency::BCH	 => Ok(Currency::BCH),
			currency => Err(format_err!("{} isn't supported", currency))
		}
	}
}

impl TryFrom<Currency> for ccex::Currency {
	type Error = Error;
	fn try_from(currency: Currency) -> Result<Self, Self::Error> {
		match currency {
			Currency::BTC => Ok(ccex::Currency::BTC),
			Currency::ETH => Ok(ccex::Currency::ETH),
			Currency::USDT => Ok(ccex::Currency::USDT),
			Currency::LTC => Ok(ccex::Currency::LTC),
			Currency::BCH => Ok(ccex::Currency::BCH),
			currency => Err(format_err!("{} isn't supported", currency))
		}
	}
}

impl FromStr for Currency {
	type Err = Error;
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		const currencies: [(&'static str, Currency); 5] = [
			("BTC", Currency::BTC),
			("ETH", Currency::ETH),
			("USDT", Currency::USDT),
			("LTC", Currency::LTC),
			("BCH", Currency::BCH),
        ];

        for &(string, currency) in &currencies {
            if string.eq_ignore_ascii_case(s) {
                return Ok(currency);
            }
        }
        Err(format_err!("couldn't parse \"{}\"", s))
	}
}

impl Display for Currency {
	fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
		f.write_str(format!("{:?}", self).to_lowercase().as_str())
	}
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Copy, Deserialize, Serialize)]
pub struct CurrencyPair(pub Currency, pub Currency);

impl TryFrom<ccex::CurrencyPair> for CurrencyPair {
	type Error = Error;
	fn try_from(ccex::CurrencyPair(base, quote): ccex::CurrencyPair) -> Result<Self, Self::Error> {
		Ok(CurrencyPair(base.try_into()?, quote.try_into()?))
	}
}

impl TryFrom<CurrencyPair> for ccex::CurrencyPair {
	type Error = Error;
	fn try_from(CurrencyPair(base, quote): CurrencyPair) -> Result<Self, Self::Error> {
		Ok(ccex::CurrencyPair(base.try_into()?, quote.try_into()?))
	}
}

impl FromStr for CurrencyPair {
	type Err = Error;
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let currencies: Vec<&str> = s.split('_').collect();
		let (base, quote) = (&currencies[0], &currencies[1]);
		let currency_pair = CurrencyPair(base.parse()?, quote.parse()?);
		Ok(currency_pair)
	}
}

impl Display for CurrencyPair {
	fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
		let CurrencyPair(base, quote) = *self;
		let (base, quote) = (base.to_string(), quote.to_string());
		f.write_str([&base, "_", &quote].concat().to_lowercase().as_str())
	}
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct GetDepth {
	pub product: String,
}

#[derive(Debug, PartialEq, PartialOrd, Clone, Deserialize, Serialize)]
pub struct Orderbook {
	pub bids: Vec<(f64, f64)>,
	pub asks: Vec<(f64, f64)>,
}

impl RestResource for GetDepth {
	type Response = HashMap<String, Orderbook>;

	fn method(&self) -> Method {
		Method::Get
	}

	fn path(&self) -> String {
		["/api/3/depth/", &self.product.to_string()].concat()
	}

	fn deserialize(&self, response: &HttpResponse) -> Result<Self::Response, Error> {
		deserialize_public_response(response)
	}
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct GetInfo {
	pub nonce: u32,
}

impl<'a> NeedsAuthentication<&'a Credential> for GetInfo{}
impl<'a> RestResource for PrivateRequest<GetInfo, &'a Credential> {
	type Response = Info;

	fn method(&self) -> Method {
		Method::Post
	}

	fn path(&self) -> String {
		"/tapi".to_owned()
	}

	fn body(&self) -> Result<Option<Payload>, Error> {
		let body = QueryBuilder::with_capacity(2)
			.param("method", "getInfo")
			.param("nonce", self.request.nonce.to_string())
			.build();

		Ok(Some(Payload::Text(body.to_string())))
	}

	fn headers(&self) -> Result<Headers, Error> {
		private_headers(self, &self.credential)
	}

	fn deserialize(&self, response: &HttpResponse) -> Result<Self::Response, Error> {
		deserialize_private_response(response)
	}
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct Info {
	/// Your account balance available for trading. Doesn’t include funds on
	/// your open orders.
	pub funds: HashMap<String, f64>,

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

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct PlaceOrder {
	pub pair: CurrencyPair,
	#[serde(rename="type")] 
	pub side: Side,
	pub rate: d128,
	pub amount: d128,
	pub nonce: u32,
}

impl<'a> NeedsAuthentication<&'a Credential> for PlaceOrder {}
impl<'a> RestResource for PrivateRequest<PlaceOrder, &'a Credential> {
	type Response = OrderPlacement;

	fn method(&self) -> Method {
		Method::Post
	}

	fn path(&self) -> String {
		"/tapi".to_owned()
	}

	fn body(&self) -> Result<Option<Payload>, Error> {
		let body = QueryBuilder::with_capacity(6)
			.param("nonce",	self.request.nonce.to_string())
			.param("method", "trade")
			.param("pair", self.request.pair.to_string())
			.param("type", self.request.side.to_string())
			.param("rate", self.request.rate.to_string())
			.param("amount", self.request.amount.to_string())
			.build();

		Ok(Some(Payload::Text(body.to_string())))
	}

	fn headers(&self) -> Result<Headers, Error> {
		private_headers(self, &self.credential)
	}

	fn deserialize(&self, response: &HttpResponse) -> Result<Self::Response, Error> {
		deserialize_private_response(response)
	}
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
	funds: HashMap<String, f64>,
}

#[derive(Debug, PartialEq, PartialOrd, Clone, Deserialize, Serialize)]
pub struct GetActiveOrders {
	pair: CurrencyPair,
	nonce: u32,
}

#[derive(Debug, PartialEq, PartialOrd, Clone, Deserialize, Serialize)]
pub struct Order {
	pub status: i32,
	pub pair: String,
	#[serde(rename = "type")]
	pub side: Side,
	pub amount: f64,
	pub rate: f64,
	pub timestamp_created: u64,
}

impl<'a> NeedsAuthentication<&'a Credential> for GetActiveOrders {}
impl<'a> RestResource for PrivateRequest<GetActiveOrders, &'a Credential> {
	type Response = HashMap<String, Order>;

	fn method(&self) -> Method {
		Method::Post
	}

	fn path(&self) -> String {
		"/tapi".to_owned()
	}

	fn body(&self) -> Result<Option<Payload>, Error> {
		let body = QueryBuilder::with_capacity(3)
			.param("method", "ActiveOrders")
			.param("nonce", self.request.nonce.to_string())
			.param("pair", self.request.pair.to_string())
			.build();

		Ok(Some(Payload::Text(body.to_string())))
	}

	fn headers(&self) -> Result<Headers, Error> {
		private_headers(self, &self.credential)
	}

	fn deserialize(&self, response: &HttpResponse) -> Result<Self::Response, Error> {
		deserialize_private_response(response)
	}
}

#[derive(Deserialize, Serialize)]
struct ErrorResponse {
	pub success: i64,
	pub error: String,
}

fn deserialize_public_response<T>(response: &HttpResponse) -> Result<T, Error>
where T: DeserializeOwned {
	let response: serde_json::Value = match response.body {
		Some(Payload::Text(ref body)) => serde_json::from_str(body)?,
		Some(Payload::Binary(ref body)) => serde_json::from_slice(body)?,
		None => return Err(format_err!("body is empty")),
	};

	let is_success = response.as_object()
		.and_then(|o| o.get("success"))
		.and_then(|o| o.as_u64())
		.map_or(true, |o| if o == 0 {false} else {true});

	if is_success {
		let response: T = serde_json::from_value(response)?;
		Ok(response)
	} else {
		let response: ErrorResponse = serde_json::from_value(response)?;
		Err(format_err!("The server returned: {}", response.error))
	}
}

fn deserialize_private_response<T>(response: &HttpResponse) -> Result<T, Error> 
where T: DeserializeOwned {
	let response = match response.body {
		Some(Payload::Text(ref body)) => body,
		Some(Payload::Binary(ref body)) => return Err(format_err!("the response body doesn't contain valid utf8 text: {:?}", body)),
		None => return Err(format_err!("the body is empty")),
	};

	let response: PrivateResponse<T> = 
		serde_json::from_str(&response)
		.context(format!("failed to deserialize: \"{}\"", response))?;

	response
		.into_result()
		.map_err(|e| format_err!("the server returned \"{}\"", e))
}

fn private_headers<R>(request: &R, credential: &Credential) -> Result<Headers, Error> 
where R: RestResource {
	let mut mac = Hmac::<Sha512>::new(credential.secret.as_bytes()).map_err(|e| format_err!("{:?}", e))?;
	if let Some(body) = request.body()? {
		mac.input(body.as_bytes());
	}
	let signature = hex::encode(mac.result().code().to_vec());

	let headers = vec![
		Header::new("Key", credential.key.clone()),
		Header::new("Sign", signature),
	];
	Ok(headers)
}

pub fn nonce() -> u32 {
	// TODO: switch to a cached nonce at some point. this has the limitations
	// of 1) only allowing one request per millisecond and 2) expiring after
	// ~50 days
	let now = Utc::now();
	(now.timestamp() as u32 - 1516812776u32) * 1000 + now.timestamp_subsec_millis()
}

#[derive(Debug, Clone)]
pub struct Liqui<Client>
where Client: HttpClient {
    pub credential: Credential,
    pub host: Url,
    pub client: Client,
}


impl<Client> ccex::RestExchange for Liqui<Client>
where Client: HttpClient {
    fn balances(&mut self) -> Result<Vec<ccex::Balance>, Error> {
        let request = GetInfo {
            nonce: nonce(),
        }.authenticate(&self.credential);
        let response = self.client.send(&self.host, request)?;

        let mut balances = Vec::with_capacity(10);
        for (currency, amount) in response.funds {
        	if let Ok(currency) = currency.parse() {
        		balances.push(ccex::Balance::new(currency, amount.try_into()?));
        	}
        }
        Ok(balances)
    }

    fn orderbook(&mut self, product: ccex::CurrencyPair) -> Result<ccex::Orderbook, Error> {
    	let product = CurrencyPair::try_from(product)?.to_string();
	    let request = GetDepth {
	    	product: product.clone()
	    };
	    let mut response = self.client.send(&self.host, request)?;

	    let liqui_orderbook = match response.remove(&product) {
	    	Some(orderbook) => orderbook,
	    	None => panic!(),
	    };

	    let capacity = Ord::max(liqui_orderbook.asks.len(), liqui_orderbook.bids.len());
	    let mut orderbook = ccex::Orderbook::with_capacity(capacity);
	    for (price, amount) in liqui_orderbook.bids.into_iter() {
	    	let price = price.try_into()?;
	    	let amount = amount.try_into()?;
	    	orderbook.add_or_update_bid(ccex::Offer::new(price, amount));
	    }
	    for (price, amount) in liqui_orderbook.asks.into_iter() {
	    	let price = price.try_into()?;
	    	let amount = amount.try_into()?;
	    	orderbook.add_or_update_ask(ccex::Offer::new(price, amount));
	    }
	    Ok(orderbook)
	}

    fn place_order(&mut self, order: ccex::NewOrder) -> Result<ccex::Order, Error> {
    	let (price, quantity) = match order.instruction {
    		ccex::NewOrderInstruction::Limit {price, quantity, ..} => (price, quantity),
    		instruction => unimplemented!("liqui doesn't support {:?}", instruction),
    	};

    	let request = PlaceOrder {
    		pair: order.product.try_into()?,
    		side: order.side.into(),
    		rate: price.clone(),
    		amount: quantity,
    		nonce: nonce(),
    	};
		let request = request.authenticate(&self.credential);
		let response = self.client.send(&self.host, request).unwrap();

		let order = ccex::Order {
			id: Some(order.id),
			server_id: Some(response.order_id.to_string()),
			side: order.side,
			product: order.product,
			status: ccex::OrderStatus::Open,
			instruction: ccex::OrderInstruction::Limit {
				price: price,
				original_quantity: d128::try_from(response.received)? + d128::try_from(response.remains)?,
				remaining_quantity: d128::try_from(response.remains)?,
				time_in_force: ccex::TimeInForce::GoodTillCancelled,
			}
		};
		Ok(order)
    }

    fn orders(&mut self, product: ccex::CurrencyPair) -> Result<Vec<ccex::Order>, Error> {
    	let request = GetActiveOrders {
    		pair: product.try_into()?,
    		nonce: nonce(),
    	};
    	let request = request.authenticate(&self.credential);
    	let response = self.client.send(&self.host, request)?;

    	// let response = match response {
    	// 	serde_json::Value::Object(response) => response,
    	// 	value => panic!("expected a serde_json::Value::Object; but got {:?}", value)
    	// };

    	let mut orders = Vec::with_capacity(response.len());
    	for (id, order) in response.into_iter() {
    		let order = ccex::Order {
    			id: None,
    			server_id: Some(id),
    			side: order.side.into(),
    			product: order.pair.parse::<CurrencyPair>()?.try_into()?,
    			status: ccex::OrderStatus::Open,
    			instruction: ccex::OrderInstruction::Limit {
    				price: order.rate.try_into()?,
    				original_quantity: d128::zero(),
    				remaining_quantity: order.amount.try_into()?,
    				time_in_force: ccex::TimeInForce::GoodTillCancelled,
    			}
    		};
    		orders.push(order);
    	}
    	Ok(orders)
    }
}