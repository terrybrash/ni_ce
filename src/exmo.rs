use std::fmt::{self, Display, Formatter};
use api::{HttpResponse, NeedsAuthentication, PrivateRequest, RestResource, Headers, Query, Method};
use decimal::{d128};
use serde_json;
use serde::de::{DeserializeOwned};
use failure::{Error, ResultExt};

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct Credential {
	key: String,
	signature: String,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
struct PrivateResponse<T> {
	success: i32,
	#[serde(rename="return")]
	ok: Option<T>,
	error: Option<String>,
}

impl<T> PrivateResponse<T> {
	pub fn is_ok(&self) -> bool {
		self.success == 1
	}

	pub fn into_result(self) -> Result<T, String> {
		if self.is_ok() {
			Ok(self.ok.unwrap())
		} else {
			Err(self.error.unwrap())
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

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Copy, Deserialize, Serialize)]
pub enum Product {
	eth_btc,
}

impl Display for Product {
	fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
		write!(f, "{:?}", self)
	}
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Copy, Deserialize, Serialize)]
pub struct GetInfo;
impl NeedsAuthentication<Credential> for GetInfo{}
impl RestResource for PrivateRequest<GetInfo, Credential> {
	type Response = Info;

	fn method(&self) -> Method {
		Method::Post
	}

	fn path(&self) -> String {
		"/getInfo".into()
	}

	fn query(&self) -> Query {
		unimplemented!()
	}

	fn headers(&self) -> Result<Headers, Error> {
		unimplemented!()
	}

	fn deserialize(&self, response: &HttpResponse) -> Result<Self::Response, Error> {
		deserialize_private_response(response)
	}
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct Info {
	/// Your account balance available for trading. Doesn’t include funds on
	/// your open orders.
	funds: Funds,

	/// The privileges of the current API key. At this time the privilege to
	/// withdraw is not used anywhere.
	rights: Rights,

	/// The number of your open orders.
	open_orders: i64,

	/// Server time (UTC).
	server_time: i64,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct Funds {
	eth: Option<d128>,
	btc: Option<d128>,
	ltc: Option<d128>,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct Rights {
	info: i32,
	trade: i32,
	withdraw: i32,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct PlaceOrder {
	pair: Product,
	#[serde(rename="type")]
	side: Side,
	rate: d128,
	amount: d128,
}

impl NeedsAuthentication<Credential> for PlaceOrder {}
impl RestResource for PrivateRequest<PlaceOrder, Credential> {
	type Response = OrderPlacement;

	fn method(&self) -> Method {
		Method::Post
	}

	fn path(&self) -> String {
		"/trade".into()
	}

	fn headers(&self) -> Result<Headers, Error> {
		unimplemented!()
	}

	fn query(&self) -> Query {
		vec![
			("pair".to_owned(), 	self.request.pair.to_string()),
			("type".to_owned(), 	self.request.side.to_string()),
			("rate".to_owned(), 	self.request.rate.to_string()),
			("amount".to_owned(), 	self.request.amount.to_string()),
		]
	}

	fn deserialize(&self, response: &HttpResponse) -> Result<Self::Response, Error> {
		deserialize_private_response(response)
	}
}

fn deserialize_private_response<T>(response: &HttpResponse) -> Result<T, Error> 
where T: DeserializeOwned {
	let response = response.body_to_string()?;
	let response: PrivateResponse<T> = 
		serde_json::from_str(&response)
		.context(format!("failed to deserialize: {}", response))?;

	response
		.into_result()
		.map_err(|e| format_err!("the server returned \"{}\"", e))
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
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