use borsh::{BorshDeserialize, BorshSerialize};
use solana_program::{
    account_info::{next_account_info, AccountInfo},
    clock::Clock,
    entrypoint,
    entrypoint::ProgramResult,
    msg,
    program_error::ProgramError,
    pubkey::Pubkey,
};
use thiserror::Error;

entrypoint!(process_instruction);

#[derive(Debug, Error, PartialEq, Eq, Clone)]
pub enum PredictChatError {
    #[error("Account does not have the expected owner")]
    InvalidOwner,
    #[error("Account is already initialized")]
    AlreadyInitialized,
    #[error("Prediction is already settled")]
    AlreadySettled,
    #[error("Prediction cannot be settled before expiry")]
    NotExpired,
    #[error("Prediction account is tied to a different room")]
    InvalidRoom,
    #[error("Oracle account is too small to contain a price feed")]
    OracleDataTooSmall,
}

impl From<PredictChatError> for ProgramError {
    fn from(value: PredictChatError) -> Self {
        ProgramError::Custom(value as u32)
    }
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub struct RoomState {
    pub authority: Pubkey,
    pub oracle_feed: Pubkey,
    pub staking_mint: Pubkey,
    pub stake_vault: Pubkey,
    pub bump: u8,
}

#[derive(BorshSerialize, BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub struct PredictionState {
    pub user: Pubkey,
    pub room: Pubkey,
    pub predicted_price: i64,
    pub expiry_slot: u64,
    pub stake: u64,
    pub resolved: bool,
    pub won: bool,
}

#[derive(BorshDeserialize, Debug, Clone, PartialEq, Eq)]
pub enum PredictInstruction {
    InitializeRoom {
        oracle_feed: Pubkey,
        staking_mint: Pubkey,
        stake_vault: Pubkey,
        bump: u8,
    },
    StakeAndCommit {
        predicted_price: i64,
        expiry_slot: u64,
        stake: u64,
    },
    SettlePrediction {},
}

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    let instruction = PredictInstruction::try_from_slice(instruction_data)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    match instruction {
        PredictInstruction::InitializeRoom {
            oracle_feed,
            staking_mint,
            stake_vault,
            bump,
        } => process_initialize_room(program_id, accounts, oracle_feed, staking_mint, stake_vault, bump),
        PredictInstruction::StakeAndCommit {
            predicted_price,
            expiry_slot,
            stake,
        } => process_stake_and_commit(program_id, accounts, predicted_price, expiry_slot, stake),
        PredictInstruction::SettlePrediction {} => process_settle_prediction(program_id, accounts),
    }
}

fn process_initialize_room(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    oracle_feed: Pubkey,
    staking_mint: Pubkey,
    stake_vault: Pubkey,
    bump: u8,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let room_account = next_account_info(account_info_iter)?;
    let authority = next_account_info(account_info_iter)?;

    if room_account.owner != program_id {
        return Err(PredictChatError::InvalidOwner.into());
    }

    if !room_account.data_is_empty() {
        return Err(PredictChatError::AlreadyInitialized.into());
    }

    let room_state = RoomState {
        authority: *authority.key,
        oracle_feed,
        staking_mint,
        stake_vault,
        bump,
    };

    room_state.serialize(&mut &mut room_account.data.borrow_mut()[..])?;
    msg!("Room initialized by {}", authority.key);

    Ok(())
}

fn process_stake_and_commit(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    predicted_price: i64,
    expiry_slot: u64,
    stake: u64,
) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let prediction_account = next_account_info(account_info_iter)?;
    let user = next_account_info(account_info_iter)?;
    let room_account = next_account_info(account_info_iter)?;

    if prediction_account.owner != program_id {
        return Err(PredictChatError::InvalidOwner.into());
    }

    if room_account.owner != program_id {
        return Err(PredictChatError::InvalidOwner.into());
    }

    if !prediction_account.data_is_empty() {
        return Err(PredictChatError::AlreadyInitialized.into());
    }

    let _room_state = RoomState::try_from_slice(&room_account.data.borrow())?;

    let prediction_state = PredictionState {
        user: *user.key,
        room: *room_account.key,
        predicted_price,
        expiry_slot,
        stake,
        resolved: false,
        won: false,
    };

    prediction_state.serialize(&mut &mut prediction_account.data.borrow_mut()[..])?;
    msg!(
        "User {} committed prediction {} with stake {}",
        user.key, predicted_price, stake
    );

    Ok(())
}

