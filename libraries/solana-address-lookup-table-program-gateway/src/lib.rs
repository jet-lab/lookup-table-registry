/// This allows the stubs to be available internally:
/// - for tests
/// - even if "full" is active
///
/// "full" only deactivates the pub re-exports
mod stub {
    pub mod id {
        #[cfg(any(test, feature = "stub-id"))]
        solana_program::declare_id!("AddressLookupTab1e1111111111111111111111111");
    }

    #[cfg(any(test, feature = "stub-instruction"))]
    pub mod instruction;

    #[cfg(any(test, feature = "stub-state"))]
    pub mod state;
}

#[cfg(feature = "full")]
pub use solana_address_lookup_table_program::*;

#[cfg(all(not(feature = "full"), feature = "stub-id"))]
pub use stub::id::*;

#[cfg(all(not(feature = "full"), feature = "stub-state"))]
pub use stub::state;

pub mod instruction {
    #[cfg(feature = "full")]
    pub use solana_address_lookup_table_program::instruction::*;

    #[cfg(all(not(feature = "full"), feature = "stub-instruction"))]
    pub use super::stub::instruction::*;

    // TODO: remove this on upgrade to solana_address_lookup_table_program 1.15
    #[cfg(feature = "stub-instruction")]
    pub use super::stub::instruction::create_lookup_table_signed;
}

#[test]
fn stub_id_is_correct() {
    assert_eq!(stub::id::ID, solana_address_lookup_table_program::ID);
}

#[cfg(test)]
pub(crate) mod test_data {
    use solana_program::pubkey::Pubkey;
    use std::str::FromStr;

    pub(crate) const SLOTS: [solana_program::slot_history::Slot; 12] = [
        0,
        1203,
        5984359,
        4923824084,
        92398304917,
        842139847,
        487239487,
        823479832,
        283497234,
        188227418,
        908423209,
        940385092,
    ];

    pub(crate) fn addresses() -> [Pubkey; 12] {
        [
            Pubkey::from_str("5bmWuR1dgP4avtGYMNKLuxumZTVKGgoN2BCMXWDNL9nY").unwrap(),
            Pubkey::from_str("4YfiMm6wp4M6rpyuCkF74aQmNrL2GtF72he2fCiS4Lo2").unwrap(),
            Pubkey::from_str("7qbRF6YsyGuLUVs6Y1q64bdVrfe4ZcUUz1JRdoVNUJnm").unwrap(),
            Pubkey::from_str("ACSZsvP1iCKWQVXoUMenHWpAfD3KgubLoMz8GtKP1NWQ").unwrap(),
            Pubkey::from_str("FbMCQ2PTDmvnMSsXKov91qEZzUMbfP7vnV6CcQf2ebr1").unwrap(),
            Pubkey::from_str("FS3VUPpibuMkTEdGx17JxTNHj8YZ2PxmchFvPWHVjLS9").unwrap(),
            Pubkey::from_str("Dcd63bUGN8pycgDCEU5nBB3j6vohPXmxFNufvZG8sU36").unwrap(),
            Pubkey::from_str("7XLWyPdHWK8Fs6s1yzWnheFS61e2C6CUP7oTYH5VW34n").unwrap(),
            Pubkey::from_str("HZfG2mtxiuL3dQeHw6VqRFHWR33nzHhJ5seWVabRfC6J").unwrap(),
            Pubkey::from_str("323MrRVVZgH877H7FiEcRFMqpZR7Uzzc6x6c4R2uE9B8").unwrap(),
            Pubkey::from_str("9RfZwn2Prux6QesG1Noo4HzMEBv3rPndJ2bN2Wwd6a7p").unwrap(),
            Pubkey::from_str("BVNo8ftg2LkkssnWT4ZWdtoFaevnfD6ExYeramwM27pe").unwrap(),
        ]
    }
}
