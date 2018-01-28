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
pub enum Product {
	btc_usdt,
	eth_btc,
	eth_usdt,
}

impl Display for Product {
	fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
		write!(f, "{:?}", self)
	}
}

fn from_string(string: &str) -> ccex::CurrencyPair {
	let currencies = string.split('_').map(str::to_uppercase).collect();
	let (base, quote) = (&currencies[0], &currencies[1]);
	(base.parse().unwrap(), quote.parse().unwrap())
}

fn into_string((base, quote): ccex::CurrencyPair) -> String {
	let (base, quote) = (base.to_string(), quote.to_string());
	[&base, "_", &quote].concat().to_lowercase()
}

impl From<ccex::CurrencyPair> for Product {
	fn from(pair: ccex::CurrencyPair) -> Self {
		match pair {
	        (ccex::Currency::BTC, ccex::Currency::USDT)	=> Product::btc_usdt,
	        (ccex::Currency::ETH, ccex::Currency::BTC) 	=> Product::eth_btc,
	        (ccex::Currency::ETH, ccex::Currency::USDT) => Product::eth_usdt,
	        product => unimplemented!("{:?}", product),	
		}
	}
}

impl From<Product> for ccex::CurrencyPair {
	fn from(pair: Product) -> Self {
		match pair {
	        Product::btc_usdt	=> (ccex::Currency::BTC, ccex::Currency::USDT),
	        Product::eth_btc 	=> (ccex::Currency::ETH, ccex::Currency::BTC),
	        Product::eth_usdt 	=> (ccex::Currency::ETH, ccex::Currency::USDT),
	        product => unimplemented!("{:?}", product),	
		}
	}
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct GetDepth {
	pub product: Product,
}

impl RestResource for GetDepth {
	type Response = Depth;

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


#[derive(Debug, PartialEq, PartialOrd, Clone, Deserialize, Serialize)]
pub struct Depth {
	pub eth_btc: Option<Orderbook>,
	pub btc_usdt: Option<Orderbook>,
	pub eth_usdt: Option<Orderbook>,
}

#[derive(Debug, PartialEq, PartialOrd, Clone, Deserialize, Serialize)]
pub struct Orderbook {
	pub bids: Vec<(f64, f64)>,
	pub asks: Vec<(f64, f64)>,
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

#[derive(Debug, PartialEq, PartialOrd, Clone, Deserialize, Serialize)]
pub struct Info {
	/// Your account balance available for trading. Doesn’t include funds on
	/// your open orders.
	pub funds: Funds,

	/// The privileges of the current API key. At this time the privilege to
	/// withdraw is not used anywhere.
	pub rights: Rights,

	/// The number of your open orders.
	pub open_orders: i64,

