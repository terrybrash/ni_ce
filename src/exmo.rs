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
use chrono::{Utc};
use crate as ccex;
use failure::{err_msg, Error, ResultExt};
use hex;
use hmac::{Hmac, Mac};
use rust_decimal::Decimal as d128;
use serde::de::{DeserializeOwned};
use serde_json;
use sha2::{Sha512};
use std::cell::RefCell;
use std::collections::HashMap;
use std::convert::{TryFrom, TryInto};
use std::fmt::{self, Display, Formatter};
use std::str::{FromStr};
use std::sync::mpsc;
use std::thread::{self, JoinHandle};
use std::time::Duration;
use url::Url;
use {AsyncExchangeRestClient, SyncExchangeRestClient, Exchange, Future, dual_channel};

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
        Err(ParseCurrencyError::InvalidOrUnsupportedCurrency(s.to_owned()))
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
            currency => Err(CurrencyConversionError::UnsupportedCurrency(currency.to_string())),
        }
    }
}

fn private_headers<R>(request: &R, credential: &Credential) -> Result<Headers, Error> 
where R: RestResource {
    let mut mac = Hmac::<Sha512>::new(credential.secret.as_bytes()).map_err(|e| format_err!("{:?}", e))?;
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

/// Deserialize a response returned from a private HTTP request.
fn deserialize_private_response<T>(response: &HttpResponse) -> Result<T, Error> 
where T: DeserializeOwned {
    let body = match response.body {
        Some(Payload::Text(ref body)) => body,
        Some(Payload::Binary(_)) => Err(format_err!("http response contained binary, expected text."))?,
        None => Err(format_err!("the body is empty"))?,
    };
    let response: serde_json::Value = serde_json::from_str(body)?;

    // If the response is an error, it will be a json object containing a
    // `result` equal to `false`.
    let is_error = response.as_object().map(|object| {
        match object.get("result") {
            Some(&serde_json::Value::Bool(result)) => !result,
            _ => false,
    }}).unwrap_or(false);

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

/// Deserialize a response returned from a public HTTP request.
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
    pub limit: u64,
}

#[derive(Debug, Deserialize)]
pub struct Orderbook {
    // The fields commented out aren't being used so there's no point in doing
    // the work to deserialize them.

    // pub ask_quantity: d128,
    // pub ask_amount: d128,
    // pub ask_top: d128,
    // pub bid_quantity: d128,
    // pub bid_amount: d128,
    // pub bid_top: d128,
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

#[derive(Debug, Hash, PartialEq, PartialOrd, Eq, Ord, Clone, Deserialize, Serialize)]
pub struct PlaceOrder {
    pub pair: CurrencyPair,
    pub quantity: d128,
    pub price: d128,
    pub instruction: OrderInstruction,
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
    // TODO: switch to a cached nonce at some point. Using milliseconds
    // elapsed since epoch has the limitations of 1) only allowing one request
    // per millisecond and 2) expiring after ~50 days
    let now = Utc::now();
    (now.timestamp() as u32 - 1518363415u32) * 1000 + now.timestamp_subsec_millis()
}

pub struct Exmo {
    credential: Credential,
    orderbook: (Instant, Orderbook),
    shared_orderbook: Arc<Mutex<(Instant, Orderbook)>>,
    place_order_channel: (mpsc::Sender<ccex::NewOrder>, mpsc::Receiver<Result<ccex::Order, Error>>),
    // balances: Option<Balance>,
    // shared_balances: Arc<Mutex<Vec<Option<Balance>>>>, // invalidate when a trade is made
}

impl Exchange {
    /// Maximum REST requests per minute.
    const MAX_REQUESTS_PER_MIN: u32 = 180;

    /// The average amount of requests allowed every second. This can probably
    /// be exceeded in bursts as long as `MAX_REQUESTS_PER_MIN` isn't
    /// exceeded. I don't know.
    const AVERAGE_REQUESTS_PER_SEC: u32 = MAX_REQUESTS_PER_MIN / 60;

    /// The average amount of seconds allowed between requests.
    const AVERAGE_SECS_PER_REQUEST: f64 = 1000.0 / AVERAGE_REQUETS_PER_SEC as f64;

    const REST_DOMAIN: &'static str = "https://api.exmo.com";
    const WEBSOCKET_DOMAIN: &'static str = "https//websocket.exmo.com";

    fn new<HttpClient>(credential: Credential) -> Self 
        where HttpClient: HttpClient {
            let mut exmo = Exmo {
                credential: Credential,
                orderbook: (Instant::now(), Orderbook::default()),
                shared_orderbook: (Instant::now(), Orderbook::default()),
            };
            exmo.spawn_orderbook_thread::<HttpClient>();
            exmo
        }

    fn spawn_orderbook_thread<HttpClient>(&self) 
        where Client: HttpClient {
            let mut client = SyncExmoRestClient {
                credential: self.credential.clone(),
                host: REST_DOMAIN.to_string(),
                client: Client::new();
            };

            let orderbook = self.shared_orderbook.clone();

            // Orderbook requests can have a pretty high budget because it's
            // important we have orderbook updates as frequently as possible.
            const ORDERBOOK_REQUEST_BUDGET: f64 = 0.85;
            const COOLDOWN_SECS: f64 = Self::AVERAGE_SECS_PER_REQUEST / ORDERBOOK_REQUEST_BUDGET;
            const COOLDOWN_MILLIS: u32 = (COOLDOWN_SECS * 1000.0) as u32;
            let cooldown = Duration::from_millis(COOLDOWN_MILLIS);

            thread::spawn(move || {
                loop {
                    let request_instant = time::Instant::now();
                    match client.orderbook(product) {
                        Ok(new_orderbook) => {
                            let time = time::Instant::now();
                            let mut orderbook = orderbook.lock().unwrap();
                            *orderbook = (time, new_orderbook);
                        }
                        Err(e) => {
                            println!("[{}] Orderbook error: {}", "Exmo", e);
                        }
                    }

                    let request_elapsed = request_instant.elapsed();
                    if request_elapsed < cooldown {
                        thread::sleep(cooldown - request_elapsed);
                    } else {
                        // Don't sleep. It's already been longer than the cooldown
                        // which means we're lagging behind!
                        //
                        // This isn't really that bad, it just means there
                        // could've been a good order to fill that we missed out
                        // on while waiting for a slow orderbook response.
                    }
                }
            });
        }

    fn orderbook(&mut self) -> Orderbook {
        self.orderbook.lock().unwrap()
    }

    fn place_order<'a>(&'a mut self, new_order: ccex::NewOrder) -> impl FnOnce() -> Result<ccex::Order, Error> + 'a {
        let (ref mut sender, ref receiver) = self.place_order_channel;
        sender.send(new_order).unwrap();
        move || {
            receiver.recv().unwrap()
        }
    }

    fn balances(&mut self) -> Result<Vec<Balance>, Error> {
        let request = GetUserInfo {
            nonce: nonce(),
        };
        let request = request.authenticate(&self.credential);
        let response = self.client.send(&self.host, request)?;

        response.balances.into_iter()
            .filter_map(|(currency, balance)| {
                match currency.parse::<Currency>() {
                    Ok(currency) => Some((currency, balance)),
                    Err(ParseCurrencyError::InvalidOrUnsupportedCurrency(currency)) => None,
                }
            })
        .map(|(currency, balance)| {
            let currency = ccex::Currency::from(currency);
            ccex::Balance::new(currency, balance)
        })
        .map(Ok)
            .collect()
    }

    fn balances(&mut self) -> Vec<Balance>;
}

