#[test]
fn test_calculate_ata() {
    use solana_sdk::pubkey::Pubkey;
    use spl_associated_token_account::get_associated_token_address;
    use std::str::FromStr;

    let owner = Pubkey::from_str("CpwzPtCKKmTAb68Dd6hbLtuFVhB3seymQ4P52N9VaNei").unwrap();
    let mint = Pubkey::from_str("So11111111111111111111111111111111111111112").unwrap();

    let ata = get_associated_token_address(&owner, &mint);

    println!("\n========================================");
    println!("Owner地址: {}", owner);
    println!("Mint地址:  {}", mint);
    println!("计算出的ATA地址: {}", ata);
    println!("========================================\n");

    // 验证ATA地址不为空
    assert!(!ata.to_string().is_empty());
}