	/// Server time (UTC).
	pub server_time: i64,
}

#[derive(Debug, PartialEq, PartialOrd, Clone, Deserialize, Serialize)]
pub struct Funds {
	pub eth: f64,
	pub btc: f64,
	pub ltc: f64,
	pub bcc: f64,
	pub usdt: f64,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct Rights {
	pub info: bool,
	pub trade: bool,
	pub withdraw: bool,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct PlaceOrder {
	pub pair: Product,
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

#[derive(Debug, PartialEq, PartialOrd, Clone, Deserialize, Serialize)]
pub struct OrderPlacement {
	/// The amount of currency bought/sold.
	received: d128,

	/// The remaining amount of currency to be bought/sold (and the initial
	/// order amount).
	remains: d128,

	/// Is equal to 0 if the request was fully “matched” by the opposite
	/// orders, otherwise the ID of the executed order will be returned.
	order_id: i64,

	/// Balance after the request.
	funds: Funds,
}

#[derive(Debug, PartialEq, PartialOrd, Clone, Deserialize, Serialize)]
pub struct GetActiveOrders {
	pair: Product,
	nonce: u32,
}

impl<'a> NeedsAuthentication<&'a Credential> for GetActiveOrders {}
impl<'a> RestResource for PrivateRequest<GetActiveOrders, &'a Credential> {
	type Response = serde_json::Value;

	fn method(&self) -> Method {
		Method::Post
	}

	fn path(&self) -> String {
		"/tapi".to_owned()
	}

	fn body(&self) -> Result<Option<Payload>, Error> {
		let body = QueryBuilder::with_capacity(3)
			.param("nonce", self.request.nonce.to_string())
			.param("method", "ActiveOrders")
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

#[derive(Debug, PartialEq, PartialOrd, Clone, Deserialize, Serialize)]
pub struct Order {
	pub status: i32,
	pub pair: Product,
	#[serde(rename = "type")]
	pub side: Side,
	pub amount: f64,
	pub rate: f64,
	pub timestamp_created: u64,
}

fn deserialize_public_response<T>(response: &HttpResponse) -> Result<T, Error>
where T: DeserializeOwned {
	match response.body {
		Some(Payload::Text(ref body)) => Ok(serde_json::from_str(body)?),
		Some(Payload::Binary(ref body)) => Ok(serde_json::from_slice(body)?),
		None => Err(format_err!("body is empty")),
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

fn d128_from_f64(float: f64) -> d128 {
    use std::str::{FromStr};
    d128::from_str(&float.to_string()).unwrap()
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
    fn balances(&mut self) -> Vec<ccex::Balance> {
        let request = GetInfo {
            nonce: nonce(),
        }.authenticate(&self.credential);
        let response = self.client.send(&self.host, request).unwrap();

        vec![
            ccex::Balance::new(ccex::Currency::BTC, d128_from_f64(response.funds.btc)),
            ccex::Balance::new(ccex::Currency::ETH, d128_from_f64(response.funds.eth)),
            ccex::Balance::new(ccex::Currency::BCH, d128_from_f64(response.funds.bcc)),
            ccex::Balance::new(ccex::Currency::USDT, d128_from_f64(response.funds.usdt)),
        ]
    }

    fn orderbook(&mut self, product: ccex::CurrencyPair) -> ccex::Orderbook {
    	let product = product.into();
	    let request = GetDepth {product};
	    let response = self.client.send(&self.host, request).unwrap();

	    let orderbook = match product {
	    	Product::btc_usdt => response.btc_usdt.unwrap(),
	    	Product::eth_usdt => response.eth_usdt.unwrap(),
	    	Product::eth_btc => response.eth_btc.unwrap(),
	    };

	    let bids = orderbook.bids.iter().map(|&(price, amount)| {
	        let price = d128::from_str(&price.to_string()).unwrap();
	        let amount = d128::from_str(&amount.to_string()).unwrap();
	        ccex::Offer::new(price, amount)
	    }).collect();

	    let asks = orderbook.asks.iter().map(|&(price, amount)| {
	        let price = d128::from_str(&price.to_string()).unwrap();
	        let amount = d128::from_str(&amount.to_string()).unwrap();
	        ccex::Offer::new(price, amount)
	    }).collect();

	    ccex::Orderbook {
	        bids,
	        asks,
	    }
	}

    fn place_order(&mut self, order: ccex::NewOrder) -> ccex::Order {
    	let (price, quantity) = match order.instruction {
    		ccex::NewOrderInstruction::Limit {price, quantity, ..} => (price, quantity),
    		instruction => unimplemented!("liqui doesn't support {:?}", instruction),
    	};

    	let request = PlaceOrder {
    		pair: order.product.into(),
    		side: order.side.into(),
    		rate: price.clone(),
    		amount: quantity,
    		nonce: nonce(),
    	};
		let request = request.authenticate(&self.credential);
		let response = self.client.send(&self.host, request).unwrap();

		ccex::Order {
			id: Some(order.id),
			server_id: Some(response.order_id.to_string()),
			side: order.side,
			product: order.product,
			status: ccex::OrderStatus::Open,
			instruction: ccex::OrderInstruction::Limit {
				price: price,
				original_quantity: response.received + response.remains,
				remaining_quantity: response.remains,
				time_in_force: ccex::TimeInForce::GoodTillCancelled,
			}
		}
    }

    fn orders(&mut self, product: ccex::CurrencyPair) -> Vec<ccex::Order> {
    	let request = GetActiveOrders {
    		pair: product.into(),
    		nonce: nonce(),
    	};
    	let request = request.authenticate(&self.credential);
    	let response = self.client.send(&self.host, request).unwrap();

    	let response = match response {
    		serde_json::Value::Object(response) => response,
    		value => panic!("expected a serde_json::Value::Object; but got {:?}", value)
    	};

    	response.into_iter().map(|(key, value)| {
    		let order: Order = serde_json::from_value(value).unwrap();
    		ccex::Order {
    			id: None,
    			server_id: Some(key),
    			side: order.side.into(),
    			product: order.pair.into(),
    			status: ccex::OrderStatus::Open,
    			instruction: ccex::OrderInstruction::Limit {
    				price: d128_from_f64(order.rate),
    				original_quantity: d128::zero(),
    				remaining_quantity: d128_from_f64(order.amount),
    				time_in_force: ccex::TimeInForce::GoodTillCancelled,
    			}
    		}
    	}).collect()
    }
}