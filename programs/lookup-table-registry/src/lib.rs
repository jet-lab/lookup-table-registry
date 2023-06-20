//! A Solana program to manage address lookup tables.
//!
//! The program solves the problem of tracking lookup tables created by a user.
//! The Solana address lookup program currently has no efficient mechanism of
//! returning a user's addresses without searching accounts owned by the program.
//! This forces a user to record either seeds used in creating or the lookup table
//! address should they want to update the table at a later stage.
//!
//! This program creates a registry that stores the addresses created and thus
//! can be queried more efficiently.
//!
//! Note: Nothing prevents a registry from having duplicate entries as the
//! address lookup program does not enforce uniqueness.
//! Thus callers (libraries) should enforce this to prevent wasting lamports
//! storing addresses that have duplicates.
//!
//! Possible use-cases:
//! - A wallet or margin account can store the ATAs owned by it, or those of common tokens.
//! - A program can store the addresses used in a market/pool.

#![allow(clippy::result_large_err, clippy::assertions_on_constants)]

use anchor_lang::prelude::*;
use solana_address_lookup_table_program_gateway as solana_address_lookup_table_program;

declare_id!("LTR8xXcSrEDsCbTWPY4JmJREFdMz4uYh65uajkVjzru");

mod state;

pub use state::*;

/// Special constants for the discriminator
pub mod discriminator {
    /// No table is stored
    pub const EMPTY: u64 = 0b0;
    /// The lookup table has been deactivated, and can be closed in a future slot
    pub const DEACTIVATED: u64 = 0b1;

    const _: () = assert!(EMPTY < DEACTIVATED);
}

/// Lookup table registry program stub
#[cfg_attr(not(feature = "program"), program)]
#[cfg(not(feature = "program"))]
#[allow(unused)]
pub mod lookup_table_registry {
    use super::*;

    /// Initialize a registry account owned by the authority.
    ///
    /// Errors if a registry account already exists.
    pub fn init_registry_account(ctx: Context<InitRegistryAccount>) -> Result<()> {
        unimplemented!()
    }

    /// Create a lookup table in the registry
    pub fn create_lookup_table(
        ctx: Context<CreateLookupTable>,
        recent_slot: u64,
        _discriminator: u64,
    ) -> Result<()> {
        unimplemented!()
    }

    /// Add addresses to a lookup table.
    pub fn append_to_lookup_table(
        ctx: Context<AppendToLookupTable>,
        addresses: Vec<Pubkey>,
        _discriminator: u64,
    ) -> Result<()> {
        unimplemented!()
    }

    /// Remove a lookup table by either deactivating or deleting it depending on its
    /// current status.
    pub fn remove_lookup_table(ctx: Context<RemoveLookupTable>) -> Result<()> {
        unimplemented!()
    }
}

/// Lookup table registry program
#[cfg_attr(feature = "program", program)]
#[cfg(feature = "program")]
pub mod lookup_table_registry {
    use solana_program::program::invoke;

    use super::*;

    /// Initialize a registry account owned by the authority.
    ///
    /// Errors if a registry account already exists.
    pub fn init_registry_account(ctx: Context<InitRegistryAccount>) -> Result<()> {
        let clock = Clock::get()?;
        let registry = &mut ctx.accounts.registry_account;
        registry.authority = ctx.accounts.authority.key();
        registry.version = 0;
        registry.len = 0;
        registry.capacity = 0;
        registry.last_created_slot = clock.slot;
        registry.seed = [*ctx.bumps.get("registry_account").unwrap()];
        registry.tables = vec![];

        Ok(())
    }

    /// Create a lookup table in the registry
    pub fn create_lookup_table(
        ctx: Context<CreateLookupTable>,
        recent_slot: u64,
        _discriminator: u64,
    ) -> Result<()> {
        if ctx.accounts.registry_account.len as usize == MAX_REGISTRY_ENTRIES {
            return err!(ErrorCode::TooManyEntries);
        }
        let discriminator = discriminator::DEACTIVATED + 1;
        // Discriminator can't be 0
        if discriminator <= discriminator::DEACTIVATED {
            return err!(ErrorCode::InvalidDiscriminator);
        }
        ctx.accounts.registry_account.last_created_slot = recent_slot;
        // Allocate space on the registry account if there are no more slots
        let (len, capacity) = {
            let registry = &ctx.accounts.registry_account;
            (registry.len, registry.capacity)
        };
        let registry_info = ctx.accounts.registry_account.to_account_info();
        let append_to_end = len == capacity;
        if append_to_end {
            // Needs realloc
            let new_size = registry_info.data_len() + REGISTRY_ENTRY_SIZE;
            let rent = Rent::get()?;
            let transfer_amount = rent
                .minimum_balance(new_size)
                .checked_sub(registry_info.lamports())
                .unwrap();
            anchor_lang::system_program::transfer(
                CpiContext::new(
                    ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.payer.to_account_info(),
                        to: registry_info,
                    },
                ),
                transfer_amount,
            )?;
            // Increment the length of the registry
            ctx.accounts.registry_account.len += 1;
        }

