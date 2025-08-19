/// 转账费信息结构体
#[derive(Debug, Clone)]
pub struct TransferFeeInfo {
    pub input_transfer_fee: u64,
    pub output_transfer_fee: u64,
    pub input_mint_decimals: u8,
    pub output_mint_decimals: u8,
}
