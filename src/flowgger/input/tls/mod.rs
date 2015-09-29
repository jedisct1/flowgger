use flowgger::config::Config;
use openssl::bn::BigNum;
use openssl::dh::DH;
use openssl::ssl::*;
use openssl::x509::X509FileType;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub mod tls_input;
#[cfg(feature = "coroutines")]
pub mod tlsco_input;

pub use super::Input;

const DEFAULT_CERT: &'static str = "flowgger.pem";
const DEFAULT_CIPHERS: &'static str = "ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-CHACHA20-POLY1305:ECDHE-RSA-CHACHA20-POLY1305:ECDHE-ECDSA-AES128-SHA256:ECDHE-RSA-AES128-SHA256:ECDHE-ECDSA-AES128-SHA:ECDHE-RSA-AES128-SHA:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384:ECDHE-ECDSA-AES256-SHA384:ECDHE-RSA-AES256-SHA384:ECDHE-ECDSA-AES256-SHA:ECDHE-RSA-AES256-SHA:AES128-GCM-SHA256:AES256-GCM-SHA384:AES128-SHA256:AES256-SHA256:AES128-SHA:AES256-SHA:ECDHE-ECDSA-DES-CBC3-SHA:ECDHE-RSA-DES-CBC3-SHA:DES-CBC3-SHA:!aNULL:!eNULL:!EXPORT:!DES:!RC4:!MD5:!PSK:!aECDH:!EDH-DSS-DES-CBC3-SHA:!EDH-RSA-DES-CBC3-SHA:!KRB5-DES-CBC3-SHA";
const DEFAULT_COMPRESSION: bool = false;
const DEFAULT_FRAMING: &'static str = "line";
const DEFAULT_KEY: &'static str = "flowgger.pem";
const DEFAULT_LISTEN: &'static str = "0.0.0.0:6514";
#[cfg(feature = "coroutines")]
const DEFAULT_THREADS: usize = 1;
const DEFAULT_TIMEOUT: u64 = 3600;
const DEFAULT_TLS_METHOD: &'static str = "any";
const DEFAULT_VERIFY_PEER: bool = false;
const TLS_VERIFY_DEPTH: u32 = 6;

#[derive(Clone)]
pub struct TlsConfig {
    framing: String,
    threads: usize,
    arc_ctx: Arc<SslContext>
}

#[cfg(feature = "ecdh")]
fn set_ecdh(ctx: &mut SslContext) {
    ctx.set_ecdh_auto(true).unwrap();
}

#[cfg(not(feature = "ecdh"))]
fn set_ecdh(ctx: &mut SslContext) {
    let _ = ctx;
}

fn set_fs(ctx: &mut SslContext) {
    let p = BigNum::from_hex_str("87A8E61DB4B6663CFFBBD19C651959998CEEF608660DD0F25D2CEED4435E3B00E00DF8F1D61957D4FAF7DF4561B2AA3016C3D91134096FAA3BF4296D830E9A7C209E0C6497517ABD5A8A9D306BCF67ED91F9E6725B4758C022E0B1EF4275BF7B6C5BFC11D45F9088B941F54EB1E59BB8BC39A0BF12307F5C4FDB70C581B23F76B63ACAE1CAA6B7902D52526735488A0EF13C6D9A51BFA4AB3AD8347796524D8EF6A167B5A41825D967E144E5140564251CCACB83E6B486F6B3CA3F7971506026C0B857F689962856DED4010ABD0BE621C3A3960A54E710C375F26375D7014103A4B54330C198AF126116D2276E11715F693877FAD7EF09CADB094AE91E1A1597").unwrap();
    let g = BigNum::from_hex_str("3FB32C9B73134D0B2E77506660EDBD484CA7B18F21EF205407F4793A1A0BA12510DBC15077BE463FFF4FED4AAC0BB555BE3A6C1B0C6B47B1BC3773BF7E8C6F62901228F8C28CBB18A55AE31341000A650196F931C77A57F2DDF463E5E9EC144B777DE62AAAB8A8628AC376D282D6ED3864E67982428EBC831D14348F6F2F9193B5045AF2767164E1DFC967C1FB3F2E55A4BD1BFFE83B9C80D052B985D182EA0ADB2A3B7313D3FE14C8484B1E052588B9B7D2BBD2DF016199ECD06E1557CD0915B3353BBB64E0EC377FD028370DF92B52C7891428CDC67EB6184B523D1DB246C32F63078490F00EF8D647D148D47954515E2327CFEF98C582664B4C0F6CC41659").unwrap();
    let q = BigNum::from_hex_str("8CF83642A709A097B447997640129DA299B1A47D1EB3750BA308B0FE64F5FBD3").unwrap();
    let dh = DH::from_params(p, g, q).unwrap();
    ctx.set_tmp_dh(dh).unwrap();
    set_ecdh(ctx);
}

