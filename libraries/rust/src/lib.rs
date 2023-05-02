pub mod client;
pub mod instructions;
pub mod registry;

use std::sync::Arc;

use anchor_lang::{prelude::Pubkey, AccountDeserialize};
use lookup_table_registry::RegistryAccount;
pub use lookup_table_registry::ID as LOOKUP_TABLE_REGISTRY_ID;
use solana_address_lookup_table_program_gateway::state::AddressLookupTable;
pub use solana_address_lookup_table_program_gateway::ID as LOOKUP_TABLE_ID;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{account::ReadableAccount, transaction::TransactionError};

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

#[derive(Debug, Clone)]
pub struct Entry {
    pub discriminator: u64,
    pub lookup_address: Pubkey,
    /// The list of addresses.
    ///
    /// It would be convenient to have this as a HashSet to remove duplicates,
    /// however this would conceal any duplicates and result in incorrect
    /// decisions made based on this. For example, if an account is repeated
    /// 255 times, a HashSet would only have one entry, while the table is actually
    /// full.
    pub addresses: Vec<Pubkey>,
}

pub fn derive_lookup_table_address(authority: &Pubkey, recent_block_slot: u64) -> Pubkey {
    solana_address_lookup_table_program_gateway::instruction::derive_lookup_table_address(
        authority,
        recent_block_slot,
    )
    .0
}

#[derive(thiserror::Error, Debug)]
pub enum LookupRegistryError {
    #[error("Registry does not exist {0}")]
    RegistryNotFound(Pubkey),
    #[error("Invalid argument: {0}")]
    InvalidArgument(String),
    #[error("Error with Solana client")]
    ClientError(#[from] solana_client::client_error::ClientError),
    #[error("Error with Anchor")]
    AnchorError(#[from] anchor_lang::error::Error),
}

pub type Result<T> = std::result::Result<T, LookupRegistryError>;