pub struct Exmo {
    pub credential: Credential,
}

impl<Client> Exchange<Client> for Exmo 
where Client: HttpClient {
    fn name(&self) -> &'static str {
        "Exmo"
    }

    fn orderbook_cooldown(&self) -> Duration {
        Duration::from_millis(500)
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
            ccex::CurrencyPair(ccex::Currency::BTC, ccex::Currency::UAHPAY) => Some(d128::new(1, 3)),
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
            ccex::CurrencyPair(ccex::Currency::ETH, ccex::Currency::UAHPAY) => Some(d128::new(1, 2)),
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
            ccex::CurrencyPair(ccex::Currency::DOGE, ccex::Currency::BTC) => Some(d128::new(100, 0)),
            ccex::CurrencyPair(ccex::Currency::WAVES, ccex::Currency::BTC) => Some(d128::new(5, 1)),
            ccex::CurrencyPair(ccex::Currency::WAVES, ccex::Currency::RUB) => Some(d128::new(5, 1)),
            ccex::CurrencyPair(ccex::Currency::KICK, ccex::Currency::BTC) => Some(d128::new(100, 0)),
            ccex::CurrencyPair(ccex::Currency::KICK, ccex::Currency::ETH) => Some(d128::new(100, 0)),
            _ => None,
        }
    }

    fn sync_rest_client(&self) -> Box<ccex::SyncExchangeRestClient> {
        Box::new(SyncExmoRestClient {
            credential: self.credential.clone(),
            host: Url::parse("https://api.exmo.com").unwrap(),
            client: Client::new(),
        })
    }

    fn async_rest_client(&self) -> Box<ccex::AsyncExchangeRestClient> {
        let sync_client = SyncExmoRestClient {
            credential: self.credential.clone(),
            host: Url::parse("https://api.exmo.com").unwrap(),
            client: Client::new(),
        };
        let async_client = AsyncExmoRestClient::from(sync_client);
        Box::new(async_client)
    }
}

