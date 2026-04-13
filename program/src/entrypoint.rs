//! Program entrypoint

#![cfg(all(target_os = "solana", not(feature = "no-entrypoint")))]

use {
    crate::{error::StakePoolError, processor::Processor},
    solana_account_info::AccountInfo,
    solana_msg::msg,
    solana_program_error::ProgramResult,
    solana_pubkey::Pubkey,
    solana_security_txt::security_txt,
};

solana_program_entrypoint::entrypoint!(process_instruction);
fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if let Err(error) = Processor::process(program_id, accounts, instruction_data) {
        // catch the error so we can print it
        msg!(error.to_str::<StakePoolError>());
        Err(error)
    } else {
        Ok(())
    }
}

security_txt! {
    // Required fields
    name: "SPL Stake Pool",
    project_url: "https://www.solana-program.com/docs/stake-pool",
    contacts: "link:https://github.com/solana-program/stake-pool/security/advisories/new,email:security@anza.xyz,discord:https://solana.com/discord",
    policy: "https://github.com/solana-program/stake-pool/blob/master/SECURITY.md",

    // Optional Fields
    preferred_languages: "en",
    source_code: "https://github.com/solana-program/stake-pool",
    source_release: concat!("program@v", env!("CARGO_PKG_VERSION")),
    auditors: "https://github.com/anza-xyz/security-audits#stake-pool"
}