        // Create the lookup table
        let (lookup_instruction, table) =
            solana_address_lookup_table_program::instruction::create_lookup_table_signed(
                ctx.accounts.authority.key(),
                ctx.accounts.payer.key(),
                recent_slot,
            );
        if table != ctx.accounts.lookup_table.key() {
            return err!(ErrorCode::InvalidLookupTable);
        }

        invoke(
            &lookup_instruction,
            &[
                ctx.accounts.lookup_table.to_account_info(),
                ctx.accounts.authority.to_account_info(),
                ctx.accounts.payer.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.address_lookup_table_program.to_account_info(),
            ],
        )?;

        // Add the account to the lookup registry
        let entry = RegistryEntry {
            discriminator,
            table,
        };
        if append_to_end {
            // Happy case, add to the end
            let registry_info = ctx.accounts.registry_account.to_account_info();
            let existing_len = registry_info.data_len();
            registry_info.realloc(existing_len + REGISTRY_ENTRY_SIZE, true)?;
            ctx.accounts.registry_account.tables.push(entry);
        } else {
            // Find a slot that's empty
            let slot = ctx.accounts.registry_account.find_empty_entry()?;
            *slot = entry;
        }
        ctx.accounts.registry_account.capacity += 1;
        // Redundant check
        if ctx.accounts.registry_account.len > ctx.accounts.registry_account.capacity {
            return err!(ErrorCode::InvalidState);
        }

        Ok(())
    }

    /// Add addresses to a lookup table.
    pub fn append_to_lookup_table(
        ctx: Context<AppendToLookupTable>,
        addresses: Vec<Pubkey>,
        _discriminator: u64,
    ) -> Result<()> {
        // Find the table in the registry
        {
            let entry = ctx
                .accounts
                .registry_account
                .find_entry(ctx.accounts.lookup_table.key)?;

            if entry.discriminator <= crate::discriminator::DEACTIVATED {
                msg!("Cannot append to a lookup table that is deactivated");
                return err!(ErrorCode::InvalidDiscriminator);
            }
            // The discriminators should be compared in future versions
        }

        let instruction = solana_address_lookup_table_program::instruction::extend_lookup_table(
            ctx.accounts.lookup_table.key(),
            ctx.accounts.authority.key(),
            Some(ctx.accounts.payer.key()),
            addresses,
        );

        invoke(
            &instruction,
            &[
                ctx.accounts.lookup_table.to_account_info(),
                ctx.accounts.authority.to_account_info(),
                ctx.accounts.payer.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.address_lookup_table_program.to_account_info(),
            ],
        )?;

        Ok(())
    }

    /// Remove a lookup table by either deactivating or deleting it depending on its
    /// current status.
    pub fn remove_lookup_table(ctx: Context<RemoveLookupTable>) -> Result<()> {
        // Find the table in the registry
        let entry = ctx
            .accounts
            .registry_account
            .find_entry_mut(ctx.accounts.lookup_table.key)?;
        // If the entry is active, deactivate it
        let to_delete = match entry.discriminator {
            discriminator::EMPTY => {
                msg!("Found an entry with an EMPTY discriminator, invalid state");
                return err!(ErrorCode::InvalidState);
            }
            discriminator::DEACTIVATED => {
                // mark as closed
                entry.discriminator = discriminator::EMPTY;
                entry.table = Pubkey::default();
                // Decrement the registry length
                ctx.accounts.registry_account.len =
                    ctx.accounts.registry_account.len.checked_sub(1).unwrap();
                true
            }
            _ => {
                // mark as deactivated
                entry.discriminator = discriminator::DEACTIVATED;
                false
            }
        };

        if to_delete {
            // Close the lookup table
            let lookup_instruction =
                solana_address_lookup_table_program::instruction::close_lookup_table(
                    ctx.accounts.lookup_table.key(),
                    ctx.accounts.authority.key(),
                    ctx.accounts.recipient.key(),
                );

            invoke(
                &lookup_instruction,
                &[
                    ctx.accounts.lookup_table.to_account_info(),
                    ctx.accounts.authority.to_account_info(),
                    ctx.accounts.recipient.to_account_info(),
                    ctx.accounts.system_program.to_account_info(),
                    ctx.accounts.address_lookup_table_program.to_account_info(),
                ],
            )?;
        } else {
            // Deactivate the lookup table
            let lookup_instruction =
                solana_address_lookup_table_program::instruction::deactivate_lookup_table(
                    ctx.accounts.lookup_table.key(),
                    ctx.accounts.authority.key(),
                );

            invoke(
                &lookup_instruction,
                &[
                    ctx.accounts.lookup_table.to_account_info(),
                    ctx.accounts.authority.to_account_info(),
                    ctx.accounts.address_lookup_table_program.to_account_info(),
                ],
            )?;
        }

        Ok(())
    }
}

