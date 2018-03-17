use reqwest;
use serde_json;
use serde;

#[derive(Debug)]
pub enum Environment {
    Production,
}

mod model {
    #[derive(Debug, Serialize, Deserialize)]
    pub struct BidAsk {
        pub price: String,
        pub size: String,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub struct Book {
        pub ask: Vec<BidAsk>,
        pub bid: Vec<BidAsk>,
        pub timestamp: Option<String>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Order {
        pub cum_quantity: Option<String>,
        pub stop_price: Option<String>,
        pub price: Option<String>,
        pub quantity: Option<String>,
        pub expire_time: Option<String>,
        pub updated_at: Option<String>,
        pub status: String,
        pub side: String,
        pub symbol: String,
        pub time_in_force: String,
        // pub type: String,
        pub id: i64,
        pub created_at: Option<String>,
        pub client_order_id: String,
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub enum OrderSide {
        Buy,
        Sell,
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub enum OrderType {
        Limit,
        Market,
        StopLimit,
        StopMarket,
    }

    #[derive(Debug, Serialize, Deserialize)]
    pub enum TimeInForce {
        /// Good Till Cancel
        GTC,

        /// Immediate or Cancel
        IOC,

        /// Fill or Kill
        FOK,

        /// 24 hours
        Day,

        /// Good Till Date
        GTD,
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct OrderForm {
        // Required
        pub symbol: String,
        pub side: OrderSide,
        pub quantity: String,
        
        // Optional
        pub client_order_id: Option<String>,
        pub type_: Option<OrderType>,
        pub time_in_force: Option<TimeInForce>,
        pub price: Option<String>,
        pub stop_price: Option<String>,
        pub expire_time: Option<String>,
        /// Strict validate amount and price precision without roudning
        pub strict_validate: Option<bool>,
    }

    #[derive(Debug, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct Balance {
        pub currency: Option<String>,
        pub available: Option<String>,
        pub reserved: Option<String>,
    }

    #[derive(Fail, Debug, Serialize, Deserialize)]
    #[fail(display = "{} ({})", code, message)]
    pub struct Error {
        pub code: i32,
        pub message: String,
        pub description: Option<String>,
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct Response {
        pub error: Error,
    }
}

#[derive(Debug, Fail)]
#[fail(display = "HitBTC error")]
pub enum Error {
    #[fail(display = "Reqwest error: {}", _0)]
    Reqwest(#[cause] reqwest::Error),
    #[fail(display = "SerdeJson error: {}", _0)]
    SerdeJson(#[cause] serde_json::error::Error),
    #[fail(display = "HitBTC error {}", _0)]
    Hitbtc(#[cause] model::Error),
}

type Result<T> = ::std::result::Result<T, Error>;

fn base_url(environment: Environment) -> &'static str {
    match environment {
        Environment::Production => "https://api.hitbtc.com/api/2",
    }
}

trait RequestExecute {
    fn execute<T: serde::de::DeserializeOwned>(&mut self) -> Result<T>;
}

impl RequestExecute for reqwest::RequestBuilder {
    fn execute<T: serde::de::DeserializeOwned>(&mut self) -> Result<T> {
        let response = self.send().map_err(Error::Reqwest)?;
        if response.status() == reqwest::StatusCode::Ok {
            Ok(serde_json::from_reader(response).map_err(Error::SerdeJson)?)
        } else {
            let error_response: model::Response = serde_json::from_reader(response).map_err(Error::SerdeJson)?;
            Err(Error::Hitbtc(error_response.error))
        }
    }    
}

pub fn get_book(client: &reqwest::Client, env: Environment, product: &str, limit: usize) -> Result<model::Book> {
    client.get(&format!("{}/public/orderbook/{}?limit={}", base_url(env), product, limit))
        .execute()
}

pub fn get_orders(client: &reqwest::Client, env: Environment, user: &str, password: &str, product: Option<&str>) -> Result<Vec<model::Order>> {
    client.get(&format!("{}/order", base_url(env)))
        .basic_auth(user, Some(password))
        .execute()
}

pub fn send_order(client: &reqwest::Client, env: Environment, user: &str, password: &str, order: &model::OrderForm) -> Result<model::Order> {
    client.post(&format!("{}/order", base_url(env)))
        .basic_auth(user, Some(password))
        .form(order)
        .execute()
}

pub fn get_balance(client: &reqwest::Client, env: Environment, user: &str, password: &str) -> Result<Vec<model::Balance>> {
    client.get(&format!("{}/account/balance", base_url(env)))
        .basic_auth(user, Some(password))
        .execute()
}
