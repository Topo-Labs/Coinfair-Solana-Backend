import {  Connection, PublicKey,} from '@solana/web3.js';
import { BN } from '@project-serum/anchor';
import { MY_WALLET_ADDRESS, POOL_PROGRAM_ID } from 'src/config/program.config';
import { getAssociatedTokenAddress, TOKEN_2022_PROGRAM_ID } from '@solana/spl-token';
import {  CLMM_PROGRAM_ID, findProgramAddress, MAX_SQRT_PRICE_X64, MIN_SQRT_PRICE_X64, SqrtPriceMath } from '@raydium-io/raydium-sdk-v2';
import { PoolState } from 'src/types/solana.types';

// 获取token1的ata账户
export async function getTokenAta(token: string, walletAddress: string) {
    const ata = await getAssociatedTokenAddress(new PublicKey(token), new PublicKey(walletAddress),true, TOKEN_2022_PROGRAM_ID);
    return ata;
}

// token排序
export function sortMint(inputMint: string, outputMint: string):[string, string] {
    const [mint0, mint1] = new PublicKey(inputMint).toBase58() < new PublicKey(outputMint).toBase58() ? [inputMint, outputMint] : [outputMint, inputMint]
    return [mint0, mint1]
}

// 计算价格限制
export function calculateSqrtPriceLimitX64(
    sqrtPriceX64: BN,
    isToken0ToToken1: boolean,
    slippage: number = 0.05 // 默认 5% 滑点
  ): BN {
    // 基于滑点调整价格限制
    const slippageFactor = new BN((1 + (isToken0ToToken1 ? slippage : -slippage)) * 10000);
    let sqrtPriceLimitX64 = sqrtPriceX64.mul(slippageFactor).div(new BN(10000));
  
    // 验证 sqrtPriceLimitX64 是否满足链上约束
    if (isToken0ToToken1) {
      // token0 → token1: 需低于当前价格，高于最小价格
      if (sqrtPriceLimitX64.gte(sqrtPriceX64) || sqrtPriceLimitX64.lte(MIN_SQRT_PRICE_X64)) {
        // 调整到合理值，例如当前价格 - 1%
        sqrtPriceLimitX64 = sqrtPriceX64.mul(new BN(99)).div(new BN(100));
      }
    } else {
      // token1 → token0: 需高于当前价格，低于最大价格
      if (sqrtPriceLimitX64.lte(sqrtPriceX64) || sqrtPriceLimitX64.gte(MAX_SQRT_PRICE_X64)) {
        // 调整到合理值，例如当前价格 + 1%
        sqrtPriceLimitX64 = sqrtPriceX64.mul(new BN(101)).div(new BN(100));
      }
    }

    console.log('sqrtPriceLimitX64==>',sqrtPriceLimitX64);
  
    return sqrtPriceLimitX64;
  }

  interface AccountMeta {
    pubkey: PublicKey;
    isSigner: boolean;
    isWritable: boolean;
  }
  
  async function getRemainingAccounts(
    poolId: string,
    tickCurrent: number,
    targetSqrtPriceX64: BN,
    tickArrayBitmap: string[],
    zeroForOne: boolean,
    programId: string,
    tickSpacing: number,
    ticksPerArray: number = 512,
    maxArrays: number = 20
  ): Promise<AccountMeta[]> {
    const remainingAccounts: AccountMeta[] = [];
    const poolIdPubkey = new PublicKey(poolId);
  
    // 计算当前和目标 tickArray 索引
    const currentArrayIndex = Math.floor(tickCurrent / (tickSpacing * ticksPerArray));
    const targetTick = Math.round(
      Math.log(
        targetSqrtPriceX64.div(new BN(2).pow(new BN(64))).pow(new BN(2)).toNumber()
      ) / Math.log(1.0001)
    );
    const targetArrayIndex = Math.floor(targetTick / (tickSpacing * ticksPerArray));
    console.log('currentArrayIndex==>',currentArrayIndex);
    console.log('targetArrayIndex==>',targetArrayIndex);
    // 确定遍历范围
    const startIndex = zeroForOne ? currentArrayIndex : currentArrayIndex;
    const endIndex = zeroForOne
      ? Math.max(currentArrayIndex - maxArrays, targetArrayIndex)
      : Math.min(currentArrayIndex + maxArrays, targetArrayIndex);
  
    // 遍历 tickArray 索引
    for (let i = startIndex; zeroForOne ? i >= endIndex : i <= endIndex; i += zeroForOne ? -1 : 1) {
      const arrayIndex = i;
      const bitmapIndex = Math.floor(arrayIndex / 64);
      const bitPosition = arrayIndex % 64;
  
      // 检查 tickArray 是否初始化
      const bitmapValue = BigInt(`0x${tickArrayBitmap[bitmapIndex] || '0'}`);
      if (bitmapValue & (BigInt(1) << BigInt(bitPosition))) {
        const startTick = arrayIndex * tickSpacing * ticksPerArray;
        const {publicKey: tickArrayAddress} = await findProgramAddress(
          [Buffer.from('tick_array'), poolIdPubkey.toBuffer(), Buffer.from(startTick.toString())],
          new PublicKey(programId)
        );
        remainingAccounts.push({
          pubkey: tickArrayAddress,
          isSigner: false,
          isWritable: true,
        });
      } else {
        console.warn(`跳过未初始化的 tickArray，索引 ${arrayIndex}`);
      }
    }
  
    console.log('Remaining Accounts:', remainingAccounts.map(a => a.pubkey.toBase58()));
    return zeroForOne ? remainingAccounts.reverse() : remainingAccounts;
  }
  
  export async function calculateRemainingAccountsForSwap(
    poolState: PoolState,
    token1Mint: string,
    amountIn: string,
    programId: string
  ): Promise<AccountMeta[]> {
    const {
      tickCurrent,
      sqrtPriceX64,
      liquidity,
      tickSpacing,
      tickArrayBitmap,
      tokenMint0
    } = poolState;
  
    // 确定 zeroForOne
    const zeroForOne = token1Mint === tokenMint0;
    console.log('zeroForOne==>',zeroForOne);
  
    // 预测目标 sqrtPriceX64
    const currentSqrtPriceX64 = new BN(sqrtPriceX64, 16);
    const nextSqrtPriceX64 = SqrtPriceMath.getNextSqrtPriceX64FromInput(
      currentSqrtPriceX64,
      new BN(liquidity, 16),
      new BN(amountIn),
      zeroForOne
    );
  
    // 设置 sqrt_price_limit（±5% 滑点）
    const slippage = 0.05;
    const sqrtPriceLimitX64 = nextSqrtPriceX64.mul(new BN(zeroForOne ? 95 : 105)).div(new BN(100));
  
    // 验证 sqrt_price_limit
    if (zeroForOne && sqrtPriceLimitX64.gte(currentSqrtPriceX64)) {
      throw new Error('sqrt_price_limit 必须小于当前价格（zeroForOne = true）');
    }
    if (!zeroForOne && sqrtPriceLimitX64.lte(currentSqrtPriceX64)) {
      throw new Error('sqrt_price_limit 必须大于当前价格（zeroForOne = false）');
    }
  
    // 获取 remainingAccounts
    return getRemainingAccounts(
      POOL_PROGRAM_ID,
      tickCurrent,
      sqrtPriceLimitX64,
      tickArrayBitmap,
      zeroForOne,
      programId,
      tickSpacing
    );
  }
  