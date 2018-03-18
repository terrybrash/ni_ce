use crate as ccex;
use Exchange;
use api::{Header, Headers, HttpClient, HttpRequest, HttpResponse, Method, Payload, Query};
use chrono::Utc;
use failure::{err_msg, Error, ResultExt};
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
use url::Url;
use std::cell::RefCell;

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
            None => return Err(err_msg("body is empty")),
        };

        let is_success = response
            .as_object()
            .and_then(|obj| obj.get("success"))
            .and_then(|is_success| is_success.as_u64())
            .map_or(true, |is_success| is_success == 1);

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
            None => return Err(err_msg("the body is empty")),
        };

        let response: PrivateResponse<T> = serde_json::from_str(response)
            .with_context(|_| format!("failed to deserialize: \"{}\"", response))?;

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
        (now.timestamp() as u32 - 1_521_186_749u32) * 1000 + now.timestamp_subsec_millis()
    }

    fn get_info(&self) -> Result<Info, Error> {
        // Liqui encodes its body data as an http query.
        let body = {
            let mut query = Query::with_capacity(2);
            query.append_param("method", "getInfo");
            query.append_param("nonce", Self::nonce().to_string());
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
        // Note: Liqui only supports limit orders
        let (price, quantity) = match order.instruction {
            ccex::NewOrderInstruction::Limit {
                price, quantity, ..
            } => (price, quantity),
        };
        let product: CurrencyPair = order.product.try_into()?;
        let side: Side = order.side.into();
        let body = {
            let mut query = Query::with_capacity(6);
            query.append_param("nonce", Self::nonce().to_string());
            query.append_param("method", "trade");
            query.append_param("pair", product.to_string());
            query.append_param("type", side.to_string());
            query.append_param("rate", price.to_string());
            query.append_param("amount", quantity.to_string());
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
                price,
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
        use Currency::*;
        use CurrencyPair;
        match product {
            CurrencyPair(ROUND, USDT) => Some(d128::one()),

            CurrencyPair(VSL, USDT) => Some(d128::new(1, 1)),

            CurrencyPair(ICN, USDT) | CurrencyPair(GNT, USDT) | CurrencyPair(WAVES, USDT) => {
                Some(d128::new(1, 2))
            }

            CurrencyPair(STEEM, BTC)
            | CurrencyPair(SBD, BTC)
            | CurrencyPair(GOLOS, BTC)
            | CurrencyPair(GBG, BTC)
            | CurrencyPair(LTC, USDT) => Some(d128::new(1, 3)),

            CurrencyPair(DASH, USDT) | CurrencyPair(INCNT, BTC) | CurrencyPair(ETH, USDT) => {
                Some(d128::new(1, 4))
            }

            CurrencyPair(BTC, USDT) => Some(d128::new(1, 5)),

            CurrencyPair(REP, ETH)
            | CurrencyPair(EDG, ETH)
            | CurrencyPair(REP, USDT)
            | CurrencyPair(EDG, USDT) => Some(d128::new(1, 7)),

            CurrencyPair(ADX, BTC)
            | CurrencyPair(ADX, ETH)
            | CurrencyPair(ADX, USDT)
            | CurrencyPair(AE, BTC)
            | CurrencyPair(AE, ETH)
            | CurrencyPair(AE, USDT)
            | CurrencyPair(AION, BTC)
            | CurrencyPair(AION, ETH)
            | CurrencyPair(AION, USDT)
            | CurrencyPair(ANS, BTC)
            | CurrencyPair(ANT, BTC)
            | CurrencyPair(ANT, ETH)
            | CurrencyPair(ANT, USDT)
            | CurrencyPair(AST, BTC)
            | CurrencyPair(AST, ETH)
            | CurrencyPair(AST, USDT)
            | CurrencyPair(BAT, BTC)
            | CurrencyPair(BAT, ETH)
            | CurrencyPair(BAT, USDT)
            | CurrencyPair(BCAP, BTC)
            | CurrencyPair(BCAP, ETH)
            | CurrencyPair(BCAP, USDT)
            | CurrencyPair(BCH, BTC)
            | CurrencyPair(BCH, ETH)
            | CurrencyPair(BCH, USDT)
            | CurrencyPair(BMC, BTC)
            | CurrencyPair(BMC, ETH)
            | CurrencyPair(BMC, USDT)
            | CurrencyPair(BNT, BTC)
            | CurrencyPair(BNT, ETH)
            | CurrencyPair(BNT, USDT)
            | CurrencyPair(CFI, BTC)
            | CurrencyPair(CFI, ETH)
            | CurrencyPair(CFI, USDT)
            | CurrencyPair(CVC, BTC)
            | CurrencyPair(CVC, ETH)
            | CurrencyPair(CVC, USDT)
            | CurrencyPair(DASH, BTC)
            | CurrencyPair(DASH, ETH)
            | CurrencyPair(DCT, BTC)
            | CurrencyPair(DGD, BTC)
            | CurrencyPair(DGD, ETH)
            | CurrencyPair(DGD, USDT)
            | CurrencyPair(DNT, BTC)
            | CurrencyPair(DNT, ETH)
            | CurrencyPair(DNT, USDT)
            | CurrencyPair(EDG, BTC)
            | CurrencyPair(ENG, BTC)
            | CurrencyPair(ENG, ETH)
            | CurrencyPair(ENG, USDT)
            | CurrencyPair(EOS, BTC)
            | CurrencyPair(EOS, ETH)
            | CurrencyPair(EOS, USDT)
            | CurrencyPair(ETH, BTC)
            | CurrencyPair(GNO, BTC)
            | CurrencyPair(GNO, ETH)
            | CurrencyPair(GNO, USDT)
            | CurrencyPair(GNT, BTC)
            | CurrencyPair(GNT, ETH)
            | CurrencyPair(GUP, BTC)
            | CurrencyPair(GUP, ETH)
            | CurrencyPair(GUP, USDT)
            | CurrencyPair(HMQ, BTC)
            | CurrencyPair(HMQ, ETH)
            | CurrencyPair(HMQ, USDT)
            | CurrencyPair(ICN, BTC)
            | CurrencyPair(ICN, ETH)
            | CurrencyPair(INCNT, ETH)
            | CurrencyPair(INCNT, USDT)
            | CurrencyPair(IND, BTC)
            | CurrencyPair(IND, ETH)
            | CurrencyPair(IND, USDT)
            | CurrencyPair(INS, BTC)
            | CurrencyPair(INS, ETH)
            | CurrencyPair(INS, USDT)
            | CurrencyPair(KNC, BTC)
            | CurrencyPair(KNC, ETH)
            | CurrencyPair(KNC, USDT)
            | CurrencyPair(LTC, BTC)
            | CurrencyPair(LTC, ETH)
            | CurrencyPair(LUN, BTC)
            | CurrencyPair(LUN, ETH)
            | CurrencyPair(LUN, USDT)
            | CurrencyPair(MANA, BTC)
            | CurrencyPair(MANA, ETH)
            | CurrencyPair(MANA, USDT)
            | CurrencyPair(MCO, BTC)
            | CurrencyPair(MCO, ETH)
            | CurrencyPair(MCO, USDT)
            | CurrencyPair(MGO, BTC)
            | CurrencyPair(MGO, ETH)
            | CurrencyPair(MGO, USDT)
            | CurrencyPair(MLN, BTC)
            | CurrencyPair(MLN, ETH)
            | CurrencyPair(MLN, USDT)
            | CurrencyPair(MYST, BTC)
            | CurrencyPair(MYST, ETH)
            | CurrencyPair(MYST, USDT)
            | CurrencyPair(NET, BTC)
            | CurrencyPair(NET, ETH)
            | CurrencyPair(NET, USDT)
            | CurrencyPair(NEU, BTC)
            | CurrencyPair(NEU, ETH)
            | CurrencyPair(NEU, USDT)
            | CurrencyPair(OAX, BTC)
            | CurrencyPair(OAX, ETH)
            | CurrencyPair(OAX, USDT)
            | CurrencyPair(OMG, BTC)
            | CurrencyPair(OMG, ETH)
            | CurrencyPair(OMG, USDT)
            | CurrencyPair(PAY, BTC)
            | CurrencyPair(PAY, ETH)
            | CurrencyPair(PAY, USDT)
            | CurrencyPair(PLU, BTC)
            | CurrencyPair(PLU, ETH)
            | CurrencyPair(PLU, USDT)
            | CurrencyPair(PRO, BTC)
            | CurrencyPair(PRO, ETH)
            | CurrencyPair(PRO, USDT)
            | CurrencyPair(PTOY, BTC)
            | CurrencyPair(PTOY, ETH)
            | CurrencyPair(PTOY, USDT)
            | CurrencyPair(QRL, BTC)
            | CurrencyPair(QRL, ETH)
            | CurrencyPair(QRL, USDT)
            | CurrencyPair(QTUM, BTC)
            | CurrencyPair(QTUM, ETH)
            | CurrencyPair(QTUM, USDT)
            | CurrencyPair(REP, BTC)
            | CurrencyPair(REQ, BTC)
            | CurrencyPair(REQ, ETH)
            | CurrencyPair(REQ, USDT)
            | CurrencyPair(RLC, BTC)
            | CurrencyPair(RLC, ETH)
            | CurrencyPair(RLC, USDT)
            | CurrencyPair(ROUND, BTC)
            | CurrencyPair(ROUND, ETH)
            | CurrencyPair(SALT, BTC)
            | CurrencyPair(SALT, ETH)
            | CurrencyPair(SALT, USDT)
            | CurrencyPair(SAN, BTC)
            | CurrencyPair(SAN, ETH)
            | CurrencyPair(SAN, USDT)
            | CurrencyPair(SNGLS, BTC)
            | CurrencyPair(SNGLS, ETH)
            | CurrencyPair(SNGLS, USDT)
            | CurrencyPair(SNM, BTC)
            | CurrencyPair(SNM, ETH)
            | CurrencyPair(SNM, USDT)
            | CurrencyPair(SNT, BTC)
            | CurrencyPair(SNT, ETH)
            | CurrencyPair(SNT, USDT)
            | CurrencyPair(SRN, BTC)
            | CurrencyPair(SRN, ETH)
            | CurrencyPair(SRN, USDT)
            | CurrencyPair(STORJ, BTC)
            | CurrencyPair(STORJ, ETH)
            | CurrencyPair(STORJ, USDT)
            | CurrencyPair(STX, BTC)
            | CurrencyPair(STX, ETH)
            | CurrencyPair(STX, USDT)
            | CurrencyPair(TAAS, BTC)
            | CurrencyPair(TAAS, ETH)
            | CurrencyPair(TAAS, USDT)
            | CurrencyPair(TIME, BTC)
            | CurrencyPair(TIME, ETH)
            | CurrencyPair(TIME, USDT)
            | CurrencyPair(TKN, BTC)
            | CurrencyPair(TKN, ETH)
            | CurrencyPair(TKN, USDT)
            | CurrencyPair(TNT, BTC)
            | CurrencyPair(TNT, ETH)
            | CurrencyPair(TNT, USDT)
            | CurrencyPair(TRST, BTC)
            | CurrencyPair(TRST, ETH)
            | CurrencyPair(TRST, USDT)
            | CurrencyPair(TRX, BTC)
            | CurrencyPair(TRX, ETH)
            | CurrencyPair(TRX, USDT)
            | CurrencyPair(VEN, BTC)
            | CurrencyPair(VEN, ETH)
            | CurrencyPair(VEN, USDT)
            | CurrencyPair(VSL, BTC)
            | CurrencyPair(VSL, ETH)
            | CurrencyPair(WAVES, BTC)
            | CurrencyPair(WAVES, ETH)
            | CurrencyPair(WINGS, BTC)
            | CurrencyPair(WINGS, ETH)
            | CurrencyPair(WINGS, USDT)
            | CurrencyPair(XID, BTC)
            | CurrencyPair(XID, ETH)
            | CurrencyPair(XID, USDT)
            | CurrencyPair(XZC, BTC)
            | CurrencyPair(ZRX, BTC)
            | CurrencyPair(ZRX, ETH)
            | CurrencyPair(ZRX, USDT) => Some(d128::new(1, 8)),
            _ => None,
        }
    }
}
