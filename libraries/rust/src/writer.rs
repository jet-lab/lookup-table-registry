//! Helpers to interact with a registry

use std::{collections::HashSet, sync::Arc};

use anchor_lang::{prelude::Pubkey, AccountDeserialize};
use lookup_table_registry::{RegistryAccount, RegistryEntry};
use solana_address_lookup_table_program_gateway::state::AddressLookupTable;
use solana_client::{nonblocking::rpc_client::RpcClient, rpc_config::RpcSendTransactionConfig};
use solana_sdk::{
    account::ReadableAccount, address_lookup_table_account::AddressLookupTableAccount,
    commitment_config::CommitmentConfig, instruction::Instruction, signature::Signature,
    signer::Signer, transaction::Transaction,
};

use crate::common::{LookupRegistryError, LookupRegistryResult};
use crate::instructions::InstructionBuilder;

/// A writer client that creates and updates a registry
pub struct LookupRegistryWriter {
    rpc: Arc<RpcClient>,
    registry_address: Pubkey,
    builder: InstructionBuilder,
}

impl LookupRegistryWriter {
    /// Create a new lookup registry instance without checking if it exists on-chain
    pub fn new(rpc: &Arc<RpcClient>, authority: Pubkey, payer: Pubkey) -> Self {
        let builder = InstructionBuilder::new(authority, payer);

        Self {
            rpc: rpc.clone(),
            registry_address: builder.registry_address(),
            builder,
        }
    }

    /// Create a new empty lookup registry
    pub async fn new_or_create(
        rpc: &Arc<RpcClient>,
        authority: Pubkey,
        payer: Pubkey,
        signer: &dyn Signer,
    ) -> LookupRegistryResult<Self> {
        let builder = InstructionBuilder::new(authority, payer);
        let create_ix = builder.init_registry();

        // Check if a registry exists, and create it if it does not.
        let registry_address = builder.registry_address();
        // We don't check for network errors. If there's a connection error,
        // it'll likely also affect creating the registry.
        if rpc.get_account(&registry_address).await.is_ok() {
            return Ok(Self {
                rpc: rpc.clone(),
                registry_address,
                builder,
            });
        }

        let hash = rpc.get_latest_blockhash().await?;
        let transaction =
            Transaction::new_signed_with_payer(&[create_ix], Some(&payer), &[signer], hash);

        rpc.send_transaction_with_config(
            &transaction,
            RpcSendTransactionConfig {
                skip_preflight: true,
                ..Default::default()
            },
        )
        .await?;

        Ok(Self {
            rpc: rpc.clone(),
            registry_address: builder.registry_address(),
            builder,
        })
    }

    /// Get the registry account's state.
    ///
    /// Errors:
    /// - Registry has not been created
    pub async fn get_registry(&self) -> LookupRegistryResult<RegistryAccount> {
        let account = self.rpc.get_account(&self.registry_address).await?;
        let registry_account = RegistryAccount::try_deserialize(&mut account.data())?;
        Ok(registry_account)
    }

    /// Find lookup table addresses in the registry by a discriminator
    pub async fn find_lookup_table_addresses(
        &self,
        discriminator: u64,
    ) -> LookupRegistryResult<Vec<Pubkey>> {
        let registry = self.get_registry().await?;
        let addresses = registry
            .tables
            .iter()
            .filter_map(|table| {
                if table.discriminator == discriminator {
                    Some(table.table)
                } else {
                    None
                }
            })
            .collect();
        Ok(addresses)
    }

    /// Get a single lookup table in the registry
    pub async fn get_lookup_table(
        &self,
        lookup_table: Pubkey,
    ) -> LookupRegistryResult<(RegistryEntry, AddressLookupTableAccount)> {
        // Get the reigstry and lookup table
        let accounts = self
            .rpc
            .get_multiple_accounts(&[self.registry_address, lookup_table])
            .await?;
        // Elide bound checks
        assert_eq!(accounts.len(), 2);
        let (Some(registry_account), Some(lookup_table_account)) = (&accounts[0], &accounts[1]) else {
            return Err(LookupRegistryError::InvalidArgument("Registry account or lookup table not found".to_string()));
        };
        let registry_account = RegistryAccount::try_deserialize(&mut registry_account.data())?;
        // Check if the registry has the lookup table, otherwise it doesn't own it
        let Some(registry_entry) = registry_account
            .tables
            .iter()
            .find(|table| table.table == lookup_table)
         else {
            return Err(LookupRegistryError::InvalidArgument("Registry account does not own the lookup account".to_string()));
        };
        // Now deserialize the lookup table
        let table = {
            let table = AddressLookupTable::deserialize(lookup_table_account.data())
                .map_err(|e| LookupRegistryError::GeneralError(e.to_string()))?;
            AddressLookupTableAccount {
                key: lookup_table,
                addresses: table.addresses.to_vec(),
            }
        };
        Ok((registry_entry.clone(), table))
    }

