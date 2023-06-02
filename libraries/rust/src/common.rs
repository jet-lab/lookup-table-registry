use std::sync::Arc;

use anchor_lang::{prelude::Pubkey, AccountDeserialize};
use lookup_table_registry::RegistryAccount;
use solana_address_lookup_table_program_gateway::state::AddressLookupTable;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{account::ReadableAccount, transaction::TransactionError};

use crate::{Entry, LOOKUP_TABLE_REGISTRY_ID};

#[derive(Debug, Clone)]
pub struct Registry {
    pub authority: Pubkey,
    pub version: u8,
    pub tables: Vec<Entry>,
}

impl Registry {
    pub async fn fetch(rpc: &Arc<RpcClient>, authority: &Pubkey) -> Result<Self> {
        let registry_address =
            Pubkey::find_program_address(&[authority.as_ref()], &LOOKUP_TABLE_REGISTRY_ID).0;
        let registry = match rpc.get_account(&registry_address).await {
            Ok(value) => value,
            Err(e) => match e.get_transaction_error() {
                Some(e) if e == TransactionError::AccountNotFound => {
                    return Err(LookupRegistryError::RegistryNotFound(registry_address))
                }
                _ => return Err(LookupRegistryError::ClientError(e)),
            },
        };
        let registry = RegistryAccount::try_deserialize(&mut registry.data())?;

        let mut pubkeys = vec![];
        let addresses = registry
            .tables
            .iter()
            .filter(|entry| {
                if entry.discriminator > 1 {
                    pubkeys.push(entry.table);
                    true
                } else {
                    false
                }
            })
            .collect::<Vec<_>>();

        let accounts = rpc.get_multiple_accounts(&pubkeys).await.unwrap();
        let tables = accounts
            .into_iter()
            .zip(addresses)
            .filter_map(|(account, entry)| {
                let Some(account) = account else {
                return None;
            };
                let Ok(table) = AddressLookupTable::deserialize(account.data()) else {
                return None;
            };
                Some(Entry {
                    discriminator: entry.discriminator,
                    lookup_address: entry.table,
                    addresses: table.addresses.iter().copied().collect(),
                })
            })
            .collect();

        Ok(Self {
            authority: *authority,
            version: registry.version,
            tables,
        })
    }
}

#[derive(thiserror::Error, Debug)]
pub enum LookupRegistryError {
    #[error("Registry does not exist {0}")]
    RegistryNotFound(Pubkey),
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
    #[cfg(feature = "client")]
    #[error("Error with Solana client")]
    ClientError(#[from] solana_client::client_error::ClientError),
    #[error("Error with Anchor")]
    AnchorError(#[from] anchor_lang::error::Error),
    #[error("General error: {0}")]
    GeneralError(String),
}

pub type Result<T> = std::result::Result<T, LookupRegistryError>;
