use reqwest;
use serde_json;

#[derive(Debug, Copy, Clone)]
pub enum Environment {
    Production,
    Sandbox,
}

impl Environment {
    fn base_address(&self) -> &'static str {
        match *self {
            Environment::Production => "https://api.gdax.com",
            Environment::Sandbox    => "https://api-public.sandbox.gdax.com",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Product {
    pub id: String,
    pub base_currency: String,
    pub quote_currency: String,
    pub base_min_size: String,
    pub base_max_size: String,
    pub quote_increment: String,
    pub display_name: String,
    pub status: String,
    pub margin_enabled: bool,
    pub status_message: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Ticker {
    pub trade_id: i64,
    pub price: String,
    pub size: String,
    pub bid: String,
    pub ask: String,
    pub volume: String,
    pub time: String,
}

#[derive(Debug, Deserialize)]
pub struct Trade {
    pub time: String,
    pub trade_id: i64,
    pub price: String,
    pub size: String,
    pub side: String,
}

#[derive(Debug, Deserialize)]
pub struct BookLevel1 {
    pub sequence: i64,
    pub bids: Vec<(String, String, i64)>,
    pub asks: Vec<(String, String, i64)>,
}

#[derive(Debug, Deserialize)]
pub struct BookLevel2 {
    pub sequence: i64,
    pub bids: Vec<(String, String, i64)>,
    pub asks: Vec<(String, String, i64)>,
}

#[derive(Debug, Deserialize)]
pub struct BookLevel3 {
    pub sequence: i64,
    pub bids: Vec<(String, String, String)>,
    pub asks: Vec<(String, String, String)>,
}

#[derive(Debug, Deserialize)]
pub struct BidAsk {
    pub price: f64,
    pub amount: f64,
}

#[derive(Debug, Deserialize)]
pub struct Book {
    pub bids: Vec<BidAsk>,
    pub asks: Vec<BidAsk>,
}

#[derive(Debug, Deserialize)]
pub struct Error {
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct Time {
    pub iso: String,
    pub epoch: f64,
}

// pub fn get_book_50(client: &mut reqwest::Client, product: &str) -> Result<Book, ()> {
//     let request = format!("{}/products/{}/book?level=2", RestBaseAddress::PRODUCTION.unwrap(), product);
//     let response: BookLevel2 = serde_json::from_reader(client.get(&request).send().unwrap()).map_err(|_| ())?;

//     let asks = response.asks.iter().map(|&(ref price, ref size, order_count)| {
//         BidAsk {
//             price: price.parse().unwrap(),
//             amount: size.parse().unwrap(),
//         }
//     });
//     let bids = response.bids.iter().map(|&(ref price, ref size, order_count)| {
//         BidAsk {
//             price: price.parse().unwrap(),
//             amount: size.parse().unwrap(),
//         }
//     });

//     let book = Book {
//         asks: asks.collect(),
//         bids: bids.collect(),
//     };

//     Ok(book)
// }

