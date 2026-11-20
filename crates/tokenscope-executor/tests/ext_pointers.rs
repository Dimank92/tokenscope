//! Pointer extensions against the real Token-2022 program in Mollusk SVM:
//! * `MetadataPointer`
//! * `GroupPointer`
//! * `GroupMemberPointer`
//!
//! Scope boundary: pointer extensions store an address pointer and update authority;
//! they do not require or manipulate TokenMetadata or TokenGroup account data.
//!
//! Verified semantics:
//! * Pointer initialization with authority and target address (or None).
//! * Pointer updates by authorized authority (including clearing to None).
//! * Unauthorized updates fail with `OwnerMismatch` (error code 4).
//! * Immutable pointers (authority = None) reject all updates with `NoAuthorityExists` (error code 29).
//! * Multiple pointer extensions coexist on a single mint without interference.

mod common;

use common::*;
use solana_pubkey::Pubkey;
use spl_token_2022_interface::error::TokenError;
use tokenscope_executor::{MintExt, MintPlan, PointerView, derive_address};

const NO_AUTHORITY: u32 = TokenError::NoAuthorityExists as u32;

fn pv(authority: Option<Pubkey>, address: Option<Pubkey>) -> Option<PointerView> {
    Some(PointerView { authority, address })
}

#[test]
fn metadata_pointer_lifecycle_and_updates() {
    let h = tokenscope_executor::Harness::new();
    let payer = h.payer();
    let mint_addr = derive_address("mint/meta-lifecycle");
    let plan = MintPlan::new(
        6,
        vec![MintExt::MetadataPointer {
            authority: Some(payer),
            metadata_address: Some(mint_addr),
        }],
    );
    let mint = h.create_mint_ext("meta-lifecycle", &plan).unwrap();
    assert_eq!(
        h.mint_metadata_pointer(&mint).unwrap(),
        pv(Some(payer), Some(mint))
    );

    // Update metadata address to a separate metadata account.
    let meta_1 = derive_address("metadata-account-1");
    let update_ix = h
        .update_metadata_pointer_ix(&mint, &payer, Some(meta_1))
        .unwrap();
    let rep = h.execute_logged(&update_ix);
    assert!(
        rep.error.is_none(),
        "update metadata pointer: {:?}",
        rep.error
    );
    assert_eq!(
        h.mint_metadata_pointer(&mint).unwrap(),
        pv(Some(payer), Some(meta_1))
    );

    // Clear metadata address by updating to None.
    let clear_ix = h.update_metadata_pointer_ix(&mint, &payer, None).unwrap();
    let rep = h.execute_logged(&clear_ix);
    assert!(
        rep.error.is_none(),
        "clear metadata pointer: {:?}",
        rep.error
    );
    assert_eq!(
        h.mint_metadata_pointer(&mint).unwrap(),
        pv(Some(payer), None)
    );
}

#[test]
fn metadata_pointer_unauthorized_update_is_rejected() {
    let h = tokenscope_executor::Harness::new();
    let alice = derive_address("alice");
    let mallory = derive_address("mallory");
    let mint_addr = derive_address("mint/meta-unauth");
    let plan = MintPlan::new(
        6,
        vec![MintExt::MetadataPointer {
            authority: Some(alice),
            metadata_address: Some(mint_addr),
        }],
    );
    let mint = h.create_mint_ext("meta-unauth", &plan).unwrap();

    // Mallory attempts to update the pointer.
    let evil_target = derive_address("evil-metadata");
    let update_ix = h
        .update_metadata_pointer_ix(&mint, &mallory, Some(evil_target))
        .unwrap();
    let rep = h.execute_logged(&update_ix);

    assert_eq!(
        code(&rep),
        Some(OWNER_MISMATCH),
        "unauthorized update must fail with OwnerMismatch: {rep:?}"
    );
    assert_eq!(
        h.mint_metadata_pointer(&mint).unwrap(),
        pv(Some(alice), Some(mint)),
        "pointer must remain unchanged"
    );
}

#[test]
fn metadata_pointer_immutable_rejects_updates_with_no_authority() {
    let h = tokenscope_executor::Harness::new();
    let payer = h.payer();
    let mint_addr = derive_address("mint/meta-immutable");
    let plan = MintPlan::new(
        6,
        vec![MintExt::MetadataPointer {
            authority: None,
            metadata_address: Some(mint_addr),
        }],
    );
    let mint = h.create_mint_ext("meta-immutable", &plan).unwrap();
    assert_eq!(
        h.mint_metadata_pointer(&mint).unwrap(),
        pv(None, Some(mint)),
        "pointer is immutable (no authority)"
    );

    // Any update attempt must fail with NoAuthorityExists (error 29).
    let new_target = derive_address("new-meta");
    let update_ix = h
        .update_metadata_pointer_ix(&mint, &payer, Some(new_target))
        .unwrap();
    let rep = h.execute_logged(&update_ix);

    assert_eq!(
        code(&rep),
        Some(NO_AUTHORITY),
        "immutable pointer update must fail with NoAuthorityExists: {rep:?}"
    );
    assert_eq!(
        h.mint_metadata_pointer(&mint).unwrap(),
        pv(None, Some(mint)),
        "pointer must remain immutable"
    );
}