fn process_settle_prediction(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let account_info_iter = &mut accounts.iter();
    let prediction_account = next_account_info(account_info_iter)?;
    let room_account = next_account_info(account_info_iter)?;
    let oracle_price_account = next_account_info(account_info_iter)?;

    if prediction_account.owner != program_id || room_account.owner != program_id {
        return Err(PredictChatError::InvalidOwner.into());
    }

    let mut prediction_state = PredictionState::try_from_slice(&prediction_account.data.borrow())?;
    let _room_state = RoomState::try_from_slice(&room_account.data.borrow())?;

    if prediction_state.resolved {
        return Err(PredictChatError::AlreadySettled.into());
    }

    if prediction_state.room != *room_account.key {
        return Err(PredictChatError::InvalidRoom.into());
    }

    let clock = Clock::get()?;
    if clock.slot < prediction_state.expiry_slot {
        return Err(PredictChatError::NotExpired.into());
    }

    const MIN_ORACLE_SIZE: usize = 8;
    if oracle_price_account.data_len() < MIN_ORACLE_SIZE {
        return Err(PredictChatError::OracleDataTooSmall.into());
    }

    let oracle_price_bytes = oracle_price_account.data.borrow();
    let observed_price = i64::from_le_bytes(
        oracle_price_bytes[0..8]
            .try_into()
            .map_err(|_| ProgramError::InvalidAccountData)?,
    );

    prediction_state.won = observed_price >= prediction_state.predicted_price;
    prediction_state.resolved = true;

    prediction_state.serialize(&mut &mut prediction_account.data.borrow_mut()[..])?;
    msg!(
        "Prediction settled. Observed price {}, target {}, won: {}",
        observed_price, prediction_state.predicted_price, prediction_state.won
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_program::clock::Clock;

    fn program_id() -> Pubkey {
        Pubkey::new_unique()
    }

    #[test]
    fn serialize_room_and_prediction() {
        let room = RoomState {
            authority: Pubkey::new_unique(),
            oracle_feed: Pubkey::new_unique(),
            staking_mint: Pubkey::new_unique(),
            stake_vault: Pubkey::new_unique(),
            bump: 255,
        };

        let mut data = vec![0u8; room.try_to_vec().unwrap().len()];
        room.serialize(&mut data.as_mut_slice()).unwrap();
        let restored = RoomState::try_from_slice(&data).unwrap();
        assert_eq!(room, restored);

        let prediction = PredictionState {
            user: Pubkey::new_unique(),
            room: Pubkey::new_unique(),
            predicted_price: 50_000,
            expiry_slot: 1_000,
            stake: 10_000,
            resolved: false,
            won: false,
        };

        let mut p_data = vec![0u8; prediction.try_to_vec().unwrap().len()];
        prediction.serialize(&mut p_data.as_mut_slice()).unwrap();
        let restored_prediction = PredictionState::try_from_slice(&p_data).unwrap();
        assert_eq!(prediction, restored_prediction);
    }

    #[test]
    fn settle_sets_won_flag() {
        let program_id = program_id();
        let room = RoomState {
            authority: Pubkey::new_unique(),
            oracle_feed: Pubkey::new_unique(),
            staking_mint: Pubkey::new_unique(),
            stake_vault: Pubkey::new_unique(),
            bump: 1,
        };

        let mut room_data = vec![0u8; room.try_to_vec().unwrap().len()];
        room.serialize(&mut room_data.as_mut_slice()).unwrap();

        let mut prediction = PredictionState {
            user: Pubkey::new_unique(),
            room: Pubkey::new_unique(),
            predicted_price: 30_000,
            expiry_slot: Clock::default().slot,
            stake: 100,
            resolved: false,
            won: false,
        };

        let mut prediction_data = vec![0u8; prediction.try_to_vec().unwrap().len()];
        prediction
            .serialize(&mut prediction_data.as_mut_slice())
            .unwrap();

        let oracle_price: i64 = 35_000;
        let mut oracle_data = oracle_price.to_le_bytes().to_vec();

        let room_account = solana_program::account_info::AccountInfo::new(
            &prediction.room,
            false,
            true,
            &mut 0u64,
            &mut room_data,
            &program_id,
            false,
            0,
        );

        let prediction_account = solana_program::account_info::AccountInfo::new(
            &Pubkey::new_unique(),
            false,
            true,
            &mut 0u64,
            &mut prediction_data,
            &program_id,
            false,
            0,
        );

        let oracle_account = solana_program::account_info::AccountInfo::new(
            &room.oracle_feed,
            false,
            false,
            &mut 0u64,
            &mut oracle_data,
            &Pubkey::new_unique(),
            false,
            0,
        );

        let accounts = vec![prediction_account, room_account, oracle_account];
        process_settle_prediction(&program_id, &accounts).unwrap();

        let resolved_prediction =
            PredictionState::try_from_slice(&accounts[0].data.borrow()).unwrap();
        assert!(resolved_prediction.resolved);
        assert!(resolved_prediction.won);
    }
}
