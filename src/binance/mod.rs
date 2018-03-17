use api::{Payload, Query, HttpRequest, HttpResponse, HttpClient, Headers, Header, Method};
use chrono::{Utc, DateTime};
use crate as ccex;
use failure::{Error};
use hex;
use serde_json;
use hmac::{Hmac, Mac};
use rust_decimal::Decimal as d128;
use serde::de::DeserializeOwned;
use sha2::Sha256;
use std::cell::{RefCell};
use std::collections::{HashMap};
use std::convert::{TryFrom, TryInto};
use url::Url;
use {Exchange};

pub struct Binance<Client: HttpClient> {
    pub host: Url,
    pub credential: ccex::Credential,
    pub http_client: RefCell<Client>,
}

impl<Client: HttpClient> Binance<Client> {
    pub fn new(credential: &ccex::Credential) -> Self {
        Binance {
            host: Url::parse("https://api.binance.com").unwrap(),
            credential: credential.clone(),
            http_client: RefCell::new(Client::new()),
        }
    }

    fn get_account(&self) -> Result<Account, Error> {
        let query = {
            let mut query = Query::with_capacity(3);
            query.insert_param("timestamp", Self::timestamp_now().to_string());
            let signature = Self::private_signature(&self.credential, query.to_string().trim_left_matches("?"))?;
            query.insert_param("signature", signature);
            query.to_string()
        };
        // let query = format!("?timestamp={}", Self::timestamp_now());
        // let signature = Self::private_seignature(&self.credential, query.as_str());
        // let query = format!("{}&{}", query, signature);
        let headers = Self::private_headers(&self.credential);
        let http_request = HttpRequest {
            method: Method::Get,
            host: self.host.as_str(),
            path: "/api/v3/account",
            query: Some(query.as_str()),
            headers: Some(headers),
            body: None,
        };

        let http_response = self.http_client.borrow_mut().send(&http_request)?;

        Self::deserialize_private_response(&http_response)
    }

    fn timestamp_now() -> u64 {
        let now = Utc::now();
        // now.timestamp() as u64 * 1000 + now.timestamp_subsec_millis() as u64
        now.timestamp() as u64 * 1000
    }

    fn private_signature(credential: &ccex::Credential, query: &str) -> Result<String, Error> {
        println!("{}", query);
        let mut mac = Hmac::<Sha256>::new(credential.secret.as_bytes())
            .map_err(|e| format_err!("{:?}", e))?;
        mac.input(query.as_bytes());
        Ok(hex::encode(mac.result().code().to_vec()))
    }

    fn private_headers(credential: &ccex::Credential) -> Headers {
        vec![
            Header::new("X-MBX-APIKEY", credential.key.clone()),
        ]
    }

    fn deserialize_private_response<T>(response: &HttpResponse) -> Result<T, Error> 
    where
        T: DeserializeOwned
    {
        Self::deserialize_public_response(response)
    }

    fn deserialize_public_response<T>(response: &HttpResponse) -> Result<T, Error>
    where
        T: DeserializeOwned
    {
        let body = match response.body {
            Some(Payload::Text(ref body)) => body,
            Some(Payload::Binary(_)) => panic!(),
            None => panic!(),
        };
        let result = serde_json::from_str(body)?;
        Ok(result)
    }
}

