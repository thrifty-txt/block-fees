//! Compute the sum of the transactions fees of a bitcoin block in mainnet using esplora API
//!
//! Example:
//! ```
//! $ cargo run --release -- 000000000000000001a4f543e574f6e9d6e6e7c4ea2b84a5c1d5193a0a295995
//! 11145972
//! ```
//! 
#![warn(clippy::pedantic)]
#[macro_use] extern crate log;

use std::{error::Error,fmt::Display};
use reqwest::{Client, Url};
use serde::{Serialize, Deserialize};
use rust_decimal::prelude::*;
use rust_decimal_macros::dec;

// Documentation: https://github.com/Blockstream/esplora/blob/master/API.md
const ESPLORA_API: &str = "https://blockstream.info/api/";
const MAGIC_NUMBER: &str = "00000000";

#[tokio::main]
async fn main() {
    env_logger::init();

    let block_hash = if let Some(s) =  std::env::args().nth(1){ s + "/" } else {
        error!("Please provide a Bitcoin mainnet block hash.");
        std::process::exit(1)
    };
    if !block_hash.starts_with(MAGIC_NUMBER) {
        error!("Invalid hash. Please provide a Bitcoin mainnet block hash.");
        std::process::exit(1)
    }
    // FIXME: could panic
    let client = Client::new();
    let url = Url::parse(ESPLORA_API).expect("URL parsing failed.");

    // TODO: more informative error messages
    let block_transaction_ids = get_block_transaction_ids(&block_hash, &client, &url)
        .await.unwrap_or_else(|error|{
            error!("Error fetching transactions from block ID: {:?}", error);
            std::process::exit(1)
    });

    //TODO: improve asynchronicity
    let mut handles = Vec::with_capacity(block_transaction_ids.len());
    for (i, txid) in block_transaction_ids.into_iter().enumerate(){
        let client = client.clone();
        let url = url.clone();
        info!("Spawning transacion {i}");
        let handle = tokio::spawn(async move{
            let fee = get_fee_from_txid(txid.as_ref(), &client, &url).await.unwrap_or_else(|error|{
                error!("Error fetching fee from one or more transactions: {:?}", error);
                std::process::exit(1)
            });
            info!("Finished transaction {i}");
            fee
        });
        handles.push(handle); 
    }    

    let mut sum = dec!(0);
    for handle in handles{
        let fee = handle.await.expect("Failed to join a thread.");
        sum += fee;
    }
    println!("{sum}");
}

#[derive(Debug, Serialize, Deserialize)]
struct Transaction{
    fee: Decimal
}

#[derive(Debug)]
enum AppError {
    ReqwestError(reqwest::Error),
    UrlParseError(url::ParseError)
}
impl Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppError::ReqwestError(error) => write!(f, "{error}"),
            AppError::UrlParseError(error) => write!(f, "{error}")

        }
    }
}
impl Error for AppError{}
impl From<reqwest::Error> for AppError{
    fn from(err: reqwest::Error) -> Self {
        AppError::ReqwestError(err)
    }
}
impl From<url::ParseError> for AppError{
    fn from(err: url::ParseError) -> Self {
        AppError::UrlParseError(err)
    }
}
    

async fn get_block_transaction_ids(block_hash: &str, client: &Client, url: &Url) -> Result<Vec<String>, AppError>{
    let url = url.join("block/")?.join(block_hash)?.join("txids")?;
    let response = client.get(
            url
        )
        .send()
        .await?
        .json()
        .await?;
    Ok(response)
}

async fn get_fee_from_txid(txid: &str, client: &Client, url: &Url) -> Result<Decimal, AppError>{
    let response: Transaction = client.get(
        url.join("tx/")?.join(txid)?
        )
        .send()
        .await?
        .json()
        .await?;
    Ok(response.fee)
}
