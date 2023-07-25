use anchor_lang::{prelude::Pubkey, AccountDeserialize};
use async_trait::async_trait;
use lookup_table_registry::RegistryAccount;
use solana_address_lookup_table_program_gateway::state::AddressLookupTable;
use solana_client::{client_error::ClientError, nonblocking::rpc_client::RpcClient};
use solana_sdk::{
    account::{Account, ReadableAccount},
    transaction::TransactionError,
};

use crate::{Entry, LOOKUP_TABLE_REGISTRY_ID};

#[derive(Debug, Clone)]
pub struct Registry {
    pub authority: Pubkey,
    pub version: u8,
    pub tables: Vec<Entry>,
}

impl Registry {
    pub async fn fetch(rpc: &(impl AccountReader + ?Sized), authority: &Pubkey) -> Result<Self> {
        let registry_address =
            Pubkey::find_program_address(&[authority.as_ref()], &LOOKUP_TABLE_REGISTRY_ID).0;
        let registry = match rpc.get_account(&registry_address).await {
            Ok(value) => value,
            Err(e) => {
                if e.is_account_not_found() {
                    return Err(LookupRegistryError::RegistryNotFound(registry_address));
                } else {
                    return Err(LookupRegistryError::AccountReadError(e));
                }
            }
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
    #[error("Error reading account: {0}")]
    AccountReadError(Box<dyn AccountReaderError>),
    #[error("Error with Anchor")]
    AnchorError(#[from] anchor_lang::error::Error),
    #[error("General error: {0}")]
    GeneralError(String),
}

pub type Result<T> = std::result::Result<T, LookupRegistryError>;

#[async_trait]
pub trait AccountReader: Send + Sync {
    async fn get_multiple_accounts(
        &self,
        pubkeys: &[Pubkey],
    ) -> std::result::Result<Vec<Option<Account>>, Box<dyn AccountReaderError>>;

    async fn get_account(
        &self,
        pubkey: &Pubkey,
    ) -> std::result::Result<Account, Box<dyn AccountReaderError>>;
}

impl_AccountReader!(RpcClient);

/// These are only to be used by the macros defined within this crate.
pub mod __private {
    pub use async_trait::async_trait;
    pub use solana_sdk::account::Account;
    pub use solana_sdk::pubkey::Pubkey;
}

/// Delegates the trait to a type with identical methods
#[macro_export]
macro_rules! impl_AccountReader {
    ($Type:ty) => {
        #[$crate::common::__private::async_trait]
        impl $crate::common::AccountReader for $Type {
            async fn get_multiple_accounts(
                &self,
                pubkeys: &[$crate::common::__private::Pubkey],
            ) -> std::result::Result<
                Vec<Option<$crate::common::__private::Account>>,
                Box<dyn $crate::common::AccountReaderError>,
            > {
                <$Type>::get_multiple_accounts(self, pubkeys)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn $crate::common::AccountReaderError>)
            }

            async fn get_account(
                &self,
                pubkey: &$crate::common::__private::Pubkey,
            ) -> std::result::Result<
                $crate::common::__private::Account,
                Box<dyn $crate::common::AccountReaderError>,
            > {
                <$Type>::get_account(self, pubkey)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn $crate::common::AccountReaderError>)
            }
        }
    };
}
use impl_AccountReader;

pub trait AccountReaderError: std::fmt::Display + std::fmt::Debug + Send + Sync + 'static {
    fn is_account_not_found(&self) -> bool;
}

impl AccountReaderError for ClientError {
    fn is_account_not_found(&self) -> bool {
        self.get_transaction_error() == Some(TransactionError::AccountNotFound)
    }
}

impl AccountReaderError for anyhow::Error {
    fn is_account_not_found(&self) -> bool {
        false
    }
}