/// Accounts for the instruction to initialize a lookup table registry account
#[derive(Accounts)]
pub struct InitRegistryAccount<'info> {
    /// The authority of the registry account
    pub authority: Signer<'info>,

    /// The payer of the transaction
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The registry account of the authority
    #[account(init,
        seeds = [authority.key.as_ref()],
        bump,
        payer = payer,
        space = 8 + std::mem::size_of::<RegistryAccount>())
    ]
    pub registry_account: Box<Account<'info, RegistryAccount>>,

    /// The system program
    pub system_program: Program<'info, System>,
}

/// Accounts for the instruction to create a lookup table in the registry
#[derive(Accounts)]
pub struct CreateLookupTable<'info> {
    /// The authority of the registry account
    pub authority: Signer<'info>,

    /// The payer of the transaction
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The registry account of the authority
    #[account(mut, constraint = registry_account.authority == authority.key())]
    pub registry_account: Box<Account<'info, RegistryAccount>>,

    /// The lookup table being created
    /// CHECK: the account will be validated by the lookup table program
    #[account(mut)]
    pub lookup_table: AccountInfo<'info>,

    /// CHECK: the account will be validated by the lookup table program
    #[account(address = solana_address_lookup_table_program::ID)]
    pub address_lookup_table_program: AccountInfo<'info>,

    /// The system program
    pub system_program: Program<'info, System>,
}

/// Accounts for the instruction to append entries to a lookup table
#[derive(Accounts)]
pub struct AppendToLookupTable<'info> {
    /// The authority of the registry account
    pub authority: Signer<'info>,

    /// The payer of the transaction
    #[account(mut)]
    pub payer: Signer<'info>,

    /// The registry account of the authority
    #[account(constraint = registry_account.authority == authority.key())]
    pub registry_account: Box<Account<'info, RegistryAccount>>,

    /// The lookup table being created
    /// CHECK: the account will be validated by the lookup table program
    #[account(mut)]
    pub lookup_table: AccountInfo<'info>,

    /// CHECK: the account will be validated by the lookup table program
    #[account(address = solana_address_lookup_table_program::ID)]
    pub address_lookup_table_program: AccountInfo<'info>,

    /// The system program
    pub system_program: Program<'info, System>,
}

/// Accounts for the instruction to remove a lookup table
#[derive(Accounts)]
pub struct RemoveLookupTable<'info> {
    /// The authority of the registry account
    pub authority: Signer<'info>,

    /// The recipient of lamports
    #[account(mut)]
    pub recipient: Signer<'info>,

    /// The registry account of the authority
    #[account(mut, constraint = registry_account.authority == authority.key())]
    pub registry_account: Box<Account<'info, RegistryAccount>>,

    /// The lookup table being closed
    /// CHECK: the account will be validated by the lookup table program
    #[account(mut)]
    pub lookup_table: AccountInfo<'info>,

    /// CHECK: the account will be validated by the lookup table program
    #[account(address = solana_address_lookup_table_program::ID)]
    pub address_lookup_table_program: AccountInfo<'info>,

    /// The system program
    pub system_program: Program<'info, System>,
}

/// Errors used in the program
#[error_code]
pub enum ErrorCode {
    /// A discriminator used is invalid
    #[msg("Invalid discriminator used")]
    InvalidDiscriminator = 10000,

    /// The slot provided cannot be earlier than the last slot used
    #[msg("Slot cannot be earlier than the last slot used")]
    InvalidSlot,

    /// The lookup table provided is invalid
    #[msg("Invalid lookup table")]
    InvalidLookupTable,

    /// Too many entries have been stored in the registry account
    #[msg("There are too many entries in the registry account")]
    TooManyEntries,

    /// Thep rogram encountered some invalid state
    #[msg("The lookup registry is in an invalid state")]
    InvalidState,
}
