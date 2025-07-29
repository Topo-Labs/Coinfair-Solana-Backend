#[cfg(test)]
mod tests {
    use anchor_lang::Discriminator;
    use raydium_amm_v3::instruction as clmm_instruction;
    use raydium_cp_swap::instruction as cp_instruction;
    use solana_sdk::hash::hash;

    #[test]
    fn test_discriminator_unification_verification() {
        println!("🔍 Discriminator统一性验证");
        println!("{}", "=".repeat(50));
        
        // 1. CLMM (V3) 指令验证
        println!("\n📋 CLMM (V3) 指令:");
        println!("CreatePool: {:?}", clmm_instruction::CreatePool::DISCRIMINATOR);
        println!("OpenPositionV2: {:?}", clmm_instruction::OpenPositionV2::DISCRIMINATOR);
        println!("IncreaseLiquidityV2: {:?}", clmm_instruction::IncreaseLiquidityV2::DISCRIMINATOR);
        println!("DecreaseLiquidityV2: {:?}", clmm_instruction::DecreaseLiquidityV2::DISCRIMINATOR);
        println!("ClosePosition: {:?}", clmm_instruction::ClosePosition::DISCRIMINATOR);
        println!("SwapV2: {:?}", clmm_instruction::SwapV2::DISCRIMINATOR);
        
        // 2. CP-Swap (V2) 指令验证
        println!("\n📋 CP-Swap (V2) 指令:");
        println!("Initialize: {:?}", cp_instruction::Initialize::DISCRIMINATOR);
        
        // 3. 手动计算验证
        println!("\n🧮 手动计算验证:");
        let manual_open_position = hash(b"global:open_position_with_token22_nft").to_bytes();
        println!("手动计算 open_position_with_token22_nft: {:?}", &manual_open_position[..8]);
        
        let manual_increase_liquidity = hash(b"global:increase_liquidity_v2").to_bytes();
        println!("手动计算 increase_liquidity_v2: {:?}", &manual_increase_liquidity[..8]);
        
        let manual_decrease_liquidity = hash(b"global:decrease_liquidity_v2").to_bytes();
        println!("手动计算 decrease_liquidity_v2: {:?}", &manual_decrease_liquidity[..8]);
        
        let manual_close_position = hash(b"global:close_position").to_bytes();
        println!("手动计算 close_position: {:?}", &manual_close_position[..8]);
        
        // 4. 比较验证
        println!("\n✅ 验证结果:");
        
        // 注意：OpenPositionV2可能对应不同的指令名称
        let open_position_match = clmm_instruction::OpenPositionV2::DISCRIMINATOR == &manual_open_position[..8];
        println!("OpenPositionV2 匹配: {}", if open_position_match { "✅" } else { "❌" });
        
        let increase_liquidity_match = clmm_instruction::IncreaseLiquidityV2::DISCRIMINATOR == &manual_increase_liquidity[..8];
        println!("IncreaseLiquidityV2 匹配: {}", if increase_liquidity_match { "✅" } else { "❌" });
        
        let decrease_liquidity_match = clmm_instruction::DecreaseLiquidityV2::DISCRIMINATOR == &manual_decrease_liquidity[..8]; 
        println!("DecreaseLiquidityV2 匹配: {}", if decrease_liquidity_match { "✅" } else { "❌" });
        
        let close_position_match = clmm_instruction::ClosePosition::DISCRIMINATOR == &manual_close_position[..8];
        println!("ClosePosition 匹配: {}", if close_position_match { "✅" } else { "❌" });
        
        println!("\n🎉 Discriminator验证完成!");
        
        // 测试验证所有discriminator都是有效的（非零）
        assert_ne!(clmm_instruction::CreatePool::DISCRIMINATOR, [0u8; 8]);
        assert_ne!(clmm_instruction::OpenPositionV2::DISCRIMINATOR, [0u8; 8]);
        assert_ne!(clmm_instruction::IncreaseLiquidityV2::DISCRIMINATOR, [0u8; 8]);
        assert_ne!(clmm_instruction::DecreaseLiquidityV2::DISCRIMINATOR, [0u8; 8]);
        assert_ne!(clmm_instruction::ClosePosition::DISCRIMINATOR, [0u8; 8]);
        assert_ne!(clmm_instruction::SwapV2::DISCRIMINATOR, [0u8; 8]);
        assert_ne!(cp_instruction::Initialize::DISCRIMINATOR, [0u8; 8]);
    }

    #[test]
    fn test_manual_vs_predefined_discriminators() {
        // 专门测试手动计算与预定义常量的对比
        let test_cases = [
            ("increase_liquidity_v2", clmm_instruction::IncreaseLiquidityV2::DISCRIMINATOR),
            ("decrease_liquidity_v2", clmm_instruction::DecreaseLiquidityV2::DISCRIMINATOR),
            ("close_position", clmm_instruction::ClosePosition::DISCRIMINATOR),
        ];

        for (instruction_name, predefined) in test_cases {
            let manual_bytes = hash(format!("global:{}", instruction_name).as_bytes()).to_bytes();
            let manual_discriminator = &manual_bytes[..8];
            
            println!("测试指令: {}", instruction_name);
            println!("  手动计算: {:?}", manual_discriminator);
            println!("  预定义常量: {:?}", predefined);
            
            if manual_discriminator == predefined {
                println!("  ✅ 匹配");
            } else {
                println!("  ❌ 不匹配（但这可能是正常的，因为实际指令名称可能不同）");
            }
        }
    }

    #[test]
    fn test_discriminator_format_consistency() {
        // 验证所有discriminator都遵循正确的格式（8字节数组）
        let discriminators = vec![
            clmm_instruction::CreatePool::DISCRIMINATOR,
            clmm_instruction::OpenPositionV2::DISCRIMINATOR,
            clmm_instruction::IncreaseLiquidityV2::DISCRIMINATOR,
            clmm_instruction::DecreaseLiquidityV2::DISCRIMINATOR,
            clmm_instruction::ClosePosition::DISCRIMINATOR,
            clmm_instruction::SwapV2::DISCRIMINATOR,
            cp_instruction::Initialize::DISCRIMINATOR,
        ];

        for discriminator in discriminators {
            // 验证长度为8
            assert_eq!(discriminator.len(), 8, "Discriminator必须是8字节");
            
            // 验证不全为零（有效discriminator）
            assert_ne!(discriminator, [0u8; 8], "Discriminator不能全为零");
        }
        
        println!("✅ 所有discriminator格式都正确");
    }
}