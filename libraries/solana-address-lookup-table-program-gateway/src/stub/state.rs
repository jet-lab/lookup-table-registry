#![allow(dead_code)]

use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use solana_program::{instruction::InstructionError, pubkey::Pubkey, slot_history::Slot};

const LOOKUP_TABLE_META_SIZE: usize = 56;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[allow(clippy::large_enum_variant)]
enum ProgramState {
    Uninitialized,
    LookupTable(LookupTableMeta),
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct AddressLookupTable<'a> {
    pub meta: LookupTableMeta,
    pub addresses: Cow<'a, [Pubkey]>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct LookupTableMeta {
    pub deactivation_slot: Slot,
    pub last_extended_slot: Slot,
    pub last_extended_slot_start_index: u8,
    pub authority: Option<Pubkey>,
    pub _padding: u16,
}

impl<'a> AddressLookupTable<'a> {
    /// Efficiently deserialize an address table without allocating
    /// for stored addresses.
    pub fn deserialize(data: &'a [u8]) -> Result<AddressLookupTable<'a>, InstructionError> {
        let program_state: ProgramState =
            bincode::deserialize(data).map_err(|_| InstructionError::InvalidAccountData)?;

        let meta = match program_state {
            ProgramState::LookupTable(meta) => Ok(meta),
            ProgramState::Uninitialized => Err(InstructionError::UninitializedAccount),
        }?;

        let raw_addresses_data = data.get(LOOKUP_TABLE_META_SIZE..).ok_or({
            // Should be impossible because table accounts must
            // always be LOOKUP_TABLE_META_SIZE in length
            InstructionError::InvalidAccountData
        })?;
        let addresses: &[Pubkey] = bytemuck::try_cast_slice(raw_addresses_data).map_err(|_| {
            // Should be impossible because raw address data
            // should be aligned and sized in multiples of 32 bytes
            InstructionError::InvalidAccountData
        })?;

        Ok(Self {
            meta,
            addresses: Cow::Borrowed(addresses),
        })
    }
}

#[cfg(test)]
mod test {
    use std::borrow::Cow;

    use solana_address_lookup_table_program::state as real;

    use crate::test_data::{addresses, SLOTS};

    #[test]
    fn real_serialized_table_deserializes_as_expected() {
        let addr = addresses();
        for i in 0..6 {
            let n = i * 2;
            let real_table = real::AddressLookupTable {
                meta: real::LookupTableMeta {
                    deactivation_slot: SLOTS[0 + n],
                    last_extended_slot: SLOTS[1 + n],
                    last_extended_slot_start_index: 123,
                    authority: if i % 2 == 0 { None } else { Some(addr[i]) },
                    _padding: 12345,
                },
                addresses: Cow::from(&addr[..]),
            };
            let fake_table = super::AddressLookupTable {
                meta: super::LookupTableMeta {
                    deactivation_slot: SLOTS[0 + n],
                    last_extended_slot: SLOTS[1 + n],
                    last_extended_slot_start_index: 123,
                    authority: if i % 2 == 0 { None } else { Some(addr[i]) },
                    _padding: 12345,
                },
                addresses: Cow::from(&addr[..]),
            };
            let serialized = real_table.serialize_for_tests().unwrap();
            let deserialized = super::AddressLookupTable::deserialize(&serialized).unwrap();
            assert_eq!(deserialized, fake_table);
        }
    }
}
