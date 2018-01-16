pub mod rest;
pub mod ws;

use hmac::{Hmac, Mac};
use sha2;
use hex;
use serde_json;
use base64;
use serde::Serialize;
use Header;
use api;

fn private_headers<S>(payload: &S, key: &str, secret: &str) -> Vec<Header> where S: Serialize {
    let payload = serde_json::to_string(payload).unwrap();
    let payload = base64::encode(&payload);
    
    let mut signing_key = Hmac::<sha2::Sha384>::new(secret.as_bytes()).unwrap();
    signing_key.input(payload.as_bytes());
    let signature = hex::encode(signing_key.result().code());

    vec![
        ("X-GEMINI-APIKEY", key.to_owned()),
        ("X-GEMINI-PAYLOAD", payload),
        ("X-GEMINI-SIGNATURE", signature),
    ]
}

fn private_headers2<S>(payload: &S, key: &str, secret: &str) -> api::Headers where S: Serialize {
    let payload = serde_json::to_string(payload).unwrap();
    let payload = base64::encode(&payload);
    
    let mut signing_key = Hmac::<sha2::Sha384>::new(secret.as_bytes()).unwrap();
    signing_key.input(payload.as_bytes());
    let signature = hex::encode(signing_key.result().code());

    let mut headers = api::Headers::with_capacity(3);
    headers.insert("X-GEMINI-APIKEY".to_owned(), vec![key.to_owned()]);
    headers.insert("X-GEMINI-PAYLOAD".to_owned(), vec![payload]);
    headers.insert("X-GEMINI-SIGNATURE".to_owned(), vec![signature]);
    headers
}
