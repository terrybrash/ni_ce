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
use failure::{Fail, err_msg, Error, ResultExt};

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct Credential {
	pub key: String,
	pub secret: String,
}

#[derive(Fail, Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub enum CurrencyConversionError {
	#[fail(display = "Unsupported currency: {}", _0)]
	UnsupportedCurrency(String),
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Copy, Deserialize, Serialize)]
pub struct CurrencyPair(Currency, Currency);

impl TryFrom<ccex::CurrencyPair> for CurrencyPair {
	type Error = CurrencyConversionError;
	fn try_from(ccex::CurrencyPair(base, quote): ccex::CurrencyPair) -> Result<Self, Self::Error> {
		Ok(CurrencyPair(base.try_into()?, quote.try_into()?))
	}
}

impl TryFrom<CurrencyPair> for ccex::CurrencyPair {
	type Error = CurrencyConversionError;
	fn try_from(CurrencyPair(base, quote): CurrencyPair) -> Result<Self, Self::Error> {
		Ok(ccex::CurrencyPair(base.try_into()?, quote.try_into()?))
	}
}

impl FromStr for CurrencyPair {
	type Err = ParseCurrencyError;
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		let currencies: Vec<&str> = s.split('_').collect();
		let (base, quote) = (&currencies[0], &currencies[1]);
		let pair = CurrencyPair(base.parse()?, quote.parse()?);
		Ok(pair)
	}
}

impl Display for CurrencyPair {
	fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
		let CurrencyPair(base, quote) = *self;
		let (base, quote) = (base.to_string(), quote.to_string());
		f.write_str([&base, "_", &quote].concat().as_str())
	}
}

#[derive(Debug, Copy, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub enum Currency {
	BCH,
	BTC,
	DASH,
	DOGE,
	ETC,
	ETH,
	EUR,
	KICK,
	LTC,
	PLN,
	RUB,
	UAH,
	USD,
	USDT,
	WAVES,
	XMR,
	XRP,
	ZEC,
}

pub enum ParseCurrencyError {
	/// The currency is either spelled incorrectly, or isn't supported by this
	/// crate; it could be a legitimate currency that needs to be added to the
	/// `Currency` enum.
	InvalidOrUnsupportedCurrency(String),
}

impl FromStr for Currency {
	type Err = ParseCurrencyError;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		const currencies: [(&'static str, Currency); 18] = [
			("BCH", Currency::BCH),
			("BTC", Currency::BTC),
			("DASH", Currency::DASH),
			("DOGE", Currency::DOGE),
			("ETC", Currency::ETC),
			("ETH", Currency::ETH),
			("EUR", Currency::EUR),
			("KICK", Currency::KICK),
			("LTC", Currency::LTC),
			("PLN", Currency::PLN),
			("RUB", Currency::RUB),
			("UAH", Currency::UAH),
			("USD", Currency::USD),
			("USDT", Currency::USDT),
			("WAVES", Currency::WAVES),
			("XMR", Currency::XMR),
			("XRP", Currency::XRP),
			("ZEC", Currency::ZEC),
		];

		for &(string, currency) in &currencies {
			if string.eq_ignore_ascii_case(s) {
				return Ok(currency);
			}
		}
		Err(ParseCurrencyError::InvalidOrUnsupportedCurrency(s.to_owned()))
	}
}

impl Display for Currency {
	fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
		write!(f, "{:?}", self)
	}
}


impl TryFrom<Currency> for ccex::Currency {
	type Error = CurrencyConversionError;

	fn try_from(currency: Currency) -> Result<Self, Self::Error> {
		match currency {
			Currency::BCH => Ok(ccex::Currency::BCH),
			Currency::BTC => Ok(ccex::Currency::BTC),
			Currency::DASH => Ok(ccex::Currency::DASH),
			Currency::DOGE => Ok(ccex::Currency::DOGE),
			Currency::ETC => Ok(ccex::Currency::ETC),
			Currency::ETH => Ok(ccex::Currency::ETH),
			Currency::EUR => Ok(ccex::Currency::EUR),
			Currency::KICK => Ok(ccex::Currency::KICK),
			Currency::LTC => Ok(ccex::Currency::LTC),
			Currency::PLN => Ok(ccex::Currency::PLN),
			Currency::RUB => Ok(ccex::Currency::RUB),
			Currency::UAH => Ok(ccex::Currency::UAH),
			Currency::USD => Ok(ccex::Currency::USD),
			Currency::USDT => Ok(ccex::Currency::USDT),
			Currency::WAVES => Ok(ccex::Currency::WAVES),
			Currency::XMR => Ok(ccex::Currency::XMR),
			Currency::XRP => Ok(ccex::Currency::XRP),
			Currency::ZEC => Ok(ccex::Currency::ZEC),
		    currency => Err(CurrencyConversionError::UnsupportedCurrency(currency.to_string())),
		}
	}
}