    /// Create a new lookup table in the registry
    pub async fn create_lookup_table(
        &self,
        payer: Option<&Pubkey>,
        signer: &dyn Signer,
        discriminator: u64,
    ) -> LookupRegistryResult<(Pubkey, u64)> {
        // Introduce a small delay to prevent slot conflicts
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        let recent_slot = self.rpc.get_slot().await?;
        let (ix, table) = self.builder.create_lookup_table(recent_slot, discriminator);

        self.send_transaction(&[ix], payer, signer).await?;

        Ok((table, recent_slot))
    }

    /// Removes a lookup table by either deactivating or closing it.
    /// Lookup tables cannot be closed while active, and require deactivating for
    /// a number of slots before being closed.
    ///
    /// Callers can invoke this function twice to close a lookup table.
    pub async fn remove_lookup_table(
        &self,
        lookup_table: Pubkey,
        payer: Option<&Pubkey>,
        signer: &dyn Signer,
    ) -> LookupRegistryResult<()> {
        let ix = self.builder.remove_lookup_table(lookup_table);

        self.send_transaction(&[ix], payer, signer).await?;

        Ok(())
    }

    // TODO: can return the remaining space, or all the accounts that exist
    pub async fn append_to_lookup_table(
        &self,
        lookup_table: Pubkey,
        addresses: &[Pubkey],
        payer: Option<&Pubkey>,
        signer: &dyn Signer,
    ) -> LookupRegistryResult<()> {
        let (entry, table) = self.get_lookup_table(lookup_table).await?;
        let distinct_addresses = addresses
            .iter()
            .filter(|input| !table.addresses.contains(input))
            .cloned()
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let ix = self.builder.append_to_lookup_table(
            lookup_table,
            &distinct_addresses[..],
            entry.discriminator,
        );

        self.send_transaction(&[ix], payer, signer).await?;

        Ok(())
    }

    async fn send_transaction(
        &self,
        instructions: &[Instruction],
        payer: Option<&Pubkey>,
        signer: &dyn Signer,
    ) -> LookupRegistryResult<Signature> {
        let hash = self.rpc.get_latest_blockhash().await?;

        let transaction = Transaction::new_signed_with_payer(instructions, payer, &[signer], hash);

        Ok(self
            .rpc
            .send_and_confirm_transaction_with_spinner_and_config(
                &transaction,
                CommitmentConfig::finalized(),
                RpcSendTransactionConfig {
                    skip_preflight: true,
                    ..Default::default()
                },
            )
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use solana_sdk::{commitment_config::CommitmentConfig, signature::Keypair, signer::Signer};

    use super::*;

    #[tokio::test]
    #[allow(clippy::result_large_err)]
    #[ignore = "this test takes over 5 minutes. run it with './check full' or 'cargo test -- --include-ignored'"]
    async fn test_create_registry() -> LookupRegistryResult<()> {
        let authority_keypair = Keypair::new();
        let authority = authority_keypair.pubkey();

        let rpc = Arc::new(RpcClient::new_with_commitment(
            "http://localhost:8899".to_string(),
            CommitmentConfig::processed(),
        ));
        rpc.request_airdrop(&authority, 3_000_000_000).await?;

        let registry =
            LookupRegistryWriter::new_or_create(&rpc, authority, authority, &authority_keypair)
                .await?;

        // Create a lookup table in the registry
        let (lookup_table, _) = registry
            .create_lookup_table(None, &authority_keypair, 2)
            .await?;

        // Append to lookup table
        let mut addresses = Vec::with_capacity(32);
        addresses.extend_from_slice(&[Keypair::new().pubkey(); 2]);
        (0..11).for_each(|_| {
            addresses.push(Keypair::new().pubkey());
        });
        registry
            .append_to_lookup_table(lookup_table, &addresses, None, &authority_keypair)
            .await?;

        // Get the lookup table, it should have 12 entries
        let (entry, table) = registry.get_lookup_table(lookup_table).await?;
        assert_eq!(entry.discriminator, 2);
        assert_eq!(table.addresses.len(), 12);
        assert_eq!(entry.table, lookup_table);
        assert_eq!(table.key, lookup_table);

        // Create another lookup table, then close the first one
        let (lookup_table2, _) = registry
            .create_lookup_table(None, &authority_keypair, 2)
            .await?;

        // Deactivate the lookup table
        registry
            .remove_lookup_table(lookup_table, Some(&authority), &authority_keypair)
            .await?;
        // Trying to close it immediately after deactivating should fail
        registry
            .remove_lookup_table(lookup_table, Some(&authority), &authority_keypair)
            .await
            .unwrap_err();

        // Wait a while for the table to be closeable
        tokio::time::sleep(Duration::from_secs(240)).await;
        registry
            .remove_lookup_table(lookup_table, Some(&authority), &authority_keypair)
            .await?;

        // Get the registry, it should have 1 entry
        let registry_account = registry.get_registry().await?;
        assert_eq!(registry_account.len, 1);
        assert_eq!(registry_account.capacity, 2);
        assert_eq!(registry_account.tables.len(), 2);
        assert_eq!(registry_account.tables.get(0).unwrap().discriminator, 0);
        assert_eq!(registry_account.tables.get(1).unwrap().discriminator, 2);
        assert_eq!(registry_account.tables.get(1).unwrap().table, lookup_table2);

        Ok(())
    }
}
