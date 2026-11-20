//! Mint plans with Token-2022 extensions, initialized with real instructions.
//!
//! The plan only decides *which* instructions to build and in what order; every accept/reject
//! decision (valid combinations, initialization order, required account space) comes from the real
//! Token-2022 program when the instructions run.

use solana_instruction::Instruction;
use solana_pubkey::Pubkey;
use spl_token_2022_interface::{
    extension::{
        ExtensionType, default_account_state, group_member_pointer, group_pointer,
        interest_bearing_mint, metadata_pointer, pausable, scaled_ui_amount, transfer_fee,
        transfer_hook,
    },
    instruction as token_ix,
    state::{AccountState, Mint},
};

use crate::{ExecError, Harness, derive_address};

/// One mint extension to initialize.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MintExt {
    /// `TransferFeeConfig` (config and withdraw authorities are the harness payer).
    TransferFee {
        /// Fee in basis points.
        basis_points: u16,
        /// Maximum fee per transfer.
        maximum_fee: u64,
    },
    /// `TransferHook` pointing at `program_id` (hook authority is the harness payer).
    TransferHook {
        /// The hook program.
        program_id: Pubkey,
    },
    /// `DefaultAccountState`: new token accounts start frozen or initialized.
    DefaultAccountState {
        /// `true` for frozen by default.
        frozen: bool,
    },
    /// `Pausable` (pause authority is the harness payer).
    Pausable,
    /// `PermanentDelegate` with the given delegate.
    PermanentDelegate {
        /// The permanent delegate.
        delegate: Pubkey,
    },
    /// `NonTransferable`.
    NonTransferable,
    /// `MintCloseAuthority` (the harness payer).
    MintCloseAuthority,
    /// `InterestBearingConfig` with a rate in basis points (rate authority: the harness payer).
    InterestBearing {
        /// Rate in basis points.
        rate: i16,
    },
    /// `ScaledUiAmount` with multiplier `multiplier_milli / 1000` (authority: the harness payer).
    ScaledUiAmount {
        /// Multiplier in thousandths.
        multiplier_milli: u32,
    },
    /// `MetadataPointer` with an optional authority and metadata address.
    MetadataPointer {
        /// Authority that can update the metadata address.
        authority: Option<Pubkey>,
        /// Address holding the metadata.
        metadata_address: Option<Pubkey>,
    },
    /// `GroupPointer` with an optional authority and group address.
    GroupPointer {
        /// Authority that can update the group address.
        authority: Option<Pubkey>,
        /// Address holding the group.
        group_address: Option<Pubkey>,
    },
    /// `GroupMemberPointer` with an optional authority and member address.
    GroupMemberPointer {
        /// Authority that can update the member address.
        authority: Option<Pubkey>,
        /// Address holding the member.
        member_address: Option<Pubkey>,
    },
}

impl MintExt {
    /// The `ExtensionType` this adds to the mint account.
    pub fn extension_type(&self) -> ExtensionType {
        match self {
            Self::TransferFee { .. } => ExtensionType::TransferFeeConfig,
            Self::TransferHook { .. } => ExtensionType::TransferHook,
            Self::DefaultAccountState { .. } => ExtensionType::DefaultAccountState,
            Self::Pausable => ExtensionType::Pausable,
            Self::PermanentDelegate { .. } => ExtensionType::PermanentDelegate,
            Self::NonTransferable => ExtensionType::NonTransferable,
            Self::MintCloseAuthority => ExtensionType::MintCloseAuthority,
            Self::InterestBearing { .. } => ExtensionType::InterestBearingConfig,
            Self::ScaledUiAmount { .. } => ExtensionType::ScaledUiAmount,
            Self::MetadataPointer { .. } => ExtensionType::MetadataPointer,
            Self::GroupPointer { .. } => ExtensionType::GroupPointer,
            Self::GroupMemberPointer { .. } => ExtensionType::GroupMemberPointer,
        }
    }

