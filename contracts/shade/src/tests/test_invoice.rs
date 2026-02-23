#![cfg(test)]

use crate::shade::{Shade, ShadeClient};
use crate::types::InvoiceStatus;
use soroban_sdk::testutils::{Address as _, Events as _};
use soroban_sdk::{Address, Env, Map, String, Symbol, TryIntoVal, Val};

fn setup_test() -> (Env, ShadeClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    let contract_id = env.register(Shade, ());
    let client = ShadeClient::new(&env, &contract_id);
    let admin = Address::generate(&env);
    client.initialize(&admin);
    (env, client, contract_id, admin)
}

fn assert_latest_invoice_event(
    env: &Env,
    contract_id: &Address,
    expected_invoice_id: u64,
    expected_merchant: &Address,
    expected_amount: i128,
    expected_token: &Address,
) {
    let events = env.events().all();
    assert!(events.len() > 0, "No events captured for invoice!");

    let (event_contract_id, _topics, data) = events.get(events.len() - 1).unwrap();
    assert_eq!(&event_contract_id, contract_id);

    let data_map: Map<Symbol, Val> = data.try_into_val(env).unwrap();

    let invoice_id_val = data_map.get(Symbol::new(env, "invoice_id")).unwrap();
    let merchant_val = data_map.get(Symbol::new(env, "merchant")).unwrap();
    let amount_val = data_map.get(Symbol::new(env, "amount")).unwrap();
    let token_val = data_map.get(Symbol::new(env, "token")).unwrap();

    let invoice_id_in_event: u64 = invoice_id_val.try_into_val(env).unwrap();
    let merchant_in_event: Address = merchant_val.try_into_val(env).unwrap();
    let amount_in_event: i128 = amount_val.try_into_val(env).unwrap();
    let token_in_event: Address = token_val.try_into_val(env).unwrap();

    assert_eq!(invoice_id_in_event, expected_invoice_id);
    assert_eq!(merchant_in_event, expected_merchant.clone());
    assert_eq!(amount_in_event, expected_amount);
    assert_eq!(token_in_event, expected_token.clone());
}