#[test]
fn group_pointer_lifecycle_and_updates() {
    let h = tokenscope_executor::Harness::new();
    let payer = h.payer();
    let mint_addr = derive_address("mint/group-lifecycle");
    let plan = MintPlan::new(
        6,
        vec![MintExt::GroupPointer {
            authority: Some(payer),
            group_address: Some(mint_addr),
        }],
    );
    let mint = h.create_mint_ext("group-lifecycle", &plan).unwrap();
    assert_eq!(
        h.mint_group_pointer(&mint).unwrap(),
        pv(Some(payer), Some(mint))
    );

    let group_1 = derive_address("group-account-1");
    let update_ix = h
        .update_group_pointer_ix(&mint, &payer, Some(group_1))
        .unwrap();
    let rep = h.execute_logged(&update_ix);
    assert!(rep.error.is_none(), "update group pointer: {:?}", rep.error);
    assert_eq!(
        h.mint_group_pointer(&mint).unwrap(),
        pv(Some(payer), Some(group_1))
    );

    let clear_ix = h.update_group_pointer_ix(&mint, &payer, None).unwrap();
    let rep = h.execute_logged(&clear_ix);
    assert!(rep.error.is_none(), "clear group pointer: {:?}", rep.error);
    assert_eq!(h.mint_group_pointer(&mint).unwrap(), pv(Some(payer), None));
}

#[test]
fn group_pointer_unauthorized_update_is_rejected() {
    let h = tokenscope_executor::Harness::new();
    let alice = derive_address("alice");
    let mallory = derive_address("mallory");
    let mint_addr = derive_address("mint/group-unauth");
    let plan = MintPlan::new(
        6,
        vec![MintExt::GroupPointer {
            authority: Some(alice),
            group_address: Some(mint_addr),
        }],
    );
    let mint = h.create_mint_ext("group-unauth", &plan).unwrap();

    let evil_target = derive_address("evil-group");
    let update_ix = h
        .update_group_pointer_ix(&mint, &mallory, Some(evil_target))
        .unwrap();
    let rep = h.execute_logged(&update_ix);

    assert_eq!(
        code(&rep),
        Some(OWNER_MISMATCH),
        "unauthorized group update must fail: {rep:?}"
    );
    assert_eq!(
        h.mint_group_pointer(&mint).unwrap(),
        pv(Some(alice), Some(mint))
    );
}

#[test]
fn group_pointer_immutable_rejects_updates_with_no_authority() {
    let h = tokenscope_executor::Harness::new();
    let payer = h.payer();
    let mint_addr = derive_address("mint/group-immutable");
    let plan = MintPlan::new(
        6,
        vec![MintExt::GroupPointer {
            authority: None,
            group_address: Some(mint_addr),
        }],
    );
    let mint = h.create_mint_ext("group-immutable", &plan).unwrap();
    assert_eq!(h.mint_group_pointer(&mint).unwrap(), pv(None, Some(mint)));

    let new_target = derive_address("new-group");
    let update_ix = h
        .update_group_pointer_ix(&mint, &payer, Some(new_target))
        .unwrap();
    let rep = h.execute_logged(&update_ix);

    assert_eq!(
        code(&rep),
        Some(NO_AUTHORITY),
        "immutable group pointer must fail with NoAuthorityExists: {rep:?}"
    );
    assert_eq!(h.mint_group_pointer(&mint).unwrap(), pv(None, Some(mint)));
}

#[test]
fn group_member_pointer_lifecycle_and_updates() {
    let h = tokenscope_executor::Harness::new();
    let payer = h.payer();
    let mint_addr = derive_address("mint/member-lifecycle");
    let plan = MintPlan::new(
        6,
        vec![MintExt::GroupMemberPointer {
            authority: Some(payer),
            member_address: Some(mint_addr),
        }],
    );
    let mint = h.create_mint_ext("member-lifecycle", &plan).unwrap();
    assert_eq!(
        h.mint_group_member_pointer(&mint).unwrap(),
        pv(Some(payer), Some(mint))
    );

    let member_1 = derive_address("member-account-1");
    let update_ix = h
        .update_group_member_pointer_ix(&mint, &payer, Some(member_1))
        .unwrap();
    let rep = h.execute_logged(&update_ix);
    assert!(
        rep.error.is_none(),
        "update member pointer: {:?}",
        rep.error
    );
    assert_eq!(
        h.mint_group_member_pointer(&mint).unwrap(),
        pv(Some(payer), Some(member_1))
    );

    let clear_ix = h
        .update_group_member_pointer_ix(&mint, &payer, None)
        .unwrap();
    let rep = h.execute_logged(&clear_ix);
    assert!(rep.error.is_none(), "clear member pointer: {:?}", rep.error);
    assert_eq!(
        h.mint_group_member_pointer(&mint).unwrap(),
        pv(Some(payer), None)
    );
}

