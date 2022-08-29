use crate::error::ProgramError;
use anchor_lang::{
    prelude::*,
    solana_program::{program::invoke_signed, program_memory::sol_memcmp, pubkey::PUBKEY_BYTES},
};
use anchor_spl::{associated_token::create, associated_token::Create, token::Mint};
use mpl_token_metadata::state::{Metadata, TokenMetadataAccount};
use std::{convert::TryInto, slice::Iter};

// Function taken from auction house contract
pub fn assert_metadata_valid<'a>(metadata: &UncheckedAccount, mint: &Pubkey) -> Result<()> {
    assert_derivation(
        &mpl_token_metadata::id(),
        &metadata.to_account_info(),
        &[
            mpl_token_metadata::state::PREFIX.as_bytes(),
            mpl_token_metadata::id().as_ref(),
            mint.as_ref(),
        ],
    )?;
    if metadata.data_is_empty() {
        return Err(ProgramError::MetadataDoesntExist.into());
    }

    Ok(())
}

// Function taken from auction house contract
pub fn assert_derivation(program_id: &Pubkey, account: &AccountInfo, path: &[&[u8]]) -> Result<u8> {
    let (key, bump) = Pubkey::find_program_address(&path, program_id);
    if key != *account.key {
        return Err(ProgramError::DerivedKeyInvalid.into());
    }
    Ok(bump)
}

// Function taken from auction house contract
pub fn assert_keys_equal(key1: Pubkey, key2: Pubkey) -> Result<()> {
    if sol_memcmp(key1.as_ref(), key2.as_ref(), PUBKEY_BYTES) != 0 {
        err!(ProgramError::PublicKeyMismatch)
    } else {
        Ok(())
    }
}

// Custom function to validate nft belongs to a specified collection and is verified
pub fn validate_nft(
    nft_token_mint: Account<Mint>,
    nft_token_metadata: UncheckedAccount,
    collection_mint: Account<Mint>,
    collection_metadata: UncheckedAccount,
) -> Result<()> {
    assert_metadata_valid(&nft_token_metadata, &nft_token_mint.key())?;

    let token_metadata: Metadata =
        Metadata::from_account_info(&nft_token_metadata.to_account_info())?;

    let token_collection = token_metadata.collection;
    let token_collection_details = token_metadata.collection_details;

    if token_collection_details.is_some() {
        return Err(ProgramError::InvalidCollectionDetails.into());
    }

    if token_collection.is_none() {
        return Err(ProgramError::InvalidCollection.into());
    }

    if token_collection.clone().unwrap().key != collection_mint.key() {
        return Err(ProgramError::InvalidCollectionMint.into());
    }

    if token_collection.unwrap().verified != true {
        return Err(ProgramError::NftNotVerified.into());
    }

    assert_metadata_valid(&collection_metadata, &collection_mint.key())?;

    let collection_metadata: Metadata =
        Metadata::from_account_info(&collection_metadata.to_account_info())?;

    let collection_collection = collection_metadata.collection;
    let collection_collection_details = collection_metadata.collection_details;

    if collection_collection.is_some() {
        return Err(ProgramError::InvalidCollection.into());
    }

    if collection_collection_details.is_none() {
        return Err(ProgramError::InvalidCollectionDetails.into());
    }

    Ok(())
}

// Helper function to calculate the total royalty fees for a given nft
pub fn calculate_royalty_fee<'info>(
    metadata_account_info: &AccountInfo<'info>,
    size: u64,
) -> Result<u64> {
    let metadata: Metadata = Metadata::from_account_info(metadata_account_info)?;
    let fees = metadata.data.seller_fee_basis_points;
    let total_fee = (fees as u128)
        .checked_mul(size as u128)
        .ok_or(ProgramError::NumericalOverflow)?
        .checked_div(10000)
        .ok_or(ProgramError::NumericalOverflow)? as u64;

    // TODO: Check for remaining dust and subtract from total fee

    Ok(total_fee)
}

// Helper function to honor NFT royalties for a given NFT
pub fn honor_royalties<'info>(
    is_pair_paying: bool, // if true, CpiContext::new_with_signer is used, otherwise CpiContext::new is used
    remaining_accounts: &mut Iter<AccountInfo<'info>>,
    metadata_account_info: &AccountInfo<'info>,
    size: u64,
    is_native: bool, // If using wrapped SOL, unwrap first and then send to royalty recipient
    associated_token_program: &AccountInfo<'info>,
    payer: &AccountInfo<'info>,
    mint: &AccountInfo<'info>,
    system_program: &AccountInfo<'info>,
    token_program: &AccountInfo<'info>,
    rent_sysvar: &AccountInfo<'info>,
) -> Result<()> {
    let metadata: Metadata = Metadata::from_account_info(metadata_account_info)?;
    let fees = metadata.data.seller_fee_basis_points;
    let total_fee = (fees as u128)
        .checked_mul(size as u128)
        .ok_or(ProgramError::NumericalOverflow)?
        .checked_div(10000)
        .ok_or(ProgramError::NumericalOverflow)? as u64;
    let mut remaining_fee = total_fee;
    let remaining_size = size
        .checked_sub(total_fee)
        .ok_or(ProgramError::NumericalOverflow)?;

    match metadata.data.creators {
        Some(creators) => {
            for creator in creators {
                let pct = creator.share as u128;
                let creator_fee =
                    pct.checked_mul(total_fee as u128)
                        .ok_or(ProgramError::NumericalOverflow)?
                        .checked_div(100)
                        .ok_or(ProgramError::NumericalOverflow)? as u64;
                remaining_fee = remaining_fee
                    .checked_sub(creator_fee)
                    .ok_or(ProgramError::NumericalOverflow)?;
                let current_creator_info = next_account_info(remaining_accounts)?;
                assert_keys_equal(creator.address, *current_creator_info.key)?;

                if !is_native {
                    let current_creator_token_account_info = next_account_info(remaining_accounts)?;
                    if current_creator_token_account_info.data_is_empty() {
                        let create_ata_accounts = Create {
                            payer: payer.clone(),
                            associated_token: associated_token_program.clone(),
                            authority: payer.clone(),
                            mint: mint.clone(),
                            system_program: system_program.clone(),
                            rent: rent_sysvar.clone(),
                            token_program: token_program.clone(),
                        };
                        let create_ctx =
                            CpiContext::new(associated_token_program.clone(), create_ata_accounts);

                        create(create_ctx)?;
                    }

                    if creator_fee > 0 {
                        if is_pair_paying {
                        } else {
                        }
                    }
                } else if creator_fee > 0 {
                    if is_pair_paying {
                    } else {
                    }
                }
                // else if it is native, unwrap first and then send to royalty recipient
            }
        }
        None => {
            // do nothing
        }
    }

    Ok(())
}
