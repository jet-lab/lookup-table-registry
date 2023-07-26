use std::{
    collections::HashSet,
    ops::Deref,
    sync::{Arc, RwLock},
    time::Duration,
};

use anchor_lang::prelude::Pubkey;
use endorphin::policy::TTLPolicy;
use solana_sdk::{
    address_lookup_table_account::AddressLookupTableAccount, instruction::Instruction,
};

use crate::common::{AccountReader, Registry};

/// A client suitable for querying instruction registries for authorities.
pub struct LookupRegistryReader<A> {
    rpc: A,
    cache: Arc<RwLock<endorphin::HashMap<Pubkey, Registry, TTLPolicy>>>,
}

impl<A: Clone> Clone for LookupRegistryReader<A> {
    fn clone(&self) -> Self {
        Self {
            rpc: self.rpc.clone(),
            cache: self.cache.clone(),
        }
    }
}

impl<A> LookupRegistryReader<A> {
    pub fn new(rpc: A) -> Self {
        Self {
            rpc,
            cache: Arc::new(RwLock::new(endorphin::HashMap::new(TTLPolicy::new()))),
        }
    }
}

impl<A: Deref<Target = X>, X: AccountReader> LookupRegistryReader<A> {
    /// Fetch the latest registry addresses for specific authorities.
    ///
    /// Returns the authorities that were not found or otherwise incurred some error
    pub async fn update_registries(&self, authorities: &[Pubkey]) -> Vec<Pubkey> {
        let mut errors = Vec::with_capacity(authorities.len());
        for authority in authorities {
            let Ok(registry) = Registry::fetch(&*self.rpc, authority).await else {
                errors.push(*authority);
                continue;
            };
            let mut writer = self.cache.write().unwrap();
            writer.insert(*authority, registry, Duration::from_secs(3600));
        }
        errors
    }

    /// Returns all the lookup tables that are in the registries owned by the
    /// provided authorities.
    pub async fn get_tables(&self, authorities: &[Pubkey]) -> Vec<AddressLookupTableAccount> {
        let mut ret = vec![];
        for authority in authorities {
            if let Some(r) = self.get_registry(authority).await {
                ret.extend(r.tables.into_iter().map(Into::into));
            }
        }
        ret
    }

    /// Find lookup addresses such that as many accounts as possible in the provided
    /// instructions use lookup addresses.
    pub fn find_addresses(
        &self,
        instructions: &[Instruction],
        authorities: &[Pubkey],
    ) -> FindAddressesResult {
        let mut accounts = HashSet::with_capacity(256);
        for ix in instructions {
            accounts.insert(ix.program_id);
            for account in &ix.accounts {
                accounts.insert(account.pubkey);
            }
        }
        let distinct = accounts.len();
        // TODO: we can use the program in the instruction to lookup discriminators to use

        let mut matches = vec![];
        for authority in authorities {
            let reader = self.cache.read().unwrap();
            let Some(registry) = reader.get(authority) else {
                continue;
            };
            // We have a registry, find matches.
            // For now we inefficiently go through all entries
            for table in registry.tables.iter() {
                // if accounts.len() <= 4 {
                //     break;
                // }
                // Create a manual intersection
                let len_a = table.addresses.len();
                let len_b = accounts.len();
                let mut intersection = HashSet::with_capacity(len_a.min(len_b));
                if len_a < len_b {
                    for address in &table.addresses {
                        if accounts.contains(address) {
                            intersection.insert(*address);
                        }
                    }
                } else {
                    for address in &accounts {
                        if table.addresses.contains(address) {
                            intersection.insert(*address);
                        }
                    }
                }

                // Use an account if it reduces 5 or more addresses
                if intersection.len() > 1 {
                    matches.push(table.lookup_address);
                    // TODO: can we use HashSet::difference()?
                    for address in intersection {
                        accounts.remove(&address);
                    }
                }
            }
        }
        // Would be useful to use the program in the instruction to get
        // a possible registry discriminator

        FindAddressesResult {
            matches,
            distinct,
            unmatched: accounts.len(),
        }
    }

    pub async fn get_registry(&self, authority: &Pubkey) -> Option<Registry> {
        let registry = {
            let reader = self.cache.read().unwrap();
            reader.get(authority).cloned()
        };
        match registry {
            Some(registry) => Some(registry),
            None => {
                let Ok(registry) = Registry::fetch(&*self.rpc, authority).await else {
                    return None;
                };
                let mut writer = self.cache.write().unwrap();
                writer.insert(*authority, registry, Duration::from_secs(3600));
                writer.get(authority).cloned()
            }
        }
    }
}

pub struct FindAddressesResult {
    pub matches: Vec<Pubkey>,
    pub distinct: usize,
    pub unmatched: usize,
}
