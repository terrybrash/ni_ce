use failure::Error;
use http;
use reqwest;

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

impl HttpClient for reqwest::Client {
    fn send(&mut self, request: &http::Request<String>) -> Result<http::Response<String>, Error> {
        let method = request.method().as_str().parse()?;
        let mut headers = reqwest::header::Headers::new();
        for (key, value) in request.headers() {
            headers.set_raw(key.as_str().to_owned(), value.to_str()?);
        }

        let request = self.request(method, request.uri().to_string().as_str())
            .body(request.body().clone())
            .headers(headers)
            .build()?;

        let mut response = self.execute(request)?;

        // TODO: add headers
        http::response::Builder::new()
            .status(response.status().as_u16())
            .body(response.text()?)
            .map_err(|e| format_err!("{}", e))
    }
}