#[derive(Debug, Clone)]
pub struct SyncExmoRestClient<Client>
where Client: HttpClient {
    pub credential: Credential,
    pub host: Url,
    pub client: Client,
}

impl<Client> SyncExmoRestClient<Client> 
where Client: HttpClient {
    fn orderbooks(&mut self, products: &[ccex::CurrencyPair], max_orders: u64) -> Result<Vec<(ccex::CurrencyPair, ccex::Orderbook)>, Error> {
        let products: Result<Vec<CurrencyPair>, Error> = products.iter()
            .map(|&product| CurrencyPair::try_from(product).map_err(Into::into))
            .collect();

        let request = GetOrderbook {
            products: products?,
            limit: max_orders,
        };
        let response = self.client.send(&self.host, request)?;

        response.into_iter()
            .map(|(product, orderbook)| {
                let product: ccex::CurrencyPair = product
                    .parse::<CurrencyPair>()?
                    .try_into()?;

                let asks = orderbook.ask.into_iter()
                    .map(|(price, amount, _)| ccex::Offer::new(price, amount))
                    .collect();
                let bids = orderbook.bid.into_iter()
                    .map(|(price, amount, _)| ccex::Offer::new(price, amount))
                    .collect();
                Ok((product, ccex::Orderbook::new(asks, bids)))
            })
        .collect()
    }
}

impl<Client> SyncExchangeRestClient for SyncExmoRestClient<Client>
where Client: HttpClient {
    fn balances(&mut self) -> Result<Vec<ccex::Balance>, Error> {
        let request = GetUserInfo {
            nonce: nonce(),
        }.authenticate(&self.credential);
        let response = self.client.send(&self.host, request)?;

        response.balances.into_iter()
            .filter_map(|(currency, balance)| {
                match currency.parse::<Currency>() {
                    Ok(currency) => Some((currency, balance)),
                    Err(ParseCurrencyError::InvalidOrUnsupportedCurrency(currency)) => None,
                }
            })
        .map(|(currency, balance)| {
            let currency = ccex::Currency::from(currency);
            ccex::Balance::new(currency, balance)
        })
        .map(Ok)
            .collect()
    }


    fn orderbook(&mut self, product: ccex::CurrencyPair) -> Result<ccex::Orderbook, Error> {
        self.orderbooks(&[product], 100)?
            .into_iter()
            .find(|&(_product, _)| _product == product)
            .map(|(_, orderbook)| orderbook)
            .ok_or_else(|| format_err!("No orderbook for {:?} returned from the server.", product))
    }

    fn orders(&mut self, product: ccex::CurrencyPair) -> Result<Vec<ccex::Order>, Error> {
        unimplemented!();
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
                ccex::Side::Ask => OrderInstruction::LimitSell,
                ccex::Side::Bid => OrderInstruction::LimitBuy,
            },
        };
        let request = request.authenticate(&self.credential);
        let response = self.client.send(&self.host, request)?;
        Ok(order.into())
    }
}

