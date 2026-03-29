#![no_std]
use soroban_sdk::{contract, contracttype, contractimpl, Address, BytesN, Env, Vec, Symbol, token, testutils::{Address as TestAddress, Arbitrary as TestArbitrary}, arbitrary::{Arbitrary, Unstructured}};
use soroban_sdk::{contract, contracttype, contractimpl, Address, Env, Vec, Symbol, token, String};
use soroban_sdk::testutils::{Address as TestAddress, Arbitrary as TestArbitrary};
use soroban_sdk::arbitrary::{Arbitrary, Unstructured};

pub mod receipt;
pub mod goal_escrow;           // ← NEW: Goal Escrow Module

// --- DATA STRUCTURES ---
const TAX_WITHHOLDING_MIN_BPS: u32 = 1000; // 10%
const TAX_WITHHOLDING_MAX_BPS: u32 = 2000; // 20%
const DEFAULT_TAX_WITHHOLDING_BPS: u32 = TAX_WITHHOLDING_MIN_BPS;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    Circle(u64),
    Member(u64, Address),
    CircleCount,
    Deposit(u64, Address),
    GroupReserve,
    // #225: Duration Proposals
    Proposal(u64, u64),
    ProposalCount(u64),
    Vote(u64, u64, Address),
    // #227: Bond Storage
    Bond(u64),
    // #228: Governance
    Stake(Address),
    GlobalFeeBP, // Basis points
    // Tax Withholding Escrow for Interest Earnings
    TaxVault(u64, Address),          // circle_id, user
    TaxWithheldTotal(u64),           // circle_id
    TaxClaimedTotal(u64),            // circle_id
    TaxWithheldByUser(u64, Address), // circle_id, user
    TaxClaimedByUser(u64, Address),  // circle_id, user
    GrossInterestTotal(u64),         // circle_id
    GrossInterestByUser(u64, Address), // circle_id, user
    TaxWithholdingBps(u64),          // circle_id
    TaxReleasedTotal(u64),           // circle_id
    TaxReleasedByUser(u64, Address), // circle_id, user
    TaxFilingProof(u64, Address),    // circle_id, user => proof hash
    TaxProofTimestamp(u64, Address), // circle_id, user
    GlobalFeeBP,
    // #234: Goal Escrow Storage
    GoalEscrow(u32),           // Escrow ID → GoalEscrow
    NextEscrowId,              // Counter for escrow IDs
}

#[contracttype]
#[derive(Clone, Debug)]
pub enum EscrowStatus {
    PendingInvoice,
    AwaitingDelivery,
    Delivered,
    Cancelled,
}

#[contracttype]
#[derive(Clone)]
pub struct GoalEscrow {
    pub id: u32,
    pub winner: Address,
    pub group_id: u32,
    pub amount: u128,
    pub asset: Address,
    pub vendor: Address,
    pub invoice_reference: String,
    pub status: EscrowStatus,
    pub created_at: u64,
    pub delivery_confirmed_at: Option<u64>,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct DurationProposal {
    pub id: u64,
    pub new_duration: u64,
    pub votes_for: u16,
    pub votes_against: u16,
    pub end_time: u64,
    pub is_active: bool,
}

#[contracttype]
#[derive(Clone)]
pub struct Member {
    pub address: Address,
    pub has_contributed: bool,
    pub contribution_count: u32,
    pub last_contribution_time: u64,
}

#[contracttype]
#[derive(Clone)]
pub struct CircleInfo {
    pub id: u64,
    pub creator: Address,
    pub contribution_amount: u64,
    pub max_members: u16,
    pub member_count: u16,
    pub current_recipient_index: u16,
    pub is_active: bool,
    pub token: Address,
    pub deadline_timestamp: u64,
    pub cycle_duration: u64,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct TaxReport {
    pub circle_id: u64,
    pub user: Address,
    pub gross_interest_total_for_circle: u64,
    pub gross_interest_for_user: u64,
    pub total_tax_withheld_for_circle: u64,
    pub total_tax_withheld_for_user: u64,
    pub total_tax_claimed_for_circle: u64,
    pub total_tax_claimed_for_user: u64,
    pub total_tax_released_for_circle: u64,
    pub total_tax_released_for_user: u64,
    pub current_tax_vault_balance: u64,
    pub withholding_bps: u32,
    pub has_tax_filing_proof: bool,
    pub last_tax_proof_timestamp: u64,
}

// --- CONTRACT TRAIT ---

pub trait SoroSusuTrait {
    fn init(env: Env, admin: Address, global_fee: u32);
    
    fn create_circle(env: Env, creator: Address, amount: u64, max_members: u16, token: Address, cycle_duration: u64, bond_amount: u64) -> u64;

    fn join_circle(env: Env, user: Address, circle_id: u64);

    fn deposit(env: Env, user: Address, circle_id: u64, rounds: u32);

