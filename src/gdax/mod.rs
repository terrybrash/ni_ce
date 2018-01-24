pub mod ws;
pub mod rest;

use chrono::{Utc};
use api;
use sha2;
use base64;
use hmac::{Hmac, Mac};
use std::io::Read;
use crate as ccex;
use failure::Error;

#[derive(Debug, Clone)]
pub struct Credential {
    pub key: String,
    pub secret: String,
    pub password: String,
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, Copy)]
pub enum CurrencyPair {
    #[serde(rename = "BTC-USD")] BTCUSD,
    #[serde(rename = "BCH-USD")] BCHUSD,
    #[serde(rename = "LTC-USD")] LTCUSD,
    #[serde(rename = "ETH-USD")] ETHUSD,
    #[serde(rename = "BCH-BTC")] BCHBTC,
    #[serde(rename = "LTC-BTC")] LTCBTC,
    #[serde(rename = "ETH-BTC")] ETHBTC,
}

impl From<CurrencyPair> for ccex::CurrencyPair {
    fn from(currency_pair: CurrencyPair) -> Self {
        match currency_pair {
            CurrencyPair::BTCUSD => (ccex::Currency::BTC, ccex::Currency::USD),
            CurrencyPair::BCHUSD => (ccex::Currency::BCH, ccex::Currency::USD),
            CurrencyPair::LTCUSD => (ccex::Currency::LTC, ccex::Currency::USD),
            CurrencyPair::ETHUSD => (ccex::Currency::ETH, ccex::Currency::USD),
            CurrencyPair::BCHBTC => (ccex::Currency::BCH, ccex::Currency::BTC),
            CurrencyPair::LTCBTC => (ccex::Currency::LTC, ccex::Currency::BTC),
            CurrencyPair::ETHBTC => (ccex::Currency::ETH, ccex::Currency::BTC),
        }
    }
}

impl From<ccex::CurrencyPair> for CurrencyPair{
    fn from(currency_pair: ccex::CurrencyPair) -> Self {
        match currency_pair {
             (ccex::Currency::BTC, ccex::Currency::USD) => CurrencyPair::BTCUSD,
             (ccex::Currency::BCH, ccex::Currency::USD) => CurrencyPair::BCHUSD,
             (ccex::Currency::LTC, ccex::Currency::USD) => CurrencyPair::LTCUSD,
             (ccex::Currency::ETH, ccex::Currency::USD) => CurrencyPair::ETHUSD,
             (ccex::Currency::BCH, ccex::Currency::BTC) => CurrencyPair::BCHBTC,
             (ccex::Currency::LTC, ccex::Currency::BTC) => CurrencyPair::LTCBTC,
             (ccex::Currency::ETH, ccex::Currency::BTC) => CurrencyPair::ETHBTC,
             pair => panic!("Unsupported currency pair: {:?}", pair),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, Copy)]
pub enum Currency {
    BTC,
    USD,
    ETH,
    LTC,
    BCH,
    GBP,
    EUR,
}

impl From<Currency> for ccex::Currency {
    fn from(currency: Currency) -> Self {
        match currency {
            Currency::BTC => ccex::Currency::BTC,
            Currency::USD => ccex::Currency::USD,
            Currency::ETH => ccex::Currency::ETH,
            Currency::LTC => ccex::Currency::LTC,
            Currency::BCH => ccex::Currency::BCH,
            Currency::GBP => ccex::Currency::GBP,
            Currency::EUR => ccex::Currency::EUR,
        }
    }
}

impl From<ccex::Currency> for Currency {
    fn from(currency: ccex::Currency) -> Self {
        match currency {
            ccex::Currency::BTC => Currency::BTC,
            ccex::Currency::USD => Currency::USD,
            ccex::Currency::ETH => Currency::ETH,
            ccex::Currency::LTC => Currency::LTC,
            ccex::Currency::BCH => Currency::BCH,
            ccex::Currency::GBP => Currency::GBP,
            ccex::Currency::EUR => Currency::EUR,
            currency => panic!("Unsupported currency: {:?}", currency)
        }
    }
}

#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all="lowercase")]
pub enum Side {
    Buy,
    Sell,
}

impl From<Side> for ccex::Side {
    fn from(side: Side) -> Self {
        match side {
            Side::Buy =>    ccex::Side::Bid,
            Side::Sell =>   ccex::Side::Ask,
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

fn private_headers<R>(request: &R, credential: &Credential) -> Result<api::Headers, Error>
where R: api::RestResource {
    let query = {
        let query = request.query();
        if query.len() > 0 {
            let query: Vec<String> = request.query().into_iter().map(|(name, value)| format!("{}={}", name, value)).collect();
            format!("?{}", query.as_slice().join("&"))
        } else {
            String::new()
        }
    };
    
    let body = String::from_utf8(request.body().unwrap())?;
    let timestamp = Utc::now().timestamp().to_string();
    let hmac_key = base64::decode(&credential.secret)?;
    let mut signature = Hmac::<sha2::Sha256>::new(&hmac_key).map_err(|e| format_err!("{:?}", e))?;
    signature.input(format!("{}{}{}{}{}", timestamp, request.method(), request.path(), query, body).as_bytes());
    let signature = base64::encode(&signature.result().code());

    let mut headers = api::Headers::with_capacity(6);
    headers.insert("Content-Type".to_owned(), "application/json".to_owned());
    headers.insert("CB-ACCESS-KEY".to_owned(), credential.key.clone());
    headers.insert("CB-ACCESS-SIGN".to_owned(), signature);
    headers.insert("CB-ACCESS-TIMESTAMP".to_owned(), timestamp);
    headers.insert("CB-ACCESS-PASSPHRASE".to_owned(), credential.password.clone());
    Ok(headers)
}