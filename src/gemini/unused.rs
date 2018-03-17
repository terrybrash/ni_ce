use ccex;
use chrono;
// use hyper;
use serde_json;
use std::borrow::Cow;
use std::sync::mpsc;
use std::thread;
use tungstenite;
use websocket;
use config;
use std::thread::JoinHandle;
use ExchangeBuilder;
use Exchange;
use ccex::gemini::Credential;
use ccex::api::WebsocketClient;

#[derive(Debug)]
pub struct Gemini {
    pub credential: Credential,
    pub market_threads: Vec<JoinHandle<()>>,
    pub order_thread: JoinHandle<()>,
}

impl Exchange for Gemini {
    fn name(&self) -> &'static str {
        "gemini"
    }
}

impl Gemini {
    pub fn from_builder(builder: ExchangeBuilder) -> Self {
        let credential = Credential {
            key: builder.credential.key.clone(),
            secret: builder.credential.secret.clone(),
        };

        let mut market_threads = Vec::new();
        for product in builder.products {
            let thread = spawn_market_stream(builder.subscribers.clone(), builder.environment, product);
            market_threads.push(thread);
        }

        let order_thread = spawn_order_stream(builder.subscribers.clone(), builder.environment, &credential);

        Gemini {
            credential,
            market_threads,
            order_thread,
        }
    }
}

fn spawn_market_stream(subscribers: Vec<mpsc::Sender<ccex::ExchangeEvent>>, environment: ccex::Environment, product: ccex::CurrencyPair) -> JoinHandle<()> {
    thread::spawn(move || market_stream(subscribers, environment, product))
}

fn market_stream(subscribers: Vec<mpsc::Sender<ccex::ExchangeEvent>>, environment: ccex::Environment, product: ccex::CurrencyPair) {
    use ccex::gemini::ws::{interface, model};

    let request = interface::GetMarketStream {
        product: product.into(),
    };

    let mut client = ccex::api::TungsteniteClient::connect(environment.into(), request).unwrap();

    while let Ok(message) = client.recv() {
        // TODO: this is ridiculous
        let ccex::gemini::ws::model::market::ExchangeEvents(events) = (message, product.into()).into();
        for event in events {
            for sub in &subscribers {
                sub.send(event.clone());
            }
        }
    }
}

fn spawn_order_stream(subscribers: Vec<mpsc::Sender<ccex::ExchangeEvent>>, environment: ccex::Environment, credential: &Credential) -> JoinHandle<()> {
    let credential = credential.clone();
    thread::spawn(move || order_stream(subscribers, environment, &credential))
}

pub fn order_stream(subscribers: Vec<mpsc::Sender<ccex::ExchangeEvent>>, environment: ccex::Environment, credential: &Credential) {
    use ccex::gemini::ws::{interface};
    use ccex::api::*;
    
    let request = interface::GetOrderStream::new(nonce()).authenticate(credential);
    let mut client = ccex::api::TungsteniteClient::connect(environment.into(), request).unwrap();
    while let Ok(message) = client.recv() {
        println!("{:?}", message);
    }
}

fn nonce() -> i64 {
    let now = chrono::Utc::now();
    now.timestamp() * 1000 + now.timestamp_subsec_millis() as i64
}