    // #225: Variable Round Duration
    fn propose_duration(env: Env, user: Address, circle_id: u64, new_duration: u64) -> u64;
    fn vote_duration(env: Env, user: Address, circle_id: u64, proposal_id: u64, approve: bool);

    // #227: Bond Management
    fn slash_bond(env: Env, admin: Address, circle_id: u64);
    fn release_bond(env: Env, admin: Address, circle_id: u64);

    // #228: XLM Staking & Governance
    fn stake_xlm(env: Env, user: Address, xlm_token: Address, amount: u64);
    fn unstake_xlm(env: Env, user: Address, xlm_token: Address, amount: u64);
    fn update_global_fee(env: Env, admin: Address, new_fee: u32);

    // Tax Withholding Escrow for Interest Earnings
    fn set_tax_withholding_rate(env: Env, admin: Address, circle_id: u64, withholding_bps: u32);
    fn get_tax_withholding_rate(env: Env, circle_id: u64) -> u32;
    fn process_interest_earning(env: Env, operator: Address, circle_id: u64, beneficiary: Address, gross_interest: u64) -> (u64, u64);
    fn claim_tax_vault(env: Env, user: Address, circle_id: u64, tax_recipient: Address) -> u64;
    fn provide_tax_filing_proof(env: Env, user: Address, circle_id: u64, proof_hash: BytesN<32>);
    fn release_tax_vault(env: Env, user: Address, circle_id: u64) -> u64;
    fn get_tax_vault_balance(env: Env, user: Address, circle_id: u64) -> u64;
    fn get_total_tax_withheld(env: Env, circle_id: u64) -> u64;
    fn get_total_tax_claimed(env: Env, circle_id: u64) -> u64;
    fn get_total_tax_released(env: Env, circle_id: u64) -> u64;
    fn get_tax_report(env: Env, user: Address, circle_id: u64) -> TaxReport;
}

fn checked_add_u64(a: u64, b: u64, context: &str) -> u64 {
    a.checked_add(b).unwrap_or_else(|| panic!("{}", context))
}

fn validate_tax_withholding_bps(withholding_bps: u32) {
    if !(TAX_WITHHOLDING_MIN_BPS..=TAX_WITHHOLDING_MAX_BPS).contains(&withholding_bps) {
        panic!("Tax withholding must be between 10% and 20%");
    }
}

fn calculate_interest_tax_split(gross_interest: u64, withholding_bps: u32) -> (u64, u64) {
    if gross_interest == 0 {
        return (0, 0);
    }

    validate_tax_withholding_bps(withholding_bps);
    let tax_withheld = (gross_interest * withholding_bps as u64) / 10_000;
    let net_interest = gross_interest - tax_withheld;
    (tax_withheld, net_interest)
    // #233: Receipt Generator
    fn generate_receipt(
        env: Env,
        contributor: Address,
        group_id: u32,
        amount: u128,
        asset_code: String,
        group_name: String,
    ) -> String;

    // #234: Sub-Susu Goal Escrow (Vendor-Direct Payout)
    fn create_goal_escrow(
        env: Env,
        group_id: u32,
        winner: Address,
        amount: u128,
        asset: Address,
        vendor: Address,
        invoice_reference: String,
    ) -> u32;

    fn confirm_delivery(env: Env, escrow_id: u32);

    fn get_goal_escrow(env: Env, escrow_id: u32) -> GoalEscrow;

    fn cancel_goal_escrow(env: Env, escrow_id: u32);
}

// --- IMPLEMENTATION ---

#[contract]
pub struct SoroSusu;

#[contractimpl]
impl SoroSusuTrait for SoroSusu {
    fn init(env: Env, admin: Address, global_fee: u32) {
        // Initialize the circle counter to 0 if it doesn't exist
        if !env.storage().instance().has(&DataKey::CircleCount) {
            env.storage().instance().set(&DataKey::CircleCount, &0u64);
        }
        // Set the admin
        env.storage().instance().set(&DataKey::Admin, &admin);
        // Set Global Fee BP
        env.storage().instance().set(&DataKey::GlobalFeeBP, &global_fee);
    }

    fn create_circle(env: Env, creator: Address, amount: u64, max_members: u16, token: Address, cycle_duration: u64, bond_amount: u64) -> u64 {
        // #227: Creator MUST pay a bond
        creator.require_auth();
        let client = token::Client::new(&env, &token);
        client.transfer(&creator, &env.current_contract_address(), &bond_amount);
        
        // 1. Get the current Circle Count
        let mut circle_count: u64 = env.storage().instance().get(&DataKey::CircleCount).unwrap_or(0);
        
        // 2. Increment the ID for the new circle
        circle_count += 1;

        // 3. Create the Circle Data Struct
        let current_time = env.ledger().timestamp();
        let new_circle = CircleInfo {
            id: circle_count,
            creator: creator.clone(),
            contribution_amount: amount,
            max_members,
            member_count: 0,
            current_recipient_index: 0,
            is_active: true,
            token,
            deadline_timestamp: current_time + cycle_duration,
            cycle_duration,
        };

        // 4. Save the Circle, Bond, and Count
        env.storage().instance().set(&DataKey::Circle(circle_count), &new_circle);
        env.storage().instance().set(&DataKey::Bond(circle_count), &bond_amount);
        env.storage()
            .instance()
            .set(&DataKey::TaxWithholdingBps(circle_count), &DEFAULT_TAX_WITHHOLDING_BPS);
        env.storage().instance().set(&DataKey::CircleCount, &circle_count);

        // 5. Initialize Group Reserve if not exists
        if !env.storage().instance().has(&DataKey::GroupReserve) {
            env.storage().instance().set(&DataKey::GroupReserve, &0u64);
        }

        // 6. Return the new ID
        circle_count
    }

