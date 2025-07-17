import { Injectable, Logger } from '@nestjs/common';
import { PublicKey } from '@solana/web3.js';
import { getPdaAmmConfigId, getPdaPoolId, SqrtPriceMath } from '@raydium-io/raydium-sdk-v2';
import { CLMM_PROGRAM_ID } from './program.config';
import { AmmConfig, PoolState } from './solana.types';
import { BN } from '@project-serum/anchor';
import Decimal from 'decimal.js';
import { SolanaFactory } from './solanaFactory';
import { SwapQueryDto } from './compute.dto';
import { sortMint } from './swap.utils';

@Injectable()
export class ComputeService {

  constructor() { }
  private readonly clmmProgram = SolanaFactory.getProgram();

  async swapBaseInOut(params: SwapQueryDto, swapType: 'BaseIn' | 'BaseOut') {
    const { inputMint, amount, outputMint, slippageBps, txVersion } = params;
    console.log({ inputMint, amount, outputMint, slippageBps, txVersion })

    const [mint0, mint1] = sortMint(inputMint, outputMint);
    const configProgramId = await getPdaAmmConfigId(new PublicKey(CLMM_PROGRAM_ID), 0);

    const poolId = await getPdaPoolId(
      new PublicKey(CLMM_PROGRAM_ID),
      new PublicKey(configProgramId.publicKey),
      new PublicKey(mint0),
      new PublicKey(mint1)
    );
    const poolState = await this.clmmProgram.account.poolState.fetch(new PublicKey(poolId.publicKey)) as unknown as PoolState;
    const ammConfig = await this.clmmProgram.account.ammConfig.fetch(poolState.ammConfig) as unknown as AmmConfig;
    const nextSqrtPriceX64 = SqrtPriceMath.getNextSqrtPriceX64FromInput(
      new BN(poolState.sqrtPriceX64),
      new BN(poolState.liquidity),
      new BN(amount),
      true
    );
    // 价格默认是min1/mint0 = price
    const nextPrice = SqrtPriceMath.sqrtPriceX64ToPrice(
      nextSqrtPriceX64,
      poolState.mintDecimals0,
      poolState.mintDecimals1
    );
    const currentPrice = SqrtPriceMath.sqrtPriceX64ToPrice(
      new BN(poolState.sqrtPriceX64),
      poolState.mintDecimals0,
      poolState.mintDecimals1
    );
    let resultAmount;
    if (mint0 == inputMint) {//正算价格
      resultAmount = Decimal.div(new Decimal(amount), currentPrice);
    } else {//反算价格
      resultAmount = Decimal.mul(new Decimal(amount), currentPrice);
    }

    let inputAmount;
    let outputAmount;
    if (swapType == 'BaseIn') {
      inputAmount = amount;
      outputAmount = resultAmount;
    } else {
      inputAmount = resultAmount;
      outputAmount = amount;
    }

    const otherAmountThreshold = Decimal.mul((swapType == 'BaseIn' ? resultAmount : amount), Decimal(10 ** 4).sub(slippageBps)).div(10 ** 4).toString();
    const priceImpactPct = Decimal.sub(currentPrice, nextPrice).div(currentPrice).mul(100).toString();
    const feeRate = Decimal(ammConfig.protocolFeeRate).div(100).toString();
    const feeAmount = Decimal(amount).mul(Decimal(ammConfig.protocolFeeRate)).div(Decimal(10 ** poolState.mintDecimals0)).toString();
    return {
      currentPrice,
      nextPrice,
      swapType,
      inputMint,
      inputAmount,
      outputMint,
      outputAmount,
      otherAmountThreshold,
      slippageBps,
      priceImpactPct,
      referrerAmount: "0",
      routePlan: [
        {
          poolId: poolId.publicKey,
          inputMint,
          outputMint,
          feeMint: inputMint,
          feeRate,
          feeAmount,
          remainingAccounts: [],
          lastPoolPriceX64: poolState.sqrtPriceX64
        }
      ]
    }
  }
}