    fn init_ix(&self, mint: &Pubkey, payer: &Pubkey) -> Result<Instruction, ExecError> {
        let token = spl_token_2022_interface::id();
        let ix = match self {
            Self::TransferFee {
                basis_points,
                maximum_fee,
            } => transfer_fee::instruction::initialize_transfer_fee_config(
                &token,
                mint,
                Some(payer),
                Some(payer),
                *basis_points,
                *maximum_fee,
            ),
            Self::TransferHook { program_id } => transfer_hook::instruction::initialize(
                &token,
                mint,
                Some(*payer),
                Some(*program_id),
            ),
            Self::DefaultAccountState { frozen } => {
                default_account_state::instruction::initialize_default_account_state(
                    &token,
                    mint,
                    if *frozen {
                        &AccountState::Frozen
                    } else {
                        &AccountState::Initialized
                    },
                )
            }
            Self::Pausable => pausable::instruction::initialize(&token, mint, payer),
            Self::PermanentDelegate { delegate } => {
                token_ix::initialize_permanent_delegate(&token, mint, delegate)
            }
            Self::NonTransferable => token_ix::initialize_non_transferable_mint(&token, mint),
            Self::MintCloseAuthority => {
                token_ix::initialize_mint_close_authority(&token, mint, Some(payer))
            }
            Self::InterestBearing { rate } => {
                interest_bearing_mint::instruction::initialize(&token, mint, Some(*payer), *rate)
            }
            Self::ScaledUiAmount { multiplier_milli } => scaled_ui_amount::instruction::initialize(
                &token,
                mint,
                Some(*payer),
                f64::from(*multiplier_milli) / 1000.0,
            ),
            Self::MetadataPointer {
                authority,
                metadata_address,
            } => metadata_pointer::instruction::initialize(
                &token,
                mint,
                *authority,
                *metadata_address,
            ),
            Self::GroupPointer {
                authority,
                group_address,
            } => group_pointer::instruction::initialize(&token, mint, *authority, *group_address),
            Self::GroupMemberPointer {
                authority,
                member_address,
            } => group_member_pointer::instruction::initialize(
                &token,
                mint,
                *authority,
                *member_address,
            ),
        };
        ix.map_err(|e| ExecError::Setup {
            step: "extension-init-ix".into(),
            detail: e.to_string(),
        })
    }
}

/// A mint to create: decimals, freeze authority and extensions (initialized in this order).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MintPlan {
    /// Mint decimals.
    pub decimals: u8,
    /// Whether the harness payer is the freeze authority.
    pub freeze_authority: bool,
    /// Extensions, initialized in order before `InitializeMint2`.
    pub extensions: Vec<MintExt>,
}

impl MintPlan {
    /// Plan with the payer as freeze authority and the given extensions.
    pub fn new(decimals: u8, extensions: Vec<MintExt>) -> Self {
        Self {
            decimals,
            freeze_authority: true,
            extensions,
        }
    }
}

impl Harness {
    /// Build (but do not run) the instructions that create and initialize the mint for `plan`:
    /// `CreateAccount`, each extension initializer in plan order, then `InitializeMint2`.
    pub fn mint_init_instructions(
        &self,
        label: &str,
        plan: &MintPlan,
    ) -> Result<(Pubkey, Vec<Instruction>), ExecError> {
        let mint = derive_address(&format!("mint/{label}"));
        let token = spl_token_2022_interface::id();
        let types: Vec<ExtensionType> = plan
            .extensions
            .iter()
            .map(MintExt::extension_type)
            .collect();
        let space = ExtensionType::try_calculate_account_len::<Mint>(&types).map_err(|e| {
            ExecError::Setup {
                step: "mint-size".into(),
                detail: e.to_string(),
            }
        })?;
        let payer = self.payer();
        let mut ixs = vec![solana_system_interface::instruction::create_account(
            &payer,
            &mint,
            self.rent_exempt_balance(space),
            space as u64,
            &token,
        )];
        for ext in &plan.extensions {
            ixs.push(ext.init_ix(&mint, &payer)?);
        }
        let freeze = plan.freeze_authority.then_some(&payer);
        ixs.push(
            token_ix::initialize_mint2(&token, &mint, &payer, freeze, plan.decimals).map_err(
                |e| ExecError::Setup {
                    step: "init-mint-ix".into(),
                    detail: e.to_string(),
                },
            )?,
        );
        Ok((mint, ixs))
    }

    /// Create and initialize a mint for `plan` with real instructions. A rejected combination or
    /// ordering surfaces as `ExecError::Setup` carrying the runtime's error and logs context.
    pub fn create_mint_ext(&self, label: &str, plan: &MintPlan) -> Result<Pubkey, ExecError> {
        let (mint, ixs) = self.mint_init_instructions(label, plan)?;
        for ix in &ixs {
            let rep = self.execute_logged(ix);
            if let Some(e) = rep.error {
                return Err(ExecError::Setup {
                    step: "create-mint-ext".into(),
                    detail: format!("{e}; logs: {:?}", rep.logs),
                });
            }
        }
        Ok(mint)
    }
}
