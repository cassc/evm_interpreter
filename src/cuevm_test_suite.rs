use std::collections::BTreeMap;

use revm::primitives::{Address, Bytes, HashMap, B256, U256};
use serde::{de, Deserialize};

#[derive(Debug, PartialEq, Eq, Deserialize, Hash, Clone)]
pub struct BytesAddress(Bytes);

impl From<BytesAddress> for Address {
    fn from(val: BytesAddress) -> Self {
        let bytes = val.0;
        let padded = {
            if bytes.len() < 20 {
                let mut padded = vec![0; 20];
                padded[20 - bytes.len()..].copy_from_slice(&bytes);
                padded
            } else {
                bytes.to_vec()
            }
        };
        Address::from_slice(&padded)
    }
}

pub fn deserialize_str_as_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: de::Deserializer<'de>,
{
    let string = String::deserialize(deserializer)?;

    if let Some(stripped) = string.strip_prefix("0x") {
        u64::from_str_radix(stripped, 16)
    } else {
        string.parse()
    }
    .map_err(serde::de::Error::custom)
}

/// Deserialize a hex string as a `Bytes` object, padding zeros if necessary.
pub fn deserialize_str_as_bytes<'de, D>(deserializer: D) -> Result<Bytes, D::Error>
where
    D: de::Deserializer<'de>,
{
    let string = String::deserialize(deserializer)?;
    let is_odd = string.len() % 2 != 0;

    let padded = {
        if let Some(stripped) = string.strip_prefix("0x") {
            if is_odd {
                format!("0{}", stripped)
            } else {
                stripped.to_owned()
            }
        } else if is_odd {
            format!("0{}", string)
        } else {
            string.to_owned()
        }
    };

    Ok(Bytes::from(padded))
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct CuEvmTestSuite(pub BTreeMap<String, CuEvmTestUnit>);

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountInfo {
    pub balance: U256,
    pub code: Bytes,
    #[serde(deserialize_with = "deserialize_str_as_u64")]
    pub nonce: u64,
    pub storage: HashMap<U256, U256>,
}

//NOTE extra fields from cuevm are ignored
// previous_hash is missing from cuevm test output
#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Env {
    pub current_coinbase: BytesAddress,
    pub current_difficulty: U256,
    pub current_gas_limit: U256,
    pub current_number: U256,
    pub current_timestamp: U256,
    pub current_base_fee: Option<U256>,
    pub previous_hash: Option<B256>,

    pub current_random: Option<B256>,
    pub current_beacon_root: Option<B256>,
    pub current_withdrawals_root: Option<B256>,

    pub parent_blob_gas_used: Option<U256>,
    pub parent_excess_blob_gas: Option<U256>,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
pub struct CuEvmTestUnit {
    pub env: Env,
    pub pre: HashMap<BytesAddress, AccountInfo>,
    pub post: Vec<CuEvmTest>,
}

/// A test unit in `post`
#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CuEvmTest {
    /// A transaction used by CuEvm
    pub msg: CuEvmMsg,
    /// Traces of stacks
    pub traces: Vec<CuEvmTrace>,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CuEvmMsg {
    pub sender: BytesAddress,
    pub value: U256,
    pub to: Option<BytesAddress>,
    pub nonce: U256,
    pub origin: BytesAddress,
    pub gas_price: Option<U256>,
    pub gas_limit: U256,
    pub data: Bytes,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CuEvmTrace {
    pub address: BytesAddress,
    pub pc: usize,
    pub opcode: u8,
    pub stack: CuEvmStack,
}

#[derive(Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CuEvmStack {
    pub data: Vec<Bytes>,
}