impl<Client: HttpClient> Exchange for Binance<Client> {
    fn name(&self) -> &'static str {
        "Binance"
    }

    fn maker_fee(&self) -> d128 {
        // 0.001 (0.1%)
        d128::new(1, 3)
    }

    fn taker_fee(&self) -> d128 {
        // 0.001 (0.1%)
        d128::new(1, 3)
    }

    fn min_quantity(&self, product: ccex::CurrencyPair) -> Option<d128> {
        use Currency::*;
        match product {
            ccex::CurrencyPair(ETH, BTC) => Some(d128::new(1, 3)),
            ccex::CurrencyPair(ETH, USDT) => Some(d128::new(1, 5)),
            _ => None,
        }
    }

    fn precision(&self) -> u32 {
        8
    }

    fn get_orderbooks(
        &self, 
        products: &[ccex::CurrencyPair],
    ) -> Result<HashMap<ccex::CurrencyPair, ccex::Orderbook>, Error> {
        // Binance doesn't support requests for multiple orderbooks in a single call so they have
        // to be done in separate requests.
        
        let mut orderbooks = HashMap::with_capacity(products.len());
        for &product in products.iter() {
            let query = {
                let mut query = Query::with_capacity(2);
                let CurrencyPair(product) = product.try_into()?;
                query.insert_param("symbol", product);
                query.insert_param("limit", "100");
                query.to_string()
            };
            let http_request = HttpRequest {
                method: Method::Get,
                host: self.host.as_str(),
                path: "/api/v1/depth",
                query: Some(query.as_str()),
                body: None,
                headers: None,
            };

            let http_response = self.http_client.borrow_mut().send(&http_request)?;

            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct Orderbook {
                last_update_id: u64,
                bids: Vec<(d128, d128, ())>,
                asks: Vec<(d128, d128, ())>,
            }
            let orderbook: Orderbook = Self::deserialize_public_response(&http_response)?;
            let asks = orderbook.asks.into_iter()
                .map(|(price, quantity, _)| ccex::Offer::new(price, quantity))
                .collect();
            let bids = orderbook.bids.into_iter()
                .map(|(price, quantity, _)| ccex::Offer::new(price, quantity))
                .collect();
            let orderbook = ccex::Orderbook::new(asks, bids);
            orderbooks.insert(product, orderbook);
        }
        Ok(orderbooks)
    }

    fn place_order(&self, order: ccex::NewOrder) -> Result<ccex::Order, Error> {
        unimplemented!()
    }

    fn get_balances(&self) -> Result<HashMap<ccex::Currency, d128>, Error> {
        let account = self.get_account()?;

        account.balances.into_iter()
            .filter_map(|balance| {
                match ccex::Currency::try_from(balance.asset) {
                    Ok(currency) => Some(Ok((currency, balance.free))),
                    Err(_) => None,
                }
            })
            .collect()
    }
}


#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Account {
    pub maker_commission: i32,
    pub taker_commission: i32,
    pub buyer_commission: i32,
    pub seller_commission: i32,
    pub can_trade: bool,
    pub can_withdraw: bool,
    pub can_deposit: bool,
    pub update_time: i64,
    pub balances: Vec<Balance>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Balance {
    pub asset: Currency,
    pub free: d128,
    pub locked: d128,
}

#[derive(Deserialize)]
pub struct Currency(String);

impl TryFrom<ccex::Currency> for Currency {
    type Error = Error;
    fn try_from(currency: ccex::Currency) -> Result<Self, Self::Error> {
        match currency {
            ccex::Currency::BTC => Ok(Currency(String::from("BTC"))),
            ccex::Currency::ETH => Ok(Currency(String::from("ETH"))),
            ccex::Currency::USDT => Ok(Currency(String::from("USDT"))),
            currency => Err(format_err!("{} isn't supported", currency)),
        }
    }
}

impl TryFrom<Currency> for ccex::Currency {
    type Error = Error;
    fn try_from(Currency(currency): Currency) -> Result<Self, Self::Error> {
        match currency.to_uppercase().as_str() {
            "BTC" => Ok(ccex::Currency::BTC),
            "ETH" => Ok(ccex::Currency::ETH),
            "USDT" => Ok(ccex::Currency::USDT),
            currency => Err(format_err!("{} isn't supported", currency)),
        }
    }
}

#[derive(Deserialize)]
pub struct CurrencyPair(String);

impl TryFrom<ccex::CurrencyPair> for CurrencyPair {
    type Error = Error;
    fn try_from(ccex::CurrencyPair(base, quote): ccex::CurrencyPair) -> Result<Self, Self::Error> {
        let Currency(base) = base.try_into()?;
        let Currency(quote) = quote.try_into()?;
        Ok(CurrencyPair(format!("{}{}", base, quote)))
    }
}

impl TryFrom<CurrencyPair> for ccex::CurrencyPair {
    type Error = Error;
    fn try_from(CurrencyPair(currency_pair): CurrencyPair) -> Result<Self, Self::Error> {
        // This has to be done manually because FOR SOME FUCKING REASON BINANCE
        // DOESNT USE A SEPARATOR IN CURRENCY PAIRS ON THEIR API WHY!!!!!!!!!!!
        // But they use a separator in currency pairs on their exchange????????
        // These people are fucking braindead holy shit.
        match currency_pair.to_uppercase().as_str() {
            "BTCUSDT" => Ok(ccex::CurrencyPair(ccex::Currency::BTC, ccex::Currency::USDT)),
            "ETHBTC" => Ok(ccex::CurrencyPair(ccex::Currency::ETH, ccex::Currency::BTC)),
            "ETHUSDT" => Ok(ccex::CurrencyPair(ccex::Currency::ETH, ccex::Currency::USDT)),
            currency_pair => Err(format_err!("{} isn't supported", currency_pair)),
        }
    }
}