impl TryFrom<ccex::Currency> for Currency {
	type Error = CurrencyConversionError;

	fn try_from(currency: ccex::Currency) -> Result<Self, Self::Error> {
		match currency {
			ccex::Currency::BCH => Ok(Currency::BCH),
			ccex::Currency::BTC => Ok(Currency::BTC),
			ccex::Currency::DASH => Ok(Currency::DASH),
			ccex::Currency::DOGE => Ok(Currency::DOGE),
			ccex::Currency::ETC => Ok(Currency::ETC),
			ccex::Currency::ETH => Ok(Currency::ETH),
			ccex::Currency::EUR => Ok(Currency::EUR),
			ccex::Currency::KICK => Ok(Currency::KICK),
			ccex::Currency::LTC => Ok(Currency::LTC),
			ccex::Currency::PLN => Ok(Currency::PLN),
			ccex::Currency::RUB => Ok(Currency::RUB),
			ccex::Currency::UAH => Ok(Currency::UAH),
			ccex::Currency::USD => Ok(Currency::USD),
			ccex::Currency::USDT => Ok(Currency::USDT),
			ccex::Currency::WAVES => Ok(Currency::WAVES),
			ccex::Currency::XMR => Ok(Currency::XMR),
			ccex::Currency::XRP => Ok(Currency::XRP),
			ccex::Currency::ZEC => Ok(Currency::ZEC),
			currency => Err(CurrencyConversionError::UnsupportedCurrency(currency.to_string())),
		}
	}
}

