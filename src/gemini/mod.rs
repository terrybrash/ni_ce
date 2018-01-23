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
use failure::Error;

#[derive(Debug, Clone)]
pub struct Credential {
    pub key: String,
    pub secret: String,
}

fn private_headers<S>(payload: &S, credential: &Credential) -> Result<api::Headers, Error>
where S: Serialize {
    let payload = serde_json::to_string(payload)?;
    let payload = base64::encode(&payload);
    
    let mut signing_key = Hmac::<sha2::Sha384>::new(credential.secret.as_bytes()).map_err(|e| format_err!("{:?}", e))?;
    signing_key.input(payload.as_bytes());
    let signature = hex::encode(signing_key.result().code());

    let mut headers = api::Headers::with_capacity(3);
    headers.insert("X-GEMINI-APIKEY".to_owned(), credential.key.clone());
    headers.insert("X-GEMINI-PAYLOAD".to_owned(), payload);
    headers.insert("X-GEMINI-SIGNATURE".to_owned(), signature);
    Ok(headers)
}
