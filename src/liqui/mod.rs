use crate as ccex;
use {Exchange, AsyncExchangeRestClient, SyncExchangeRestClient, Future, dual_channel};

use api::{
	Header,
	Headers,
	HttpClient,
	HttpResponse,
	Method,
	Query,
	NeedsAuthentication,
	Payload,
	PrivateRequest,
	QueryBuilder,
	RestResource,
};
use chrono::{Utc};
use failure::{Error, ResultExt};
use hex;
use hmac::{Hmac, Mac};
use num_traits::*;
use rust_decimal::Decimal as d128;
use serde::de::{DeserializeOwned};
use serde_json;
use sha2::{Sha512};
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::fmt::{self, Display, Formatter};
use std::str::{FromStr};
use std::time::Duration;
use url::Url;
use std::cell::RefCell;
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::ops::Deref;

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
	ADX,
	AE,
	AION,
	ANS,
	ANT,
	AST,
	BAT,
	BCAP,
	BCC,
	BMC,
	BNT,
	BTC,
	CFI,
	CVC,
	DASH,
	DCT,
	DGD,
	DNT,
	EDG,
	ENG,
	EOS,
	ETH,
	GBG,
	GNO,
	GNT,
	GOLOS,
	GUP,
	HMQ,
	ICN,
	INCNT,
	IND,
	INS,
	KNC,
	LTC,
	LUN,
	MANA,
	MCO,
	MGO,
	MLN,
	MYST,
	NET,
	NEU,
	OAX,
	OMG,
	PAY,
	PLU,
	PRO,
	PTOY,
	QRL,
	QTUM,
	REP,
	REQ,
	RLC,
	ROUND,
	SALT,
	SAN,
	SBD,
	SNGLS,
	SNM,
	SNT,
	SRN,
	STEEM,
	STORJ,
	STX,
	TAAS,
	TIME,
	TKN,
	TNT,
	TRST,
	TRX,
	USDT,
	VEN,
	VSL,
	WAVES,
	WINGS,
	XID,
	XXX,
	XZC,
	ZRX,
}

