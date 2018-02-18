pub mod rest;
// pub mod ws;

use api::{Header, Headers};
use base64;
use failure::Error;
use hex;
use hmac::{Hmac, Mac};
use serde::Serialize;
use serde_json;
use sha2::{Sha384};

#[derive(Debug, Clone)]
pub struct Credential {
    pub key: String,
    pub secret: String,
}

fn private_headers<S>(payload: &S, credential: &Credential) -> Result<Headers, Error>
where S: Serialize {
    let payload = serde_json::to_string(payload)
        .map(|json| base64::encode(json.as_bytes()))?;
    
    let mut mac = Hmac::<Sha384>::new(credential.secret.as_bytes()).map_err(|e| format_err!("{:?}", e))?;
    mac.input(payload.as_bytes());
    let signature = hex::encode(mac.result().code());

    let headers = vec![
        Header::new("X-GEMINI-APIKEY", credential.key.clone()),
        Header::new("X-GEMINI-PAYLOAD", payload),
        Header::new("X-GEMINI-SIGNATURE", signature),
    ];
    Ok(headers)
}