#[test]
fn group_member_pointer_unauthorized_update_is_rejected() {
    let h = tokenscope_executor::Harness::new();
    let alice = derive_address("alice");
    let mallory = derive_address("mallory");
    let mint_addr = derive_address("mint/member-unauth");
    let plan = MintPlan::new(
        6,
        vec![MintExt::GroupMemberPointer {
            authority: Some(alice),
            member_address: Some(mint_addr),
        }],
    );
    let mint = h.create_mint_ext("member-unauth", &plan).unwrap();

    let evil_target = derive_address("evil-member");
    let update_ix = h
        .update_group_member_pointer_ix(&mint, &mallory, Some(evil_target))
        .unwrap();
    let rep = h.execute_logged(&update_ix);

    assert_eq!(
        code(&rep),
        Some(OWNER_MISMATCH),
        "unauthorized member update must fail: {rep:?}"
    );
    assert_eq!(
        h.mint_group_member_pointer(&mint).unwrap(),
        pv(Some(alice), Some(mint))
    );
}

#[test]
fn group_member_pointer_immutable_rejects_updates_with_no_authority() {
    let h = tokenscope_executor::Harness::new();
    let payer = h.payer();
    let mint_addr = derive_address("mint/member-immutable");
    let plan = MintPlan::new(
        6,
        vec![MintExt::GroupMemberPointer {
            authority: None,
            member_address: Some(mint_addr),
        }],
    );
    let mint = h.create_mint_ext("member-immutable", &plan).unwrap();
    assert_eq!(
        h.mint_group_member_pointer(&mint).unwrap(),
        pv(None, Some(mint))
    );

    let new_target = derive_address("new-member");
    let update_ix = h
        .update_group_member_pointer_ix(&mint, &payer, Some(new_target))
        .unwrap();
    let rep = h.execute_logged(&update_ix);

    assert_eq!(
        code(&rep),
        Some(NO_AUTHORITY),
        "immutable member pointer must fail with NoAuthorityExists: {rep:?}"
    );
    assert_eq!(
        h.mint_group_member_pointer(&mint).unwrap(),
        pv(None, Some(mint))
    );
}

#[test]
fn multiple_pointers_coexist_on_same_mint() {
    use spl_token_2022_interface::extension::ExtensionType;

    let h = tokenscope_executor::Harness::new();
    let payer = h.payer();
    let mint_addr = derive_address("mint/all-pointers");
    let group_addr = derive_address("shared-group");
    let member_addr = derive_address("shared-member");

    let plan = MintPlan::new(
        6,
        vec![
            MintExt::MetadataPointer {
                authority: Some(payer),
                metadata_address: Some(mint_addr),
            },
            MintExt::GroupPointer {
                authority: Some(payer),
                group_address: Some(group_addr),
            },
            MintExt::GroupMemberPointer {
                authority: Some(payer),
                member_address: Some(member_addr),
            },
        ],
    );
    let mint = h.create_mint_ext("all-pointers", &plan).unwrap();

    // Verify all three extension types are present.
    let ext_types = h.mint_extension_types(&mint).unwrap();
    assert!(ext_types.contains(&ExtensionType::MetadataPointer));
    assert!(ext_types.contains(&ExtensionType::GroupPointer));
    assert!(ext_types.contains(&ExtensionType::GroupMemberPointer));

    // Verify each pointer decodes its own independent state.
    assert_eq!(
        h.mint_metadata_pointer(&mint).unwrap(),
        pv(Some(payer), Some(mint))
    );
    assert_eq!(
        h.mint_group_pointer(&mint).unwrap(),
        pv(Some(payer), Some(group_addr))
    );
    assert_eq!(
        h.mint_group_member_pointer(&mint).unwrap(),
        pv(Some(payer), Some(member_addr))
    );

    // Updating one pointer does not affect the others.
    let updated_meta = derive_address("meta-standalone");
    let update_ix = h
        .update_metadata_pointer_ix(&mint, &payer, Some(updated_meta))
        .unwrap();
    let rep = h.execute_logged(&update_ix);
    assert!(rep.error.is_none());

    assert_eq!(
        h.mint_metadata_pointer(&mint).unwrap(),
        pv(Some(payer), Some(updated_meta))
    );
    assert_eq!(
        h.mint_group_pointer(&mint).unwrap(),
        pv(Some(payer), Some(group_addr)),
        "group pointer must remain unchanged"
    );
    assert_eq!(
        h.mint_group_member_pointer(&mint).unwrap(),
        pv(Some(payer), Some(member_addr)),
        "member pointer must remain unchanged"
    );
}