impl TryFrom<ccex::Currency> for Currency {
	type Error = Error;
	fn try_from(currency: ccex::Currency) -> Result<Self, Self::Error> {
		match currency {
			ccex::Currency::ADX => Ok(Currency::ADX),
			ccex::Currency::AE => Ok(Currency::AE),
			ccex::Currency::AION => Ok(Currency::AION),
			ccex::Currency::ANS => Ok(Currency::ANS),
			ccex::Currency::ANT => Ok(Currency::ANT),
			ccex::Currency::AST => Ok(Currency::AST),
			ccex::Currency::BAT => Ok(Currency::BAT),
			ccex::Currency::BCAP => Ok(Currency::BCAP),
			ccex::Currency::BCH => Ok(Currency::BCC),
			ccex::Currency::BMC => Ok(Currency::BMC),
			ccex::Currency::BNT => Ok(Currency::BNT),
			ccex::Currency::BTC => Ok(Currency::BTC),
			ccex::Currency::CFI => Ok(Currency::CFI),
			ccex::Currency::CVC => Ok(Currency::CVC),
			ccex::Currency::DASH => Ok(Currency::DASH),
			ccex::Currency::DCT => Ok(Currency::DCT),
			ccex::Currency::DGD => Ok(Currency::DGD),
			ccex::Currency::DNT => Ok(Currency::DNT),
			ccex::Currency::EDG => Ok(Currency::EDG),
			ccex::Currency::ENG => Ok(Currency::ENG),
			ccex::Currency::EOS => Ok(Currency::EOS),
			ccex::Currency::ETH => Ok(Currency::ETH),
			ccex::Currency::GBG => Ok(Currency::GBG),
			ccex::Currency::GNO => Ok(Currency::GNO),
			ccex::Currency::GNT => Ok(Currency::GNT),
			ccex::Currency::GOLOS => Ok(Currency::GOLOS),
			ccex::Currency::GUP => Ok(Currency::GUP),
			ccex::Currency::HMQ => Ok(Currency::HMQ),
			ccex::Currency::ICN => Ok(Currency::ICN),
			ccex::Currency::INCNT => Ok(Currency::INCNT),
			ccex::Currency::IND => Ok(Currency::IND),
			ccex::Currency::INS => Ok(Currency::INS),
			ccex::Currency::KNC => Ok(Currency::KNC),
			ccex::Currency::LTC => Ok(Currency::LTC),
			ccex::Currency::LUN => Ok(Currency::LUN),
			ccex::Currency::MANA => Ok(Currency::MANA),
			ccex::Currency::MCO => Ok(Currency::MCO),
			ccex::Currency::MGO => Ok(Currency::MGO),
			ccex::Currency::MLN => Ok(Currency::MLN),
			ccex::Currency::MYST => Ok(Currency::MYST),
			ccex::Currency::NET => Ok(Currency::NET),
			ccex::Currency::NEU => Ok(Currency::NEU),
			ccex::Currency::OAX => Ok(Currency::OAX),
			ccex::Currency::OMG => Ok(Currency::OMG),
			ccex::Currency::PAY => Ok(Currency::PAY),
			ccex::Currency::PLU => Ok(Currency::PLU),
			ccex::Currency::PRO => Ok(Currency::PRO),
			ccex::Currency::PTOY => Ok(Currency::PTOY),
			ccex::Currency::QRL => Ok(Currency::QRL),
			ccex::Currency::QTUM => Ok(Currency::QTUM),
			ccex::Currency::REP => Ok(Currency::REP),
			ccex::Currency::REQ => Ok(Currency::REQ),
			ccex::Currency::RLC => Ok(Currency::RLC),
			ccex::Currency::ROUND => Ok(Currency::ROUND),
			ccex::Currency::SALT => Ok(Currency::SALT),
			ccex::Currency::SAN => Ok(Currency::SAN),
			ccex::Currency::SBD => Ok(Currency::SBD),
			ccex::Currency::SNGLS => Ok(Currency::SNGLS),
			ccex::Currency::SNM => Ok(Currency::SNM),
			ccex::Currency::SNT => Ok(Currency::SNT),
			ccex::Currency::SRN => Ok(Currency::SRN),
			ccex::Currency::STEEM => Ok(Currency::STEEM),
			ccex::Currency::STORJ => Ok(Currency::STORJ),
			ccex::Currency::STX => Ok(Currency::STX),
			ccex::Currency::TAAS => Ok(Currency::TAAS),
			ccex::Currency::TIME => Ok(Currency::TIME),
			ccex::Currency::TKN => Ok(Currency::TKN),
			ccex::Currency::TNT => Ok(Currency::TNT),
			ccex::Currency::TRST => Ok(Currency::TRST),
			ccex::Currency::TRX => Ok(Currency::TRX),
			ccex::Currency::USDT => Ok(Currency::USDT),
			ccex::Currency::VEN => Ok(Currency::VEN),
			ccex::Currency::VSL => Ok(Currency::VSL),
			ccex::Currency::WAVES => Ok(Currency::WAVES),
			ccex::Currency::WINGS => Ok(Currency::WINGS),
			ccex::Currency::XID => Ok(Currency::XID),
			ccex::Currency::XXX => Ok(Currency::XXX),
			ccex::Currency::XZC => Ok(Currency::XZC),
			ccex::Currency::ZRX => Ok(Currency::ZRX),
			currency => Err(format_err!("{} isn't supported", currency))
		}
	}
}

