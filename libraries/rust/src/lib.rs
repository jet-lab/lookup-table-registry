use anchor_lang::prelude::Pubkey;
use solana_sdk::address_lookup_table_account::AddressLookupTableAccount;

pub mod instructions;
#[cfg(feature = "client")]
pub mod reader;
#[cfg(feature = "client")]
pub mod writer;

#[cfg(feature = "client")]
pub mod common;

pub use lookup_table_registry::ID as LOOKUP_TABLE_REGISTRY_ID;
pub use solana_address_lookup_table_program_gateway::ID as LOOKUP_TABLE_ID;

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

impl From<Entry> for AddressLookupTableAccount {
    fn from(value: Entry) -> Self {
        AddressLookupTableAccount {
            key: value.lookup_address,
            addresses: value.addresses,
        }
    }
}

pub fn derive_lookup_table_address(authority: &Pubkey, recent_block_slot: u64) -> Pubkey {
    solana_address_lookup_table_program_gateway::instruction::derive_lookup_table_address(
        authority,
        recent_block_slot,
    )
    .0
}
