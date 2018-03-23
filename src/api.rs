use failure::Error;
use http;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Query {
    pub params: Vec<(String, String)>,
}

impl Query {
    pub fn with_capacity(capacity: usize) -> Self {
        Query {
            params: Vec::with_capacity(capacity),
        }
    }

    pub fn append_param<K, V>(&mut self, key: K, value: V)
    where
        K: Into<String>,
        V: Into<String>, {
        self.params.push((key.into(), value.into()));
    }

    pub fn to_string(&self) -> String {
        if self.params.is_empty() {
            String::new()
        } else {
            self.params
                .iter()
                .map(|&(ref name, ref value)| [name.as_str(), "=", value.as_str()])
                .collect::<Vec<[&str; 3]>>()
                .join(&"&")
                .into_iter()
                .collect()
        }
    }
}

pub trait HttpClient {
    fn send(&mut self, request: &http::Request<String>) -> Result<http::Response<String>, Error>;
}

// impl From<reqwest::Response> for HttpResponse {
//     fn from(mut response: reqwest::Response) -> Self {
//         let mut body = Vec::with_capacity(1024);
//         response.read_to_end(&mut body).unwrap();
//
//         let body = if body.is_empty() {
//             None
//         } else {
//             match String::from_utf8(body) {
//                 Ok(body) => Some(Payload::Text(body)),
//                 Err(body) => Some(Payload::Binary(body.into_bytes())),
//             }
//         };
//
//         let headers = response
//             .headers()
//             .iter()
//             .map(|header| Header::new(header.name(), header.value_string()))
//             .collect();
//
//         HttpResponse {
//             status: StatusCode::try_from(response.status().as_u16()).unwrap(),
//             headers,
//             body,
//         }
//     }
// }

// impl HttpClient for reqwest::Client {
//     fn send(&mut self, request: &http::Request<String>) -> Result<http::Response, Error> {
//         let mut request_builder = self.request(
//             request.method.clone().into(),
//             request.url().map_err(|e| format_err!("{}", e))?,
//         );
//
//         if let Some(ref headers) = request.headers {
//             let mut reqwest_headers = reqwest::header::Headers::new();
//             for header in headers {
//                 reqwest_headers.set_raw(header.name.clone(), header.values.as_slice().join("; "));
//             }
//             request_builder.headers(reqwest_headers);
//         }
//
//         if let Some(body) = request.body {
//             request_builder.body(body.to_owned());
//         }
//
//         let request = request_builder.build()?;
//         let response = self.execute(request)?.into();
//
//         Ok(response)
//     }
// }
