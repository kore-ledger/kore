use std::env;

pub fn build_address_http() -> String {
    env::var("KORE_HTTP_ADDRESS").unwrap_or("0.0.0.0:3000".to_owned())
}

pub fn build_address_https() -> String {
    env::var("KORE_HTTPS_ADDRESS").unwrap_or_default()
}

pub fn build_https_cert() -> String {
    env::var("KORE_HTTPS_CERT").unwrap_or_default()
}

pub fn build_https_private_key() -> String {
    env::var("KORE_HTTPS_PRIVATE_KEY").unwrap_or_default()
}

pub fn build_doc() -> bool {
    env::var("KORE_HTTPS_DOC").unwrap_or_default() == "true"
}