impl From<Currency> for ccex::Currency {
	fn from(currency: Currency) -> Self {
		match currency {
			Currency::ADX => ccex::Currency::ADX,
			Currency::AE => ccex::Currency::AE,
			Currency::AION => ccex::Currency::AION,
			Currency::ANS => ccex::Currency::ANS,
			Currency::ANT => ccex::Currency::ANT,
			Currency::AST => ccex::Currency::AST,
			Currency::BAT => ccex::Currency::BAT,
			Currency::BCAP => ccex::Currency::BCAP,
			Currency::BCC => ccex::Currency::BCH,
			Currency::BMC => ccex::Currency::BMC,
			Currency::BNT => ccex::Currency::BNT,
			Currency::BTC => ccex::Currency::BTC,
			Currency::CFI => ccex::Currency::CFI,
			Currency::CVC => ccex::Currency::CVC,
			Currency::DASH => ccex::Currency::DASH,
			Currency::DCT => ccex::Currency::DCT,
			Currency::DGD => ccex::Currency::DGD,
			Currency::DNT => ccex::Currency::DNT,
			Currency::EDG => ccex::Currency::EDG,
			Currency::ENG => ccex::Currency::ENG,
			Currency::EOS => ccex::Currency::EOS,
			Currency::ETH => ccex::Currency::ETH,
			Currency::GBG => ccex::Currency::GBG,
			Currency::GNO => ccex::Currency::GNO,
			Currency::GNT => ccex::Currency::GNT,
			Currency::GOLOS => ccex::Currency::GOLOS,
			Currency::GUP => ccex::Currency::GUP,
			Currency::HMQ => ccex::Currency::HMQ,
			Currency::ICN => ccex::Currency::ICN,
			Currency::INCNT => ccex::Currency::INCNT,
			Currency::IND => ccex::Currency::IND,
			Currency::INS => ccex::Currency::INS,
			Currency::KNC => ccex::Currency::KNC,
			Currency::LTC => ccex::Currency::LTC,
			Currency::LUN => ccex::Currency::LUN,
			Currency::MANA => ccex::Currency::MANA,
			Currency::MCO => ccex::Currency::MCO,
			Currency::MGO => ccex::Currency::MGO,
			Currency::MLN => ccex::Currency::MLN,
			Currency::MYST => ccex::Currency::MYST,
			Currency::NET => ccex::Currency::NET,
			Currency::NEU => ccex::Currency::NEU,
			Currency::OAX => ccex::Currency::OAX,
			Currency::OMG => ccex::Currency::OMG,
			Currency::PAY => ccex::Currency::PAY,
			Currency::PLU => ccex::Currency::PLU,
			Currency::PRO => ccex::Currency::PRO,
			Currency::PTOY => ccex::Currency::PTOY,
			Currency::QRL => ccex::Currency::QRL,
			Currency::QTUM => ccex::Currency::QTUM,
			Currency::REP => ccex::Currency::REP,
			Currency::REQ => ccex::Currency::REQ,
			Currency::RLC => ccex::Currency::RLC,
			Currency::ROUND => ccex::Currency::ROUND,
			Currency::SALT => ccex::Currency::SALT,
			Currency::SAN => ccex::Currency::SAN,
			Currency::SBD => ccex::Currency::SBD,
			Currency::SNGLS => ccex::Currency::SNGLS,
			Currency::SNM => ccex::Currency::SNM,
			Currency::SNT => ccex::Currency::SNT,
			Currency::SRN => ccex::Currency::SRN,
			Currency::STEEM => ccex::Currency::STEEM,
			Currency::STORJ => ccex::Currency::STORJ,
			Currency::STX => ccex::Currency::STX,
			Currency::TAAS => ccex::Currency::TAAS,
			Currency::TIME => ccex::Currency::TIME,
			Currency::TKN => ccex::Currency::TKN,
			Currency::TNT => ccex::Currency::TNT,
			Currency::TRST => ccex::Currency::TRST,
			Currency::TRX => ccex::Currency::TRX,
			Currency::USDT => ccex::Currency::USDT,
			Currency::VEN => ccex::Currency::VEN,
			Currency::VSL => ccex::Currency::VSL,
			Currency::WAVES => ccex::Currency::WAVES,
			Currency::WINGS => ccex::Currency::WINGS,
			Currency::XID => ccex::Currency::XID,
			Currency::XXX => ccex::Currency::XXX,
			Currency::XZC => ccex::Currency::XZC,
			Currency::ZRX => ccex::Currency::ZRX,
		}
	}
}

