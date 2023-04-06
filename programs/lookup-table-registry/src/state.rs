use anchor_lang::prelude::*;

// TODO: we can leave this as unlimited
pub const REGISTRY_ENTRY_SIZE: usize = std::mem::size_of::<RegistryEntry>();
/// The maximum number of registry entries.
///
/// Note that this size can be increased, however a practical limit of u8::MAX
/// has been selected on a reasonable assumption that 255 entries are sufficient.
/// Each lookup table can store up to 256 accounts, thus a registry can have 65k records.
pub const MAX_REGISTRY_ENTRIES: usize =
    (10240 - std::mem::size_of::<RegistryAccount>()) / REGISTRY_ENTRY_SIZE;

/// Current format allows up to 254 lookup accounts
const _: () = assert!(MAX_REGISTRY_ENTRIES == 254);
const _: () = assert!(MAX_REGISTRY_ENTRIES < u8::MAX as usize);

/// A registry account that stores the lookup tables that an authority has created.
#[account]
#[repr(C)]
#[derive(Debug)]
pub struct RegistryAccount {
    /// The authority that owns and signs for changes to the registry account
    pub authority: Pubkey,
    /// The version of the registry account. The version denotes some change in
    /// functionality.
    /// - 0: initial version with no discriminators
    pub version: u8,
    /// The seed returned when deriving the registry account's address
    pub seed: [u8; 1],
    /// The length of the populated entries in the registry
    pub len: u8,
    /// The allocated capacity of lookup entries. The capacity can be > len
    pub capacity: u8,
    /// Reserved bytes used as padding
    pub reserved0: [u8; 4],
    /// The slot when the last lookup table was created.
    /// Used to prevent a user creating multiple addresses in same slot
    pub last_created_slot: u64,
    /// A growable list of registry entries
    pub tables: Vec<RegistryEntry>,
}

/// An entry that tracks a lookup table and its state.
#[derive(AnchorSerialize, AnchorDeserialize, Debug, Clone)]
pub struct RegistryEntry {
    /// An identifier to track the state (and in future purpose) of an entry
    pub discriminator: u64,
    /// The lookup table address
    pub table: Pubkey,
}

impl RegistryAccount {
    /// Find an entry in the registry by its address
    pub fn find_entry(&self, address: &Pubkey) -> Result<&RegistryEntry> {
        self.tables
            .iter()
            .find(|entry| &entry.table == address)
            .ok_or(crate::ErrorCode::InvalidLookupTable.into())
    }

    /// Find an entry in the registry by its address for mutation
    pub fn find_entry_mut(&mut self, address: &Pubkey) -> Result<&mut RegistryEntry> {
        self.tables
            .iter_mut()
            .find(|entry| &entry.table == address)
            .ok_or(crate::ErrorCode::InvalidLookupTable.into())
    }

    /// Find an empty entry in the registry. An empty entry is one with a discriminator = [crate::discriminator::EMPTY]
    pub fn find_empty_entry(&mut self) -> Result<&mut RegistryEntry> {
        self.tables
            .iter_mut()
            .find(|entry| entry.discriminator == crate::discriminator::EMPTY)
            .ok_or(crate::ErrorCode::InvalidLookupTable.into())
    }
}
