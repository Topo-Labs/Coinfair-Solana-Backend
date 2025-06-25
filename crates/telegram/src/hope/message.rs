// pub fn message_coinfair_home() -> String {
//     format!("💵 Focus on Monitor & Swap trading on Coinfair. 🤑")
// }

// pub fn message_chain(chain: Option<&str>) -> String {
//     match chain {
//         Some(chain) => format!("🔗 Chain: {chain}"),
//         None => format!("🔗 Choose Chain"),
//     }
// }

// pub fn message_subscribe(subs: &str) -> String {
//     format!("🔔 Your Subscriptions:\n {subs}")
// }

// pub fn message_subscribe_ok(
//     chain_id: u32,
//     token_address: String,
//     target_address: String,
// ) -> String {
//     format!("🔔 New Subscription ✅ \n⛓️ chain_id: {chain_id}\n🪙 token_address: {token_address}\n💵 target_address: {target_address}\n")
// }

// // 🪙 CA: TVZY1MhwneB7onfF1FNCVSBDGAp2AexZYs
// // 📈 Buy Link (https://t.me/Tronsnipebot?start=ca_TVZY1MhwneB7onfF1FNCVSBDGAp2AexZYs)
// // 🏷 Name: Baby Whale
// // 💲 Symbol: BabyWhale
// // 💰 Total Supply: 1,000,000,000
// //

// // #️ 0
// // ⛓️  chain_id: 31337,
// // 🪙 token_address: 0x610178da211fef7d417bc0e6fed39f05609ad788,
// // 💵 target_address: 0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266
// // ================
// // #️ 1
// // ⛓️  chain_id: 65,
// // 🪙 token_address: 0x610178da211fef7d417bc0e6fed39f05609ad789,
// // 💵 target_address: 0xf39fd6e51aad88f6f4ce6ab8827279cfffb94455
// //

// // [(0, Subscription { chain_id: 31337, token_address: 0x610178da211fef7d417bc0e6fed39f05609ad788, target_address: 0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266 })]

// // HOPE: X, Telegram, DEX, Explorer(HOPE)，🕊， HOPE地址，Pair地址，Website,
// pub fn message_token_info(symbol: String, ca: String, holders: u32, price: f64) -> String {
//     format!(
//         r#"
// ${symbol}: {ca}

// 🔔 Audit: NoMint ✅ / Blacklist ✅ / Burnt 100%✅
// ✅ Top 10: 0%
// 🐀 Insiders: --
// ❌ Liq: $0.5875 (0.00123 SOL)
// 💊 Pump: 6.85%(38D)
// 🐦 [Twitter](https://twitter.com/sluulycoin) | 🌏 [Website](https://sluuly.dev-web.cyou/) | ✈️ [Telegram](https://t.me/sluulycoin)

// Price $0.0{5}66897    MC $6689.7178    Price Chart (https://gmgn.ai/sol/token/6Sq2FdAdei6JTmj8Ux7uQUogWnYfxU5JEX5WW8tapump?utm_source=telegram&utm_campaign=tg_cmdbot_hot)

// 💎 Holding 0 SOL ($0)
// | Token 0 $SLUULY
// | XXX -- 🚀
// | Avg Cost $-- (MC: $--)
// | Bought -- SOL
// | Sold -- SOL
// 💳 Balance 0 SOL

// ---------------------
// ⛽️ Suggest Tip: High 0.0043 SOL | Very High 0.0066 SOL
// ⚙️ Buy 0.008 SOL | Sell 0.008 SOL (Tap /set to set)
// ⚠️ Safety Tip: Don't click scam ads at the top of Telegram to prevent your wallet from being drained(Learn more (https://docs.gmgn.ai/index/safety-tip))

//             🔔 New Subscription ✅ \n⛓️ chain_id: {chain_id}\n🪙 token_address: {token_address}\n💵 target_address: {target_address}\n"#
//     )
// }