#[derive(Debug)]
pub struct AsyncExmoRestClient {
    pub threads: Vec<JoinHandle<()>>,
    pub orderbook_channel:		RefCell<(mpsc::Sender<ccex::CurrencyPair>, 	mpsc::Receiver<Result<ccex::Orderbook, Error>>)>,
    pub place_order_channel: 	RefCell<(mpsc::Sender<ccex::NewOrder>, 		mpsc::Receiver<Result<ccex::Order, Error>>)>,
    pub balances_channel: 		RefCell<(mpsc::Sender<()>, 					mpsc::Receiver<Result<Vec<ccex::Balance>, Error>>)>,
}

impl AsyncExchangeRestClient for AsyncExmoRestClient {
    fn balances<'a>(&'a self) -> Future<'a, Result<Vec<ccex::Balance>, Error>> {
        let (ref mut sender, _) = *self.balances_channel.borrow_mut();
        sender.send(()).unwrap();

        Future::new(move || {
            let (_, ref mut receiver) = *self.balances_channel.borrow_mut();
            receiver.recv().unwrap()
        })
    }

    fn orderbook<'a>(&'a self, product: ccex::CurrencyPair) -> Future<'a, Result<ccex::Orderbook, Error>> {
        let (ref mut sender, _) = *self.orderbook_channel.borrow_mut();
        sender.send(product).unwrap();

        Future::new(move || {
            let (_, ref receiver) = *self.orderbook_channel.borrow_mut();
            receiver.recv().unwrap()
        })
    }

    fn orders<'a>(&'a self, product: ccex::CurrencyPair) -> Future<'a, Result<Vec<ccex::Order>, Error>> {
        unimplemented!()
    }

    fn place_order<'a>(&'a self, new_order: ccex::NewOrder) -> Future<'a, Result<ccex::Order, Error>> {
        let (ref mut sender, _) = *self.place_order_channel.borrow_mut();
        sender.send(new_order).unwrap();

        Future::new(move || {
            let (_, ref mut receiver) = *self.place_order_channel.borrow_mut();
            receiver.recv().unwrap()
        })
    }
}

impl<Client> From<SyncExmoRestClient<Client>> for AsyncExmoRestClient
where Client: HttpClient {
    fn from(exmo: SyncExmoRestClient<Client>) -> Self {
        let (orderbook_channel, worker_orderbook_channel) = dual_channel();
        let orderbook_thread = {
            let mut exmo = exmo.clone();
            let (mut sender, mut receiver) = worker_orderbook_channel;
            thread::spawn(move || {
                for product in receiver.iter() {
                    sender.send(exmo.orderbook(product)).unwrap();
                }
            })
        };

        let (place_order_channel, worker_place_order_channel) = dual_channel();
        let place_order_thread = {
            let mut exmo = exmo.clone();
            let (mut sender, mut receiver) = worker_place_order_channel;
            thread::spawn(move || {
                for new_order in receiver.iter() {
                    sender.send(exmo.place_order(new_order)).unwrap();
                }
            })
        };

        let (balances_channel, worker_balances_channel) = dual_channel();
        let balances_thread = {
            let mut exmo = exmo.clone();
            let (mut sender, mut receiver) = worker_balances_channel;
            thread::spawn(move || {
                for _ in receiver.iter() {
                    sender.send(exmo.balances()).unwrap();
                }
            })
        };

        AsyncExmoRestClient {
            orderbook_channel: RefCell::new(orderbook_channel),
            place_order_channel: RefCell::new(place_order_channel),
            balances_channel: RefCell::new(balances_channel),
            threads: vec![
                orderbook_thread,
                place_order_thread,
                balances_thread,
            ],
        }
    }
}