#[cfg(feature = "coroutines")]
fn get_default_threads(config: &Config) -> usize {
    config.lookup("input.tls_threads").map_or(DEFAULT_THREADS, |x| x.as_integer().
        expect("input.tls_threads must be an unsigned integer") as usize)
}

#[cfg(not(feature = "coroutines"))]
fn get_default_threads(_config: &Config) -> usize {
    1
}

pub fn config_parse(config: &Config) -> (TlsConfig, String, u64) {
    let listen = config.lookup("input.listen").map_or(DEFAULT_LISTEN, |x| x.as_str().
        expect("input.listen must be an ip:port string")).to_owned();
    let threads = get_default_threads(&config);
    let cert = config.lookup("input.tls_cert").map_or(DEFAULT_CERT, |x| x.as_str().
        expect("input.tls_cert must be a path to a .pem file")).to_owned();
    let key = config.lookup("input.tls_key").map_or(DEFAULT_KEY, |x| x.as_str().
        expect("input.tls_key must be a path to a .pem file")).to_owned();
    let ciphers = config.lookup("input.tls_ciphers").map_or(DEFAULT_CIPHERS, |x| x.as_str().
        expect("input.tls_ciphers must be a string with a cipher suite")).to_owned();
    let tls_method = match config.lookup("input.tls_method").map_or(DEFAULT_TLS_METHOD, |x| x.as_str().
        expect("input.tls_method must be a string with the TLS method")).to_lowercase().as_ref() {
            "any" | "sslv23" => SslMethod::Sslv23,
            "tlsv1" | "tlsv1.0" => SslMethod::Tlsv1,
            "tlsv1.1" => SslMethod::Tlsv1_1,
            "tlsv1.2" => SslMethod::Tlsv1_2,
            _ => panic!(r#"TLS method must be "any", "TLSv1.0", "TLSv1.1" or "TLSv1.2""#)
    };
    let verify_peer = config.lookup("input.tls_verify_peer").map_or(DEFAULT_VERIFY_PEER, |x| x.as_bool().
        expect("input.tls_verify_peer must be a boolean"));
    let ca_file: Option<PathBuf> = config.lookup("input.tls_ca_file").map_or(None, |x|
        Some(PathBuf::from(x.as_str().expect("input.tls_ca_file must be a path to a file"))));
    let compression = config.lookup("input.tls_compression").map_or(DEFAULT_COMPRESSION, |x| x.as_bool().
        expect("input.tls_compression must be a boolean"));
    let timeout = config.lookup("input.timeout").map_or(DEFAULT_TIMEOUT, |x| x.as_integer().
        expect("input.timeout must be an integer") as u64);
    let framing = if config.lookup("input.framed").map_or(false, |x| x.as_bool().
        expect("input.framed must be a boolean")) {
        "syslen"
    } else {
        DEFAULT_FRAMING
    };
    let framing = config.lookup("input.framing").map_or(framing, |x| x.as_str().
        expect(r#"input.framing must be a string set to "line", "nul" or "syslen""#)).to_owned();
    let mut ctx = SslContext::new(tls_method).unwrap();
    if verify_peer == false {
        ctx.set_verify(SSL_VERIFY_NONE, None);
    } else {
        ctx.set_verify_depth(TLS_VERIFY_DEPTH);
        ctx.set_verify(SSL_VERIFY_PEER | SSL_VERIFY_FAIL_IF_NO_PEER_CERT, None);
        if let Some(ca_file) = ca_file {
            if ctx.set_CA_file(&ca_file).is_err() {
                panic!("Unable to read the trusted CA file");
            }
        }
    }
    let mut opts = SSL_OP_CIPHER_SERVER_PREFERENCE | SSL_OP_NO_SESSION_RESUMPTION_ON_RENEGOTIATION;
    if compression == false {
        opts = opts | SSL_OP_NO_COMPRESSION;
    }
    ctx.set_options(opts);
    set_fs(&mut ctx);
    ctx.set_certificate_file(&Path::new(&cert), X509FileType::PEM).unwrap();
    ctx.set_private_key_file(&Path::new(&key), X509FileType::PEM).unwrap();
    ctx.set_cipher_list(&ciphers).unwrap();
    let arc_ctx = Arc::new(ctx);
    let tls_config = TlsConfig {
        framing: framing,
        threads: threads,
        arc_ctx: arc_ctx
    };
    (tls_config, listen, timeout)
}