#[test]
fn test_create_and_get_invoice_success() {
    let (env, client, contract_id, _admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = Address::generate(&env);
    let description = String::from_str(&env, "Test Invoice");
    let amount: i128 = 1000;

    let invoice_id = client.create_invoice(&merchant, &description, &amount, &token);
    assert_eq!(invoice_id, 1);

    assert_latest_invoice_event(&env, &contract_id, invoice_id, &merchant, amount, &token);

    let invoice = client.get_invoice(&invoice_id);

    assert_eq!(invoice.id, 1);
    assert_eq!(invoice.merchant_id, 1);
    assert_eq!(invoice.amount, amount);
    assert_eq!(invoice.token, token);
    assert_eq!(invoice.description, description);
    assert_eq!(invoice.status, InvoiceStatus::Pending);
}

#[test]
fn test_create_multiple_invoices() {
    let (env, client, _contract_id, _admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token1 = Address::generate(&env);
    let token2 = Address::generate(&env);

    let id1 = client.create_invoice(
        &merchant,
        &String::from_str(&env, "Invoice 1"),
        &1000,
        &token1,
    );
    let id2 = client.create_invoice(
        &merchant,
        &String::from_str(&env, "Invoice 2"),
        &2000,
        &token2,
    );
    let id3 = client.create_invoice(
        &merchant,
        &String::from_str(&env, "Invoice 3"),
        &500,
        &token1,
    );

    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);
}

#[should_panic(expected = "HostError: Error(Contract, #8)")]
#[test]
fn test_get_invoice_not_found() {
    let (_env, client, _contract_id, _admin) = setup_test();
    client.get_invoice(&999);
}

#[should_panic(expected = "HostError: Error(Contract, #1)")]
#[test]
fn test_create_invoice_unregistered_merchant() {
    let (env, client, _contract_id, _admin) = setup_test();

    let unregistered_merchant = Address::generate(&env);
    let token = Address::generate(&env);
    let description = String::from_str(&env, "Test Invoice");
    let amount: i128 = 1000;

    client.create_invoice(&unregistered_merchant, &description, &amount, &token);
}

#[should_panic(expected = "HostError: Error(Contract, #7)")]
#[test]
fn test_create_invoice_invalid_amount() {
    let (env, client, _contract_id, _admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = Address::generate(&env);
    let description = String::from_str(&env, "Test Invoice");
    let amount: i128 = 0;

    client.create_invoice(&merchant, &description, &amount, &token);
}

// Void Invoice Tests

#[test]
fn test_void_invoice_success() {
    let (env, client, _contract_id, _admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = Address::generate(&env);
    let description = String::from_str(&env, "Test Invoice");
    let invoice_id = client.create_invoice(&merchant, &description, &1000, &token);

    // Verify invoice is Pending
    let invoice_before = client.get_invoice(&invoice_id);
    assert_eq!(invoice_before.status, InvoiceStatus::Pending);

    // Void the invoice
    client.void_invoice(&merchant, &invoice_id);

    // Verify invoice is now Cancelled
    let invoice_after = client.get_invoice(&invoice_id);
    assert_eq!(invoice_after.status, InvoiceStatus::Cancelled);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #1)")]
fn test_void_invoice_non_owner() {
    let (env, client, _contract_id, _admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = Address::generate(&env);
    let description = String::from_str(&env, "Test Invoice");
    let invoice_id = client.create_invoice(&merchant, &description, &1000, &token);

    // Try to void with different merchant (should panic with NotAuthorized)
    let other_merchant = Address::generate(&env);
    client.register_merchant(&other_merchant);
    client.void_invoice(&other_merchant, &invoice_id);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #15)")]
fn test_void_invoice_already_paid() {
    let (env, client, _contract_id, admin, token) = setup_test_with_payment();

    // Register merchant
    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    // Create merchant account
    let merchant_account = Address::generate(&env);
    client.set_merchant_account(&merchant, &merchant_account);

    // Create and pay invoice
    let description = String::from_str(&env, "Test Invoice");
    let invoice_id = client.create_invoice(&merchant, &description, &1000, &token);

    let customer = Address::generate(&env);
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    token_client.mint(&customer, &1000);

    client.pay_invoice(&customer, &invoice_id);

    // Try to void paid invoice (should panic with InvalidInvoiceStatus)
    client.void_invoice(&merchant, &invoice_id);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #15)")]
fn test_void_invoice_already_cancelled() {
    let (env, client, _contract_id, _admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    let token = Address::generate(&env);
    let description = String::from_str(&env, "Test Invoice");
    let invoice_id = client.create_invoice(&merchant, &description, &1000, &token);

    // Void the invoice once
    client.void_invoice(&merchant, &invoice_id);

    // Try to void again (should panic with InvalidInvoiceStatus)
    client.void_invoice(&merchant, &invoice_id);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #15)")]
fn test_pay_cancelled_invoice() {
    let (env, client, _contract_id, admin, token) = setup_test_with_payment();

    // Register merchant
    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    // Create merchant account
    let merchant_account = Address::generate(&env);
    client.set_merchant_account(&merchant, &merchant_account);

    // Create invoice
    let description = String::from_str(&env, "Test Invoice");
    let invoice_id = client.create_invoice(&merchant, &description, &1000, &token);

    // Void the invoice
    client.void_invoice(&merchant, &invoice_id);

    // Try to pay cancelled invoice (should panic with InvalidInvoiceStatus)
    let customer = Address::generate(&env);
    let token_client = soroban_sdk::token::StellarAssetClient::new(&env, &token);
    token_client.mint(&customer, &1000);

    client.pay_invoice(&customer, &invoice_id);
}

#[test]
#[should_panic(expected = "HostError: Error(Contract, #8)")]
fn test_void_non_existent_invoice() {
    let (env, client, _contract_id, _admin) = setup_test();

    let merchant = Address::generate(&env);
    client.register_merchant(&merchant);

    // Try to void non-existent invoice (should panic with InvoiceNotFound)
    client.void_invoice(&merchant, &999);
}

fn setup_test_with_payment() -> (Env, ShadeClient<'static>, Address, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();

    let shade_contract_id = env.register(Shade, ());
    let shade_client = ShadeClient::new(&env, &shade_contract_id);

    let admin = Address::generate(&env);
    shade_client.initialize(&admin);

    let token_admin = Address::generate(&env);
    let token = env.register_stellar_asset_contract_v2(token_admin.clone());

    shade_client.add_accepted_token(&admin, &token.address());
    shade_client.set_fee(&admin, &token.address(), &500);

    (env, shade_client, shade_contract_id, admin, token.address())
}
