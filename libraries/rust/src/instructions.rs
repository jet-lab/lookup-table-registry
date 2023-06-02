//! Build instructions to interact with the lookup registry.
//! The instruction builder is useful if wanting to combine instructions,
//! otherwise use [crate::registy::LookupRegistry].

use anchor_lang::{InstructionData, ToAccountMetas};
use lookup_table_registry::{
    accounts as ix_accounts, instruction as ix_data, ID as LOOKUP_REGISTRY_ID,
};
use solana_address_lookup_table_program_gateway::ID as LOOKUP_ID;
use solana_sdk::{
    instruction::Instruction, pubkey::Pubkey, system_program::ID as SYSTEM_PROGAM_ID,
};

/// An instruction builder of the lookup table registry program.
pub struct InstructionBuilder {
    /// The authority that owns the lookup table
    pub authority: Pubkey,
    /// The payer of transaction costs and rent
    pub payer: Pubkey,
}

impl InstructionBuilder {
    /// Creates a new instruction builder
    pub fn new(authority: Pubkey, payer: Pubkey) -> Self {
        Self { authority, payer }
    }

    /// Creates an instruction to initialize a lookup table registry.
    pub fn init_registry(&self) -> Instruction {
        let accounts = ix_accounts::InitRegistryAccount {
            authority: self.authority,
            payer: self.payer,
            registry_account: self.registry_address(),
            system_program: SYSTEM_PROGAM_ID,
        }
        .to_account_metas(None);

        Instruction {
            program_id: LOOKUP_REGISTRY_ID,
            accounts,
            data: ix_data::InitRegistryAccount {}.data(),
        }
    }

    /// Instruction to create a lookup table.
    ///
    /// Returns the address of the lookup table with the instruction to create it.
    pub fn create_lookup_table(
        &self,
        recent_slot: u64,
        // Not required, kept for future compat purposes
        _discriminator: u64,
    ) -> (Instruction, Pubkey) {
        // Get slot
        let lookup_table =
            solana_address_lookup_table_program_gateway::instruction::derive_lookup_table_address(
                &self.authority,
                recent_slot,
            )
            .0;
        let accounts = ix_accounts::CreateLookupTable {
            authority: self.authority,
            payer: self.payer,
            registry_account: self.registry_address(),
            lookup_table,
            address_lookup_table_program: LOOKUP_ID,
            system_program: SYSTEM_PROGAM_ID,
        }
        .to_account_metas(None);

        (
            Instruction {
                program_id: LOOKUP_REGISTRY_ID,
                accounts,
                data: ix_data::CreateLookupTable {
                    recent_slot,
                    _discriminator: 0,
                }
                .data(),
            },
            lookup_table,
        )
    }

    /// Creates an instruction to remove a lookup table.
    pub fn remove_lookup_table(&self, lookup_table: Pubkey) -> Instruction {
        let accounts = ix_accounts::RemoveLookupTable {
            authority: self.authority,
            recipient: self.payer,
            registry_account: self.registry_address(),
            lookup_table,
            address_lookup_table_program: LOOKUP_ID,
            system_program: SYSTEM_PROGAM_ID,
        }
        .to_account_metas(None);

        Instruction {
            program_id: LOOKUP_REGISTRY_ID,
            accounts,
            data: ix_data::RemoveLookupTable.data(),
        }
    }

    /// Creates an instruction to append addresses to a lookup table.
    /// First inspects the lookup table to remove any duplicate addresses,
    /// then appends the unique new addresses.
    ///
    /// An error is returned if the addresses would exceed the lookup table's limit.
    pub fn append_to_lookup_table(
        &self,
        lookup_table: Pubkey,
        addresses: &[Pubkey],
        // Not required, kept for future compat purposes
        _discriminator: u64,
    ) -> Instruction {
        let accounts = ix_accounts::AppendToLookupTable {
            authority: self.authority,
            payer: self.payer,
            registry_account: self.registry_address(),
            lookup_table,
            address_lookup_table_program: LOOKUP_ID,
            system_program: SYSTEM_PROGAM_ID,
        }
        .to_account_metas(None);

        Instruction {
            program_id: LOOKUP_REGISTRY_ID,
            accounts,
            data: ix_data::AppendToLookupTable {
                _discriminator: 0,
                addresses: addresses.to_vec(),
            }
            .data(),
        }
    }

    /// Derive the address of the registry account using the authority.
    pub fn registry_address(&self) -> Pubkey {
        Pubkey::find_program_address(&[self.authority.as_ref()], &LOOKUP_REGISTRY_ID).0
    }
}
