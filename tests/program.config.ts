import { PublicKey } from "@solana/web3.js";

// 读取配置
export const clientConfig = {
    http_url: "https://api.devnet.solana.com",
    ws_url: "wss://api.devnet.solana.com",
    payer_path: "/Users/zhaoyu/Desktop/solana/payer.json",
    admin_path: "/Users/zhaoyu/Desktop/solana/admin.json",
    raydium_v3_program: new PublicKey("FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX"),
    slippage: 0.005
};

export const CLMM_PROGRAM_ID = "FA1RJDDXysgwg5Gm3fJXWxt26JQzPkAzhTA114miqNUX";
export const POOL_PROGRAM_ID = "EjiZeDVjSMbZKVQjAVKDZEBd6KAPs9gJCNVDunmxb9fi";
export const MY_WALLET_ADDRESS = "6LUxSdXgc9uenwffgVNQnFhTnyUEJgbiZHozYXER9ETq";
export const TOKEN1 = "5pbcULDGXotRZjJvmoiqj3qYaHJeDYAWpsaT58j6Ao56";
export const TOKEN2 = "CKgtJw9y47qAgxRHBdgjABY7DP4u6bLHXM1G68anWwJm";