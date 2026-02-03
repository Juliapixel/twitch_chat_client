#[macro_export]
macro_rules! res {
    ($file: literal) => {
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/res/", $file))
    };
}

#[macro_export]
macro_rules! res_str {
    ($file: literal) => {
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/res/", $file))
    };
}

pub fn default_client() -> reqwest::Client {
    reqwest::Client::builder()
        .brotli(true)
        .deflate(true)
        .gzip(true)
        .user_agent(concat!(
            env!("CARGO_PKG_NAME"),
            "/",
            env!("CARGO_PKG_VERSION"),
        ))
        .build()
        .unwrap()
}