fn private_headers<R>(request: &R, credential: &Credential) -> Result<Headers, Error> 
where R: RestResource {
	let mut mac = Hmac:: <Sha512>::new(credential.secret.as_bytes()).map_err(|e| format_err!("{:?}", e))?;
	match request.body()? {
		Some(Payload::Text(body)) => mac.input(body.as_bytes()),
		Some(Payload::Binary(body)) => mac.input(body.as_slice()),
		None => (),
	}
	let signature = hex::encode(mac.result().code().to_vec());

	let headers = vec![
		Header::new("Content-Length", signature.len().to_string()),
		Header::new("Content-Type", "application/x-www-form-urlencoded"),
		Header::new("Key", credential.key.clone()),
		Header::new("Sign", signature),
	];
	Ok(headers)
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
struct ErrorResponse {
	pub result: bool,
	pub error: String,
}

fn deserialize_private_response<T>(response: &HttpResponse) -> Result<T, Error> 
where T: DeserializeOwned {
	let body = match response.body {
		Some(Payload::Text(ref body)) => body,
		Some(Payload::Binary(ref body)) => panic!("expected text"),
		None => panic!(),
		// None => Err(format_err!("the body is empty"))?,
	};
	let response: serde_json::Value = serde_json::from_str(body)?;
	let is_error = response.as_object().map(|object|
		match object.get("result") {
			Some(&serde_json::Value::Bool(result)) => !result,
			_ => false,
		}).unwrap_or(false);

	if is_error {
		let error: ErrorResponse = serde_json::from_value(response)
			.with_context(|_| format!("failed to deserialize: \"{}\"", body))?;
		Err(format_err!("Server returned: {}", error.error))
	} else {
		let response = 
			serde_json::from_value(response)
			.context(format!("failed to deserialize: \"{}\"", body))?;
		Ok(response)
	}
}

fn deserialize_public_response<T>(response: &HttpResponse) -> Result<T, Error>
where T: DeserializeOwned {
	match response.body {
		Some(Payload::Text(ref body)) => Ok(serde_json::from_str(body)?),
		Some(Payload::Binary(ref body)) => Ok(serde_json::from_slice(body)?),
		None => panic!(),
	}
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct GetOrderbook {
	pub products: Vec<CurrencyPair>,
	pub limit: usize,
}

#[derive(Debug, Deserialize)]
pub struct Orderbook {
	pub ask_quantity: d128,
	pub ask_amount: d128,
	pub ask_top: d128,
	pub bid_quantity: d128,
	pub bid_amount: d128,
	pub bid_top: d128,
	pub ask: Vec<(d128, d128, d128)>,
	pub bid: Vec<(d128, d128, d128)>,
}

impl RestResource for GetOrderbook {
	type Response = HashMap<String, Orderbook>;

	fn method(&self) -> Method {
		Method::Get
	}

	fn query(&self) -> Query {
		let products: Vec<String> = self.products.iter().map(ToString::to_string).collect();
		let products = products.as_slice().join(",");

		QueryBuilder::with_capacity(2)
			.param("pair", products)
			.param("limit", self.limit.to_string())
			.build()
	}

	fn path(&self) -> String {
		"/v1/order_book".to_owned()
	}

	fn deserialize(&self, response: &HttpResponse) -> Result<Self::Response, Error> {
		deserialize_public_response(response)
	}
}


#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct GetUserInfo {
	pub nonce: u32,
}

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
pub struct UserInfo {
	pub uid: i64,
	pub server_date: u64,
	pub balances: HashMap<String, d128>,
	pub reserved: HashMap<String, d128>,
}

impl<'a> NeedsAuthentication<&'a Credential> for GetUserInfo {}
impl<'a> RestResource for PrivateRequest<GetUserInfo, &'a Credential> {
	type Response = UserInfo;

	fn method(&self) -> Method {
		Method::Post
	}

	fn path(&self) -> String {
		"/v1/user_info".to_string()
	}

	fn headers(&self) -> Result<Headers, Error> {
		private_headers(self, &self.credential)
	}

	fn body(&self) -> Result<Option<Payload>, Error> {
		let query = self.query().to_string().trim_left_matches("?").to_owned();
		Ok(Some(Payload::Text(query)))
	}

	fn query(&self) -> Query {
		QueryBuilder::with_capacity(3)
			.param("nonce", self.request.nonce.to_string())
			.build()
	}

	fn deserialize(&self, response: &HttpResponse) -> Result<Self::Response, Error> {
		deserialize_private_response(response)
	}
}

#[derive(Debug, PartialEq, Eq, Copy, Hash, PartialOrd, Ord, Clone, Deserialize, Serialize)]
pub enum PlaceOrderInstruction {
	LimitBuy,
	LimitSell,
	MarketBuy,
	MarketSell,
	MarketBuyTotal,
	MarketSellTotal,
}

impl Display for PlaceOrderInstruction {
	fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
		match *self {
			PlaceOrderInstruction::LimitBuy => f.write_str("buy"),
			PlaceOrderInstruction::LimitSell => f.write_str("sell"),
			PlaceOrderInstruction::MarketBuy => f.write_str("market_buy"),
			PlaceOrderInstruction::MarketSell => f.write_str("market_sell"),
			PlaceOrderInstruction::MarketBuyTotal => f.write_str("market_buy_total"),
			PlaceOrderInstruction::MarketSellTotal => f.write_str("market_sell_total"),
		}
	}
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct PlaceOrder {
	pub pair: CurrencyPair,
	pub quantity: d128,
	pub price: d128,
	pub instruction: PlaceOrderInstruction,
	pub nonce: u32,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct Order {
	pub order_id: i64,
}

impl<'a> NeedsAuthentication<&'a Credential> for PlaceOrder {}
impl<'a> RestResource for PrivateRequest<PlaceOrder, &'a Credential> {
	type Response = Order;

	fn method(&self) -> Method {
		Method::Post
	}

	fn path(&self) -> String {
		"/v1/order_create".to_string()
	}

	fn headers(&self) -> Result<Headers, Error> {
		private_headers(self, &self.credential)
	}

	fn body(&self) -> Result<Option<Payload>, Error> {
		let query = self.query().to_string().trim_left_matches("?").to_owned();
		Ok(Some(Payload::Text(query)))
	}

	fn query(&self) -> Query {
		QueryBuilder::with_capacity(5)
			.param("nonce", self.request.nonce.to_string())
			.param("pair", self.request.pair.to_string())
			.param("quantity", self.request.quantity.to_string())
			.param("price", self.request.price.to_string())
			.param("type", self.request.instruction.to_string())
			.build()
	}

	fn deserialize(&self, response: &HttpResponse) -> Result<Self::Response, Error> {
		deserialize_private_response(response)
	}

}

pub fn nonce() -> u32 {
	// TODO: switch to a cached nonce at some point. this has the limitations
	// of 1) only allowing one request per millisecond and 2) expiring after
	// ~50 days
	// let now = Utc::now();
	// (now.timestamp() as u32 - 1517298754u32) * 1000 + now.timestamp_subsec_millis()
	Utc::now().timestamp() as u32
}

#[derive(Debug, Clone)]
pub struct Exmo<Client>
where Client: HttpClient {
	pub credential: Credential,
	pub host: Url,
	pub client: Client,
}

impl<Client> ccex::RestExchange for Exmo<Client>
where Client: HttpClient {
	fn balances(&mut self) -> Result<Vec<ccex::Balance>, Error> {
		let request = GetUserInfo {
			nonce: nonce(),
		}.authenticate(&self.credential);
		let response = self.client.send(&self.host, request)?;

		let balances = response.balances.into_iter()
			.filter_map(|(currency, balance)| {
				match currency.parse::<Currency>() {
					Ok(currency) => Some((currency, balance)),
					Err(ParseCurrencyError::InvalidOrUnsupportedCurrency(currency)) => None,
				}
			}).filter_map(|(currency, balance)| {
				match ccex::Currency::try_from(currency) {
					Ok(currency) => Some((currency, balance)),
					Err(CurrencyConversionError::UnsupportedCurrency(currency)) => None,
				}
			}).map(|(currency, balance)| ccex::Balance::new(currency, balance))
			.collect();
		Ok(balances)
	}

	fn place_order(&mut self, order: ccex::NewOrder) -> Result<ccex::Order, Error> {
		let (price, quantity) = match order.instruction {
			ccex::NewOrderInstruction::Limit {price, quantity, ..} => (price, quantity),
			_ => return Err(err_msg("only limit orders are supported on exmo")),
		};

		let request = PlaceOrder {
			nonce: nonce(),
			pair: order.product.try_into()?,
			quantity: quantity,
			price: price,
			instruction: match order.side {
				ccex::Side::Ask => PlaceOrderInstruction::LimitSell,
				ccex::Side::Bid => PlaceOrderInstruction::LimitBuy,
			},
		}.authenticate(&self.credential);
		let response = self.client.send(&self.host, request)?;
		Ok(order.into())
	}

    fn orders(&mut self, product: ccex::CurrencyPair) -> Result<Vec<ccex::Order>, Error> {
    	unimplemented!();
    }

    fn orderbook(&mut self, product: ccex::CurrencyPair) -> Result<ccex::Orderbook, Error> {
    	// exmo has the capability to query multiple orderbooks in one
    	// request, but for now we're only doing single-orderbook requests
    	let request = GetOrderbook {
    		products: vec![product.try_into()?],
    		limit: 100,
    	};
    	let response = self.client.send(&self.host, request)?;

    	let p = product;
    	for (product, exmo_orderbook) in response.into_iter() {
    		let product = match product.parse::<CurrencyPair>() {
    			Ok(product) => product,
    			Err(_) => continue,
    		};
    		let product = match ccex::CurrencyPair::try_from(product) {
    			Ok(product) => product,
    			Err(_) => continue,
    		};

    		if product != p {
    			continue;
    		}

    		let capacity = Ord::max(exmo_orderbook.ask.len(), exmo_orderbook.bid.len());
    		let mut orderbook = ccex::Orderbook::with_capacity(capacity);
    		for (price, amount, _) in exmo_orderbook.ask.into_iter() {
    			orderbook.add_or_update_ask(ccex::Offer::new(price, amount));
    		}
    		for (price, amount, _) in exmo_orderbook.bid.into_iter() {
    			orderbook.add_or_update_bid(ccex::Offer::new(price, amount));
    		}
    		return Ok(orderbook);
    	}

    	Err(format_err!("no orderbook"))
    }
}