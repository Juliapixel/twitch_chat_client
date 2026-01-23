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