    fn join_circle(env: Env, user: Address, circle_id: u64) {
        // 1. Authorization: The user MUST sign this transaction
        user.require_auth();

        // 2. Retrieve the circle data
        let mut circle: CircleInfo = env.storage().instance().get(&DataKey::Circle(circle_id)).unwrap();

        // 3. Check if the circle is full
        if circle.member_count >= circle.max_members {
            panic!("Circle is full");
        }

        // 4. Check if user is already a member to prevent duplicates
        let member_key = DataKey::Member(circle_id, user.clone());
        if env.storage().instance().has(&member_key) {
            panic!("User is already a member");
        }

        // 5. Create and store the new member
        let new_member = Member {
            address: user.clone(),
            has_contributed: false,
            contribution_count: 0,
            last_contribution_time: 0,
    // ... (all your existing functions remain unchanged: init, create_circle, join_circle, deposit, propose_duration, vote_duration, slash_bond, release_bond, stake_xlm, unstake_xlm, update_global_fee, generate_receipt)

    // Keep all your existing implementations here...
    // (I'm omitting them for brevity — they stay exactly as you had them)

    // ==================== NEW: GOAL ESCROW FUNCTIONS (#234) ====================

    fn create_goal_escrow(
        env: Env,
        group_id: u32,
        winner: Address,
        amount: u128,
        asset: Address,
        vendor: Address,
        invoice_reference: String,
    ) -> u32 {
        winner.require_auth(); // Winner must authorize

        let mut next_id: u32 = env.storage().instance().get(&DataKey::NextEscrowId).unwrap_or(0);
        next_id += 1;

        let escrow = GoalEscrow {
            id: next_id,
            winner: winner.clone(),
            group_id,
            amount,
            asset: asset.clone(),
            vendor: vendor.clone(),
            invoice_reference,
            status: EscrowStatus::AwaitingDelivery,
            created_at: env.ledger().timestamp(),
            delivery_confirmed_at: None,
        };

        // Transfer funds into escrow (from contract balance)
        let token_client = token::Client::new(&env, &asset);
        token_client.transfer(&env.current_contract_address(), &env.current_contract_address(), &(amount as i128)); // Self-transfer to lock

        env.storage().instance().set(&DataKey::GoalEscrow(next_id), &escrow);
        env.storage().instance().set(&DataKey::NextEscrowId, &next_id);

        env.events().publish((Symbol::new(&env, "goal_escrow_created"),), (next_id, winner, amount, vendor));

        next_id
    }

    fn confirm_delivery(env: Env, escrow_id: u32) {
        let mut escrow: GoalEscrow = env.storage().instance().get(&DataKey::GoalEscrow(escrow_id))
            .unwrap_or_else(|| panic!("Escrow not found"));

        escrow.winner.require_auth();

        if escrow.status != EscrowStatus::AwaitingDelivery {
            panic!("Invalid escrow state");
        }

        // Release funds to vendor
        let token_client = token::Client::new(&env, &escrow.asset);
        token_client.transfer(&env.current_contract_address(), &escrow.vendor, &(escrow.amount as i128));

        escrow.status = EscrowStatus::Delivered;
        escrow.delivery_confirmed_at = Some(env.ledger().timestamp());

        env.storage().instance().set(&DataKey::GoalEscrow(escrow_id), &escrow);

        env.events().publish((Symbol::new(&env, "goal_escrow_delivered"),), (escrow_id, escrow.vendor, escrow.amount));
    }

    fn get_goal_escrow(env: Env, escrow_id: u32) -> GoalEscrow {
        env.storage().instance().get(&DataKey::GoalEscrow(escrow_id))
            .unwrap_or_else(|| panic!("Escrow not found"))
    }

    fn cancel_goal_escrow(env: Env, escrow_id: u32) {
        // TODO: Implement admin or timeout-based cancellation
        // For now, stub
        panic!("Cancel not yet implemented");
    }

    // Keep your existing generate_receipt function
    fn generate_receipt(
        env: Env,
        contributor: Address,
        group_id: u32,
        amount: u128,
        asset_code: String,
        group_name: String,
    ) -> String {
        Self::generate_receipt(env, contributor, group_id, amount, asset_code, group_name)
    }
}