impl FromStr for Currency {
	type Err = Error;
	fn from_str(s: &str) -> Result<Self, Self::Err> {
		const CURRENCIES: [(&'static str, Currency); 79] = [
			("ADX", Currency::ADX),
			("AE", Currency::AE),
			("AION", Currency::AION),
			("ANS", Currency::ANS),
			("ANT", Currency::ANT),
			("AST", Currency::AST),
			("BAT", Currency::BAT),
			("BCAP", Currency::BCAP),
			("BCC", Currency::BCC),
			("BMC", Currency::BMC),
			("BNT", Currency::BNT),
			("BTC", Currency::BTC),
			("CFI", Currency::CFI),
			("CVC", Currency::CVC),
			("DASH", Currency::DASH),
			("DCT", Currency::DCT),
			("DGD", Currency::DGD),
			("DNT", Currency::DNT),
			("EDG", Currency::EDG),
			("ENG", Currency::ENG),
			("EOS", Currency::EOS),
			("ETH", Currency::ETH),
			("GBG", Currency::GBG),
			("GNO", Currency::GNO),
			("GNT", Currency::GNT),
			("GOLOS", Currency::GOLOS),
			("GUP", Currency::GUP),
			("HMQ", Currency::HMQ),
			("ICN", Currency::ICN),
			("INCNT", Currency::INCNT),
			("IND", Currency::IND),
			("INS", Currency::INS),
			("KNC", Currency::KNC),
			("LTC", Currency::LTC),
			("LUN", Currency::LUN),
			("MANA", Currency::MANA),
			("MCO", Currency::MCO),
			("MGO", Currency::MGO),
			("MLN", Currency::MLN),
			("MYST", Currency::MYST),
			("NET", Currency::NET),
			("NEU", Currency::NEU),
			("OAX", Currency::OAX),
			("OMG", Currency::OMG),
			("PAY", Currency::PAY),
			("PLU", Currency::PLU),
			("PRO", Currency::PRO),
			("PTOY", Currency::PTOY),
			("QRL", Currency::QRL),
			("QTUM", Currency::QTUM),
			("REP", Currency::REP),
			("REQ", Currency::REQ),
			("RLC", Currency::RLC),
			("ROUND", Currency::ROUND),
			("SALT", Currency::SALT),
			("SAN", Currency::SAN),
			("SBD", Currency::SBD),
			("SNGLS", Currency::SNGLS),
			("SNM", Currency::SNM),
			("SNT", Currency::SNT),
			("SRN", Currency::SRN),
			("STEEM", Currency::STEEM),
			("STORJ", Currency::STORJ),
			("STX", Currency::STX),
			("TAAS", Currency::TAAS),
			("TIME", Currency::TIME),
			("TKN", Currency::TKN),
			("TNT", Currency::TNT),
			("TRST", Currency::TRST),
			("TRX", Currency::TRX),
			("USDT", Currency::USDT),
			("VEN", Currency::VEN),
			("VSL", Currency::VSL),
			("WAVES", Currency::WAVES),
			("WINGS", Currency::WINGS),
			("XID", Currency::XID),
			("XXX", Currency::XXX),
			("XZC", Currency::XZC),
			("ZRX", Currency::ZRX),
        ];

        for &(string, currency) in CURRENCIES.iter() {
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

// type CurrencyString = String;
// type CurrencyPairString = String;

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct CurrencyString(String);

impl From<Currency> for CurrencyString {
	fn from(currency: Currency) -> CurrencyString {
		CurrencyString(currency.to_string())
	}
}

impl TryFrom<CurrencyString> for Currency {
	type Error = Error;
	fn try_from(CurrencyString(string): CurrencyString) -> Result<Self, Self::Error> {
		string.parse()
	}
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct CurrencyPairString(String);

impl From<CurrencyPair> for CurrencyPairString {
	fn from(pair: CurrencyPair) -> Self {
		CurrencyPairString(pair.to_string())
	}
}

impl TryFrom<CurrencyPairString> for CurrencyPair {
	type Error = Error;
	fn try_from(CurrencyPairString(string): CurrencyPairString) -> Result<Self, Self::Error> {
		string.parse()
	}
}

impl Deref for CurrencyPairString {
	type Target = str;
	fn deref(&self) -> &str {
		&self.0
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

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone)]
pub struct GetDepth<'a> {
	pub product: &'a CurrencyPairString,
}

#[derive(Debug, PartialEq, PartialOrd, Clone, Deserialize, Serialize)]
pub struct Orderbook {
	pub bids: Vec<(f64, f64)>,
	pub asks: Vec<(f64, f64)>,
}

impl<'a> RestResource for GetDepth<'a> {
	type Response = HashMap<CurrencyPairString, Orderbook>;

	fn method(&self) -> Method {
		Method::Get
	}

	fn path(&self) -> String {
		["/api/3/depth/", &self.product].concat()
	}

	fn deserialize(&self, response: &HttpResponse) -> Result<Self::Response, Error> {
		deserialize_public_response(response)
	}
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct GetInfo {
	pub nonce: u32,
}

#[derive(Debug, PartialEq, Clone, Deserialize, Serialize)]
pub struct Info {
	/// Your account balance available for trading. Doesn’t include funds on
	/// your open orders.
	pub funds: HashMap<CurrencyString, f64>,

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

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct PlaceOrder {
	pub pair: CurrencyPair,
	#[serde(rename="type")] 
	pub side: Side,
	pub rate: d128,
	pub amount: d128,
	pub nonce: u32,
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
	funds: HashMap<CurrencyString, f64>,
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

pub type OrderId = String;

#[derive(Debug, PartialEq, PartialOrd, Clone, Deserialize, Serialize)]
pub struct GetActiveOrders {
	pair: CurrencyPair,
	nonce: u32,
}

#[derive(Debug, PartialEq, PartialOrd, Clone, Deserialize, Serialize)]
pub struct Order {
	pub status: i32,
	pub pair: CurrencyPairString,
	#[serde(rename = "type")]
	pub side: Side,
	pub amount: f64,
	pub rate: f64,
	pub timestamp_created: u64,
}

impl<'a> NeedsAuthentication<&'a Credential> for GetActiveOrders {}
impl<'a> RestResource for PrivateRequest<GetActiveOrders, &'a Credential> {
	type Response = HashMap<OrderId, Order>;

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
		.and_then(|obj| obj.get("success"))
		.and_then(|obj| obj.as_u64())
		.map_or(true, |obj| if obj == 0 {false} else {true});

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
		.with_context(|e| format!("failed to deserialize: \"{}\"", response))?;

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
pub struct Liqui {
    pub credential: Credential,
}

impl Liqui {
    pub fn new(credential: Credential) -> Self {
        Liqui {
            credential,
        }
    }
}

impl<Client> Exchange<Client> for Liqui
where Client: HttpClient {
	fn name(&self) -> &'static str {
		"Liqui"
	}

    fn orderbook(&mut self, product: ccex::CurrencyPair) -> Future<Result<ccex::Orderbook, Error>> {
        unimplemented!()
    }

    fn place_order(&mut self, order: ccex::NewOrder) -> Future<Result<ccex::Order, Error>> {
        unimplemented!()
    }

    fn balances(&mut self) -> Future<Result<Vec<ccex::Balance>, Error>> {
        unimplemented!()
    }

	// fn orderbook_cooldown(&self) -> Duration {
	// 	Duration::from_millis(2100)
	// }

	fn maker_fee(&self) -> d128 {
		// 0.01% / 0.001
		d128::new(1, 3)
	}

	fn taker_fee(&self) -> d128 {
		// 0.025% / 0.0025
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
			ccex::CurrencyPair(ccex::Currency::WAVES, ccex::Currency::USDT) => Some(d128::new(1, 2)),
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
			ccex::CurrencyPair(ccex::Currency::WINGS, ccex::Currency::USDT) => Some(d128::new(1, 8)),
			ccex::CurrencyPair(ccex::Currency::PLU, ccex::Currency::USDT) => Some(d128::new(1, 8)),
			ccex::CurrencyPair(ccex::Currency::INCNT, ccex::Currency::USDT) => Some(d128::new(1, 8)),
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
			ccex::CurrencyPair(ccex::Currency::SNGLS, ccex::Currency::USDT) => Some(d128::new(1, 8)),
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
			ccex::CurrencyPair(ccex::Currency::STORJ, ccex::Currency::USDT) => Some(d128::new(1, 8)),
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

    //
	// fn sync_rest_client(&self) -> Box<SyncExchangeRestClient> {
	// 	Box::new(SyncLiquiRestClient {
	// 		credential: self.credential.clone(),
	// 		host: Url::parse("https://api.liqui.io").unwrap(),
	// 		client: Client::new(),
	// 	})
	// }
    //
	// fn async_rest_client(&self) -> Box<AsyncExchangeRestClient> {
	// 	let sync_client = SyncLiquiRestClient {
	// 		credential: self.credential.clone(),
	// 		host: Url::parse("https://api.liqui.io").unwrap(),
	// 		client: Client::new(),
	// 	};
	// 	let async_client = AsyncLiquiRestClient::from(sync_client);
	// 	Box::new(async_client)
	// }
}

#[derive(Debug, Clone)]
pub struct SyncLiquiRestClient<Client>
where Client: HttpClient {
	pub credential: Credential,
	pub host: Url,
	pub client: Client,
}

impl<Client> SyncExchangeRestClient for SyncLiquiRestClient<Client>
where Client: HttpClient {
    fn balances(&mut self) -> Result<Vec<ccex::Balance>, Error> {
        let request = GetInfo {
            nonce: nonce(),
        };
        let request = request.authenticate(&self.credential);
        let response = self.client.send(&self.host, request)?;

        response.funds.into_iter()
        	// If a currency can't be converted, it means it's been newly
        	// added to Liqui and hasn't been added to the `Currency` enum. In
        	// that case, ignoring it is fine.
        	.filter_map(|(currency, amount)| {
        		match Currency::try_from(currency) {
        			Ok(currency) => Some((currency, amount)),
        			Err(_) => None
        		}
        	})
        	.map(|(currency, amount)| {
        		let amount = d128::from_f64(amount)
        			.ok_or_else(|| format_err!("Couldn't convert {} into a decimal", amount))?;
        		let balance = ccex::Balance::new(currency.into(), amount);
        		Ok(balance)
        	})
        	.collect()
    }

    fn orderbook(&mut self, product: ccex::CurrencyPair) -> Result<ccex::Orderbook, Error> {
    	let product: CurrencyPairString = CurrencyPair::try_from(product)?.into();
	    let request = GetDepth {
	    	product: &product,
	    };
	    let response = self.client.send(&self.host, request)?;

	    let orderbook = response.get(&product)
	    	.ok_or_else(|| format_err!("The request succeeded but an orderbook for {:?} wasn't returned", &product))?;

	    let asks: Result<ccex::Asks, Error> = orderbook.asks.iter()
	    	.map(|&(price, amount)| {
	    		let price = d128::from_f64(price).ok_or_else(|| format_err!("Couldn't convert {} into a decimal", price))?;
	    		let amount = d128::from_f64(amount).ok_or_else(|| format_err!("Couldn't convert {} into a decimal", amount))?;
	    		Ok(ccex::Offer::new(price, amount))
	    	})
	    	.collect();

	    let bids: Result<ccex::Bids, Error> = orderbook.bids.iter()
	    	.map(|&(price, amount)| {
	    		let price = d128::from_f64(price).ok_or_else(|| format_err!("Couldn't convert {} into a decimal", price))?;
	    		let amount = d128::from_f64(amount).ok_or_else(|| format_err!("Couldn't convert {} into a decimal", amount))?;
	    		Ok(ccex::Offer::new(price, amount))
	    	})
	    	.collect();

	    Ok(ccex::Orderbook::new(asks?, bids?))
	}

	// todo: cleanup
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
				original_quantity: d128::from_f64(response.received).unwrap() + d128::from_f64(response.remains).unwrap(),
				remaining_quantity: d128::from_f64(response.remains).unwrap(),
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
    				price: d128::from_f64(order.rate).unwrap(),
    				original_quantity: d128::zero(),
    				remaining_quantity: d128::from_f64(order.amount).unwrap(),
    				time_in_force: ccex::TimeInForce::GoodTillCancelled,
    			}
    		};
    		orders.push(order);
    	}
    	Ok(orders)
    }
}


// pub struct AsyncLiquiRestClient {
// 	pub threads: Vec<JoinHandle<()>>,
// 	pub orderbook_channel:		RefCell<(mpsc::Sender<ccex::CurrencyPair>, 	mpsc::Receiver<Result<ccex::Orderbook, Error>>)>,
// 	pub place_order_channel: 	RefCell<(mpsc::Sender<ccex::NewOrder>, 		mpsc::Receiver<Result<ccex::Order, Error>>)>,
// 	pub balances_channel: 		RefCell<(mpsc::Sender<()>, 					mpsc::Receiver<Result<Vec<ccex::Balance>, Error>>)>,
// }
//
// impl AsyncExchangeRestClient for AsyncLiquiRestClient {
// 	fn balances<'a>(&'a self) -> Future<Result<Vec<ccex::Balance>, Error>> {
// 		let (ref mut sender, _) = *self.balances_channel.borrow_mut();
// 		sender.send(()).unwrap();
//
// 		Future::new(move || {
// 			let (_, ref mut receiver) = *self.balances_channel.borrow_mut();
// 			receiver.recv().unwrap()
// 		})
// 	}
//
// 	fn orderbook<'a>(&'a self, product: ccex::CurrencyPair) -> Future<Result<ccex::Orderbook, Error>> {
// 		let (ref mut sender, _) = *self.orderbook_channel.borrow_mut();
// 		sender.send(product).unwrap();
//
// 		Future::new(move || {
// 			let (_, ref receiver) = *self.orderbook_channel.borrow_mut();
// 			receiver.recv().unwrap()
// 		})
// 	}
//
// 	fn orders<'a>(&'a self, product: ccex::CurrencyPair) -> Future<Result<Vec<ccex::Order>, Error>> {
// 		unimplemented!()
// 	}
//
// 	fn place_order<'a>(&'a self, new_order: ccex::NewOrder) -> Future<Result<ccex::Order, Error>> {
// 		let (ref mut sender, _) = *self.place_order_channel.borrow_mut();
// 		sender.send(new_order).unwrap();
//
// 		Future::new(move || {
// 			let (_, ref mut receiver) = *self.place_order_channel.borrow_mut();
// 			receiver.recv().unwrap()
// 		})
// 	}
// }
//
// impl<Client> From<SyncLiquiRestClient<Client>> for AsyncLiquiRestClient
// where Client: HttpClient {
// 	fn from(client: SyncLiquiRestClient<Client>) -> Self {
// 		let (orderbook_channel, worker_orderbook_channel) = dual_channel();
// 		let orderbook_thread = {
// 			let mut client = client.clone();
// 			let (mut sender, mut receiver) = worker_orderbook_channel;
// 			thread::spawn(move || {
// 				for product in receiver.iter() {
// 					sender.send(client.orderbook(product)).unwrap();
// 				}
// 			})
// 		};
//
// 		let (place_order_channel, worker_place_order_channel) = dual_channel();
// 		let place_order_thread = {
// 			let mut client = client.clone();
// 			let (mut sender, mut receiver) = worker_place_order_channel;
// 			thread::spawn(move || {
// 				for new_order in receiver.iter() {
// 					sender.send(client.place_order(new_order)).unwrap();
// 				}
// 			})
// 		};
//
// 		let (balances_channel, worker_balances_channel) = dual_channel();
// 		let balances_thread = {
// 			let mut client = client.clone();
// 			let (mut sender, mut receiver) = worker_balances_channel;
// 			thread::spawn(move || {
// 				for _ in receiver.iter() {
// 					sender.send(client.balances()).unwrap();
// 				}
// 			})
// 		};
//
// 		AsyncLiquiRestClient {
// 			orderbook_channel: RefCell::new(orderbook_channel),
// 			place_order_channel: RefCell::new(place_order_channel),
// 			balances_channel: RefCell::new(balances_channel),
// 			threads: vec![
// 				orderbook_thread,
// 				place_order_thread,
// 				balances_thread,
// 			],
// 		}
// 	}
// }
//
