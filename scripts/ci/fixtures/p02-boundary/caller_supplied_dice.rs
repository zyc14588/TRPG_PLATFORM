use trpg_ruleset_coc7::dice_roll_contract::{
    record_dice_roll_contract, DiceRollOutcome,
};
use trpg_ruleset_coc7::Coc7EventPayload;
use trpg_shared_kernel::{AuthorityContract, CommandEnvelope, EventStore};

fn attempt<T>(
    contract: &AuthorityContract,
    store: &mut EventStore<Coc7EventPayload>,
    command: &CommandEnvelope<T>,
    caller_supplied: &DiceRollOutcome,
) {
    let _ = record_dice_roll_contract(contract, store, command, caller_supplied);
}

fn main() {}
