use api::{Header, Headers, HttpClient, HttpRequest, HttpResponse, Method, NeedsAuthentication,
          Payload, PrivateRequest, Query, QueryBuilder, RestResource};
use chrono::Utc;
use crate as ccex;
use failure::{err_msg, Error, ResultExt};
use hex;
use hmac::{Hmac, Mac};
use rust_decimal::Decimal as d128;
use serde::de::DeserializeOwned;
use serde_json;
use sha2::Sha512;
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::fmt::{self, Display, Formatter};
use std::str::FromStr;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{self, Duration};
use url::Url;
use {dual_channel, Exchange, Future, FutureLock};

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

impl From<CurrencyPair> for ccex::CurrencyPair {
    fn from(CurrencyPair(base, quote): CurrencyPair) -> Self {
        ccex::CurrencyPair(base.into(), quote.into())
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

#[derive(Fail, Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub enum ParseCurrencyError {
    /// The currency is either spelled incorrectly, or isn't supported by this
    /// crate; it could be a legitimate currency that needs to be added to the
    /// `Currency` enum.
    #[fail(display = "Invalid or unsupported currency {}", _0)]
    InvalidOrUnsupportedCurrency(String),
}

impl FromStr for Currency {
    type Err = ParseCurrencyError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        const CURRENCIES: [(&'static str, Currency); 18] = [
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

        for &(string, currency) in CURRENCIES.iter() {
            if string.eq_ignore_ascii_case(s) {
                return Ok(currency);
            }
        }
        Err(ParseCurrencyError::InvalidOrUnsupportedCurrency(
            s.to_owned(),
        ))
    }
}

impl Display for Currency {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        write!(f, "{:?}", self)
    }
}

impl From<Currency> for ccex::Currency {
    fn from(currency: Currency) -> Self {
        match currency {
            Currency::BCH => ccex::Currency::BCH,
            Currency::BTC => ccex::Currency::BTC,
            Currency::DASH => ccex::Currency::DASH,
            Currency::DOGE => ccex::Currency::DOGE,
            Currency::ETC => ccex::Currency::ETC,
            Currency::ETH => ccex::Currency::ETH,
            Currency::EUR => ccex::Currency::EUR,
            Currency::KICK => ccex::Currency::KICK,
            Currency::LTC => ccex::Currency::LTC,
            Currency::PLN => ccex::Currency::PLN,
            Currency::RUB => ccex::Currency::RUB,
            Currency::UAH => ccex::Currency::UAHPAY,
            Currency::USD => ccex::Currency::USD,
            Currency::USDT => ccex::Currency::USDT,
            Currency::WAVES => ccex::Currency::WAVES,
            Currency::XMR => ccex::Currency::XMR,
            Currency::XRP => ccex::Currency::XRP,
            Currency::ZEC => ccex::Currency::ZEC,
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
            ccex::Currency::UAHPAY => Ok(Currency::UAH),
            ccex::Currency::USD => Ok(Currency::USD),
            ccex::Currency::USDT => Ok(Currency::USDT),
            ccex::Currency::WAVES => Ok(Currency::WAVES),
            ccex::Currency::XMR => Ok(Currency::XMR),
            ccex::Currency::XRP => Ok(Currency::XRP),
            ccex::Currency::ZEC => Ok(Currency::ZEC),
            currency => Err(CurrencyConversionError::UnsupportedCurrency(
                currency.to_string(),
            )),
        }
    }
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
struct ErrorResponse {
    pub result: bool,
    pub error: String,
}

#[derive(Debug, PartialEq, Eq, Copy, Hash, PartialOrd, Ord, Clone, Deserialize, Serialize)]
pub enum OrderInstruction {
    LimitBuy,
    LimitSell,
    MarketBuy,
    MarketSell,
    MarketBuyTotal,
    MarketSellTotal,
}

impl Display for OrderInstruction {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::Error> {
        match *self {
            OrderInstruction::LimitBuy => f.write_str("buy"),
            OrderInstruction::LimitSell => f.write_str("sell"),
            OrderInstruction::MarketBuy => f.write_str("market_buy"),
            OrderInstruction::MarketSell => f.write_str("market_sell"),
            OrderInstruction::MarketBuyTotal => f.write_str("market_buy_total"),
            OrderInstruction::MarketSellTotal => f.write_str("market_sell_total"),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Orderbook {
    pub ask_quantity: d128,
    pub ask_amount: d128,
    pub ask_top: d128,
    pub bid_quantity: d128,
    pub bid_amount: d128,
    pub bid_top: d128,
    pub ask: Vec<(d128, d128, d128)>,
    pub bid: Vec<(d128, d128, d128)>,
}

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
struct UserInfo {
    pub uid: i64,
    pub server_date: u64,
    pub balances: HashMap<String, d128>,
    pub reserved: HashMap<String, d128>,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
struct NewOrder {
    pub product: CurrencyPair,
    pub quantity: d128,
    pub price: d128,
    pub instruction: OrderInstruction,
    pub nonce: u32,
}

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
struct Order {
    pub order_id: i64,
}

struct Exmo<Client: HttpClient> {
    pub host: Url,
    pub http_client: Client,
}

impl<Client: HttpClient> Exmo<Client> {
    fn nonce() -> u32 {
        // TODO: switch to a cached nonce at some point. Using milliseconds
        // elapsed since epoch has the limitations of 1) only allowing one request
        // per millisecond and 2) expiring after ~50 days
        let now = Utc::now();
        (now.timestamp() as u32 - 1518363415u32) * 1000 + now.timestamp_subsec_millis()
    }

    fn get_user_info(
        &mut self,
        nonce: u32,
        credential: &ccex::Credential,
    ) -> Result<UserInfo, Error> {
        let query = QueryBuilder::with_capacity(2)
            .param("nonce", nonce.to_string())
            .build();
        let body = query.to_string().trim_left_matches("?").to_owned();
        let headers = Self::private_headers(credential, &body)?;
        let http_request = HttpRequest {
            method: Method::Post,
            path: "/v1/user_info",
            host: self.host.as_str(),
            headers: Some(headers),
            body: Some(Payload::Text(body)),
            query: Some(query),
        };
        let http_response = self.http_client.send(&http_request)?;
        Self::deserialize_private_response(&http_response)
    }

    fn private_headers(
        credential: &ccex::Credential,
        request_body: &str,
    ) -> Result<Headers, Error> {
        let mut mac =
            Hmac::<Sha512>::new(credential.secret.as_bytes()).map_err(|e| format_err!("{:?}", e))?;
        mac.input(request_body.as_bytes());
        let signature = hex::encode(mac.result().code().to_vec());
        let headers = vec![
            Header::new("Content-Length", signature.len().to_string()),
            Header::new("Content-Type", "application/x-www-form-urlencoded"),
            Header::new("Key", credential.key.clone()),
            Header::new("Sign", signature),
        ];
        Ok(headers)
    }

    /// Deserialize a response returned from a private HTTP request.
    fn deserialize_private_response<T>(response: &HttpResponse) -> Result<T, Error>
    where
        T: DeserializeOwned,
    {
        let body = match response.body {
            Some(Payload::Text(ref body)) => body,
            Some(Payload::Binary(_)) => Err(format_err!(
                "http response contained binary, expected text."
            ))?,
            None => Err(format_err!("the body is empty"))?,
        };
        let response: serde_json::Value = serde_json::from_str(body)?;

        // If the response is an error, it will be a json object containing a
        // `result` equal to `false`.
        let is_error = response
            .as_object()
            .map(|object| match object.get("result") {
                Some(&serde_json::Value::Bool(result)) => !result,
                _ => false,
            })
            .unwrap_or(false);

        if is_error {
            let error: ErrorResponse = serde_json::from_value(response)
                .with_context(|_| format!("failed to deserialize: \"{}\"", body))?;
            Err(format_err!("Server returned: {}", error.error))
        } else {
            let response = serde_json::from_value(response)
                .context(format!("failed to deserialize: \"{}\"", body))?;
            Ok(response)
        }
    }

    /// Deserialize a response returned from a public HTTP request.
    fn deserialize_public_response<T>(response: &HttpResponse) -> Result<T, Error>
    where
        T: DeserializeOwned,
    {
        match response.body {
            Some(Payload::Text(ref body)) => Ok(serde_json::from_str(body)?),
            Some(Payload::Binary(ref body)) => Ok(serde_json::from_slice(body)?),
            None => panic!(),
        }
    }
}

impl<Client: HttpClient> Exchange for Exmo<Client> {
    fn get_balances(&mut self, credential: &ccex::Credential) -> Result<Vec<ccex::Balance>, Error> {
        let user_info = self.get_user_info(Self::nonce(), credential)?;

        let mut balances = Vec::with_capacity(user_info.balances.len());
        for (currency, balance) in user_info.balances.into_iter() {
            match currency.parse::<Currency>() {
                Ok(currency) => {
                    let currency = ccex::Currency::from(currency);
                    let balance = ccex::Balance::new(currency, balance);
                    balances.push(balance);
                }
                Err(ParseCurrencyError::InvalidOrUnsupportedCurrency(currency)) => {
                    // The currency isn't support. We'll just silently skip it.
                }
            }
        }
        Ok(balances)
    }

    fn get_orderbooks(
        &mut self,
        products: &[ccex::CurrencyPair],
    ) -> Result<HashMap<ccex::CurrencyPair, ccex::Orderbook>, Error> {
        let mut exmo_products = Vec::with_capacity(products.len());
        for product in products {
            let exmo_product = CurrencyPair::try_from(*product)?;
            exmo_products.push(exmo_product.to_string());
        }

        let query = QueryBuilder::with_capacity(2)
            .param("pair", exmo_products.as_slice().join(","))
            .param("limit", "100")
            .build();
        let http_request = HttpRequest {
            method: Method::Get,
            host: self.host.as_str(),
            path: "/v1/order_book",
            query: Some(query),
            body: None,
            headers: None,
        };
        let http_response = self.http_client.send(&http_request)?;
        let orderbooks: HashMap<String, Orderbook> =
            Self::deserialize_public_response(&http_response)?;

        orderbooks
            .into_iter()
            .map(|(product, orderbook)| {
                let product: CurrencyPair = product.parse()?;
                let product: ccex::CurrencyPair = product.into();

                let asks = orderbook
                    .ask
                    .into_iter()
                    .map(|(price, amount, _)| ccex::Offer::new(price, amount))
                    .collect();
                let bids = orderbook
                    .bid
                    .into_iter()
                    .map(|(price, amount, _)| ccex::Offer::new(price, amount))
                    .collect();

                Ok((product, ccex::Orderbook::new(asks, bids)))
            })
            .collect()
    }

    fn place_order(
        &mut self,
        credential: &ccex::Credential,
        order: ccex::NewOrder,
    ) -> Result<ccex::Order, Error> {
        let exmo_product: CurrencyPair = order.product.try_into()?;
        let exmo_instruction = match order.side {
            ccex::Side::Ask => OrderInstruction::LimitSell,
            ccex::Side::Bid => OrderInstruction::LimitBuy,
        };
        let (price, quantity) = match order.instruction {
            ccex::NewOrderInstruction::Limit {
                price, quantity, ..
            } => (price, quantity),
            _ => return Err(err_msg("only limit orders are supported on exmo")),
        };

        let query = QueryBuilder::with_capacity(5)
            .param("nonce", Self::nonce().to_string())
            .param("pair", exmo_product.to_string())
            .param("quantity", quantity.to_string())
            .param("price", price.to_string())
            .param("type", exmo_instruction.to_string())
            .build();
        let body = query.to_string().trim_left_matches("?").to_owned();
        let headers = Self::private_headers(credential, &body)?;
        let http_request = HttpRequest {
            method: Method::Post,
            path: "/v1/order_create",
            host: self.host.as_str(),
            headers: Some(headers),
            body: Some(Payload::Text(body)),
            query: Some(query),
        };

        let http_response = self.http_client.send(&http_request)?;
        let response: Order = Self::deserialize_private_response(&http_response)?;

        Ok(order.into())
    }

    fn name(&self) -> &'static str {
        "Exmo"
    }

    fn maker_fee(&self) -> d128 {
        // 0.02% / 0.002
        d128::new(2, 3)
    }

    fn taker_fee(&self) -> d128 {
        // 0.02% / 0.002
        d128::new(2, 3)
    }

    fn precision(&self) -> u32 {
        8
    }

    fn min_quantity(&self, product: ccex::CurrencyPair) -> Option<d128> {
        match product {
            ccex::CurrencyPair(ccex::Currency::BTC, ccex::Currency::USD) => Some(d128::new(1, 3)),
            ccex::CurrencyPair(ccex::Currency::BTC, ccex::Currency::EUR) => Some(d128::new(1, 3)),
            ccex::CurrencyPair(ccex::Currency::BTC, ccex::Currency::RUB) => Some(d128::new(1, 3)),
            ccex::CurrencyPair(ccex::Currency::BTC, ccex::Currency::UAHPAY) => {
                Some(d128::new(1, 3))
            }
            ccex::CurrencyPair(ccex::Currency::BTC, ccex::Currency::PLN) => Some(d128::new(1, 3)),
            ccex::CurrencyPair(ccex::Currency::BCH, ccex::Currency::BTC) => Some(d128::new(3, 3)),
            ccex::CurrencyPair(ccex::Currency::BCH, ccex::Currency::USD) => Some(d128::new(3, 3)),
            ccex::CurrencyPair(ccex::Currency::BCH, ccex::Currency::RUB) => Some(d128::new(3, 3)),
            ccex::CurrencyPair(ccex::Currency::BCH, ccex::Currency::ETH) => Some(d128::new(3, 3)),
            ccex::CurrencyPair(ccex::Currency::DASH, ccex::Currency::BTC) => Some(d128::new(1, 2)),
            ccex::CurrencyPair(ccex::Currency::DASH, ccex::Currency::USD) => Some(d128::new(1, 2)),
            ccex::CurrencyPair(ccex::Currency::DASH, ccex::Currency::RUB) => Some(d128::new(1, 2)),
            ccex::CurrencyPair(ccex::Currency::ETH, ccex::Currency::BTC) => Some(d128::new(1, 2)),
            ccex::CurrencyPair(ccex::Currency::ETH, ccex::Currency::LTC) => Some(d128::new(1, 2)),
            ccex::CurrencyPair(ccex::Currency::ETH, ccex::Currency::USD) => Some(d128::new(1, 2)),
            ccex::CurrencyPair(ccex::Currency::ETH, ccex::Currency::EUR) => Some(d128::new(1, 2)),
            ccex::CurrencyPair(ccex::Currency::ETH, ccex::Currency::RUB) => Some(d128::new(1, 2)),
            ccex::CurrencyPair(ccex::Currency::ETH, ccex::Currency::UAHPAY) => {
                Some(d128::new(1, 2))
            }
            ccex::CurrencyPair(ccex::Currency::ETH, ccex::Currency::PLN) => Some(d128::new(1, 3)),
            ccex::CurrencyPair(ccex::Currency::ETC, ccex::Currency::BTC) => Some(d128::new(2, 1)),
            ccex::CurrencyPair(ccex::Currency::ETC, ccex::Currency::USD) => Some(d128::new(2, 1)),
            ccex::CurrencyPair(ccex::Currency::ETC, ccex::Currency::RUB) => Some(d128::new(2, 1)),
            ccex::CurrencyPair(ccex::Currency::LTC, ccex::Currency::BTC) => Some(d128::new(5, 2)),
            ccex::CurrencyPair(ccex::Currency::LTC, ccex::Currency::USD) => Some(d128::new(5, 2)),
            ccex::CurrencyPair(ccex::Currency::LTC, ccex::Currency::EUR) => Some(d128::new(5, 2)),
            ccex::CurrencyPair(ccex::Currency::LTC, ccex::Currency::RUB) => Some(d128::new(5, 2)),
            ccex::CurrencyPair(ccex::Currency::ZEC, ccex::Currency::BTC) => Some(d128::new(1, 2)),
            ccex::CurrencyPair(ccex::Currency::ZEC, ccex::Currency::USD) => Some(d128::new(1, 2)),
            ccex::CurrencyPair(ccex::Currency::ZEC, ccex::Currency::EUR) => Some(d128::new(1, 2)),
            ccex::CurrencyPair(ccex::Currency::ZEC, ccex::Currency::RUB) => Some(d128::new(1, 2)),
            ccex::CurrencyPair(ccex::Currency::XRP, ccex::Currency::BTC) => Some(d128::new(1, 1)),
            ccex::CurrencyPair(ccex::Currency::XRP, ccex::Currency::USD) => Some(d128::new(15, 0)),
            ccex::CurrencyPair(ccex::Currency::XRP, ccex::Currency::RUB) => Some(d128::new(15, 0)),
            ccex::CurrencyPair(ccex::Currency::XMR, ccex::Currency::BTC) => Some(d128::new(3, 2)),
            ccex::CurrencyPair(ccex::Currency::XMR, ccex::Currency::USD) => Some(d128::new(3, 2)),
            ccex::CurrencyPair(ccex::Currency::XMR, ccex::Currency::EUR) => Some(d128::new(3, 2)),
            ccex::CurrencyPair(ccex::Currency::BTC, ccex::Currency::USDT) => Some(d128::new(1, 3)),
            ccex::CurrencyPair(ccex::Currency::ETH, ccex::Currency::USDT) => Some(d128::new(1, 2)),
            ccex::CurrencyPair(ccex::Currency::USDT, ccex::Currency::USD) => Some(d128::new(3, 0)),
            ccex::CurrencyPair(ccex::Currency::USDT, ccex::Currency::RUB) => Some(d128::new(3, 0)),
            ccex::CurrencyPair(ccex::Currency::USD, ccex::Currency::RUB) => Some(d128::new(3, 0)),
            ccex::CurrencyPair(ccex::Currency::DOGE, ccex::Currency::BTC) => {
                Some(d128::new(100, 0))
            }
            ccex::CurrencyPair(ccex::Currency::WAVES, ccex::Currency::BTC) => Some(d128::new(5, 1)),
            ccex::CurrencyPair(ccex::Currency::WAVES, ccex::Currency::RUB) => Some(d128::new(5, 1)),
            ccex::CurrencyPair(ccex::Currency::KICK, ccex::Currency::BTC) => {
                Some(d128::new(100, 0))
            }
            ccex::CurrencyPair(ccex::Currency::KICK, ccex::Currency::ETH) => {
                Some(d128::new(100, 0))
            }
            _ => None,
        }
    }
}

// pub struct Exmo {
//     credential: Credential,
//     orderbook_channel: (mpsc::Sender<(ccex::CurrencyPair, FutureLock<Result<ccex::Orderbook, Error>>)>, ()),
//     place_order_channel: (mpsc::Sender<(ccex::NewOrder, FutureLock<Result<ccex::Order, Error>>)>, ()),
//     balances_channel: (mpsc::Sender<((), FutureLock<Result<Vec<ccex::Balance>, Error>>)>, ()),
// }
//
// impl Exmo {
//     /// Maximum REST requests per minute.
//     const MAX_REQUESTS_PER_MIN: u32 = 180;
//
//     /// The average amount of requests allowed every second. This can probably
//     /// be exceeded in bursts as long as `MAX_REQUESTS_PER_MIN` isn't
//     /// exceeded. I don't know.
//     const AVERAGE_REQUESTS_PER_SEC: u32 = Self::MAX_REQUESTS_PER_MIN / 60;
//
//     /// The average amount of seconds allowed between requests.
//     const AVERAGE_SECS_PER_REQUEST: f64 = 1000.0 / Self::AVERAGE_REQUESTS_PER_SEC as f64;
//
//     /// Exmo's REST domain.
//     const REST_DOMAIN: &'static str = "https://api.exmo.com";
//
//     pub fn new<Client>(credential: Credential) -> Self
//     where Client: HttpClient {
//         let new_sync_client = || {
//             SyncExmoRestClient {
//                 credential: credential.clone(),
//                 host: Url::parse(Self::REST_DOMAIN).unwrap(),
//                 client: Client::new(),
//             }
//         };
//
//         Exmo {
//             credential: credential.clone(),
//             orderbook_channel: Exmo::spawn_orderbook_thread(new_sync_client()),
//             place_order_channel: Exmo::spawn_order_thread(new_sync_client()),
//             balances_channel: Exmo::spawn_balances_thread(new_sync_client()),
//         }
//     }
//
//     fn spawn_orderbook_thread<Client: HttpClient>(mut client: SyncExmoRestClient<Client>) -> (mpsc::Sender<(ccex::CurrencyPair, FutureLock<Result<ccex::Orderbook, Error>>)>, ()) {
//         let (send, recv) = mpsc::channel::<(ccex::CurrencyPair, FutureLock<Result<ccex::Orderbook, Error>>)>();
//         thread::spawn(move || {
//             for (product, lock) in recv.iter() {
//                 let orderbook = client.orderbook(product);
//                 lock.send(orderbook);
//             }
//         });
//         (send, ())
//     }
//
//     fn spawn_order_thread<Client: HttpClient>(mut client: SyncExmoRestClient<Client>) -> (mpsc::Sender<(ccex::NewOrder, FutureLock<Result<ccex::Order, Error>>)>, ()) {
//         let (send, recv) = mpsc::channel::<(ccex::NewOrder, FutureLock<Result<ccex::Order, Error>>)>();
//         thread::spawn(move || {
//             for (new_order, lock) in recv.iter() {
//                 let order_result = client.place_order(new_order);
//                 lock.send(order_result);
//             }
//         });
//         (send, ())
//     }
//
//     fn spawn_balances_thread<Client: HttpClient>(mut client: SyncExmoRestClient<Client>) -> (mpsc::Sender<((), FutureLock<Result<Vec<ccex::Balance>, Error>>)>, ()) {
//         let (send, recv) = mpsc::channel::<((), FutureLock<Result<Vec<ccex::Balance>, Error>>)>();
//         thread::spawn(move || {
//             for ((), lock) in recv.iter() {
//                 let balances_result = client.balances();
//                 lock.send(balances_result);
//             }
//         });
//         (send, ())
//     }
// }
//
// impl<Client> Exchange<Client> for Exmo
// where Client: HttpClient {
//     fn orderbook(&mut self, product: ccex::CurrencyPair) -> Future<Result<ccex::Orderbook, Error>> {
//         let (future, lock) = Future::await();
//         let (ref mut sender, _) = self.orderbook_channel;
//         sender.send((product, lock));
//         future
//     }
//
//     fn place_order(&mut self, order: ccex::NewOrder) -> Future<Result<ccex::Order, Error>> {
//         let (future, lock) = Future::await();
//         let (ref mut sender, _) = self.place_order_channel;
//         sender.send((order, lock));
//         future
//     }
//
//     fn balances(&mut self) -> Future<Result<Vec<ccex::Balance>, Error>> {
//         let (future, lock) = Future::await();
//         let (ref mut sender, _) = self.balances_channel;
//         sender.send(((), lock));
//         future
//     }
// }
