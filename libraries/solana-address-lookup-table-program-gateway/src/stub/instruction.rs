#![allow(unused, clippy::enum_variant_names)]

use serde::Serialize;
use solana_program::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    slot_history::Slot,
    system_program,
};

use crate::id;

/// Derives the address of an address table account from a wallet address and a recent block's slot.
pub fn derive_lookup_table_address(
    authority_address: &Pubkey,
    recent_block_slot: Slot,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[authority_address.as_ref(), &recent_block_slot.to_le_bytes()],
        &id(),
    )
}

/// Constructs an instruction to create a table account and returns
/// the instruction and the table account's derived address.
///
/// # Note
///
/// This instruction requires the authority to be a signer but
/// in v1.12 the address lookup table program will no longer require
/// the authority to sign the transaction.
#[allow(dead_code)] // TODO: remove this and add a unit test on upgrade to solana 1.15
pub fn create_lookup_table_signed(
    authority_address: Pubkey,
    payer_address: Pubkey,
    recent_slot: Slot,
) -> (Instruction, Pubkey) {
    create_lookup_table_common(authority_address, payer_address, recent_slot, true)
}

/// Constructs an instruction to create a table account and returns
/// the instruction and the table account's derived address.
///
/// # Note
///
/// This instruction doesn't require the authority to be a signer but
/// until v1.12 the address lookup table program still requires the
/// authority to sign the transaction.
pub fn create_lookup_table(
    authority_address: Pubkey,
    payer_address: Pubkey,
    recent_slot: Slot,
) -> (Instruction, Pubkey) {
    create_lookup_table_common(authority_address, payer_address, recent_slot, false)
}

/// Constructs an instruction to create a table account and returns
/// the instruction and the table account's derived address.
fn create_lookup_table_common(
    authority_address: Pubkey,
    payer_address: Pubkey,
    recent_slot: Slot,
    authority_is_signer: bool,
) -> (Instruction, Pubkey) {
    let (lookup_table_address, bump_seed) =
        derive_lookup_table_address(&authority_address, recent_slot);
    let instruction = Instruction::new_with_bincode(
        id(),
        &ProgramInstruction::CreateLookupTable {
            recent_slot,
            bump_seed,
        },
        vec![
            AccountMeta::new(lookup_table_address, false),
            AccountMeta::new_readonly(authority_address, authority_is_signer),
            AccountMeta::new(payer_address, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
    );

    (instruction, lookup_table_address)
}

/// Constructs an instruction which extends an address lookup
/// table account with new addresses.
pub fn extend_lookup_table(
    lookup_table_address: Pubkey,
    authority_address: Pubkey,
    payer_address: Option<Pubkey>,
    new_addresses: Vec<Pubkey>,
) -> Instruction {
    let mut accounts = vec![
        AccountMeta::new(lookup_table_address, false),
        AccountMeta::new_readonly(authority_address, true),
    ];

    if let Some(payer_address) = payer_address {
        accounts.extend([
            AccountMeta::new(payer_address, true),
            AccountMeta::new_readonly(system_program::id(), false),
        ]);
    }

    Instruction::new_with_bincode(
        id(),
        &ProgramInstruction::ExtendLookupTable { new_addresses },
        accounts,
    )
}

#[derive(Serialize)]
enum ProgramInstruction {
    CreateLookupTable {
        recent_slot: Slot,
        bump_seed: u8,
    },
    #[allow(dead_code)]
    FreezeLookupTable,
    ExtendLookupTable {
        new_addresses: Vec<Pubkey>,
    },
}

#[cfg(test)]
mod test {
    use solana_address_lookup_table_program::instruction as real;

    use crate::test_data::{addresses, SLOTS};

    #[test]
    fn derive_lookup_table_address() {
        let addr = addresses();
        for i in 0..12 {
            assert_eq!(
                real::derive_lookup_table_address(&addr[i], SLOTS[i]),
                super::derive_lookup_table_address(&addr[i], SLOTS[i])
            );
        }
    }

    #[test]
    fn extend_lookup_table() {
        let addr = addresses();
        for i in 0..2 {
            let n = i * 4;
            assert_eq!(
                real::extend_lookup_table(
                    addr[0 + n],
                    addr[1 + n],
                    if i == 0 { None } else { Some(addr[2 + n]) },
                    vec![addr[3 + n], addr[4 + n]][0..(i + 1)].to_vec(),
                ),
                super::extend_lookup_table(
                    addr[0 + n],
                    addr[1 + n],
                    if i == 0 { None } else { Some(addr[2 + n]) },
                    vec![addr[3 + n], addr[4 + n]][0..(i + 1)].to_vec(),
                ),
            );
        }
    }

    #[test]
    fn create_lookup_table() {
        let addr = addresses();
        for i in 0..6 {
            let n = i * 2;
            assert_eq!(
                real::create_lookup_table(addr[0 + n], addr[0 + n], SLOTS[i],),
                super::create_lookup_table(addr[0 + n], addr[0 + n], SLOTS[i],),
            );
        }
    }
}
