// src/utils/solana.factory.ts
import { Connection, PublicKey } from '@solana/web3.js';
import { AnchorProvider, BN, Program } from '@project-serum/anchor';
import { CLMM_PROGRAM_ID, MY_WALLET_ADDRESS, POOL_PROGRAM_ID, TOKEN1, TOKEN2 } from '../config/program.config';
import raydiumAmmV3IDL from '../config/IDL/amm_v3.json';
import { clientConfig } from '../config/program.config';
import { PoolState } from 'src/types/solana.types';
import { MAX_SQRT_PRICE_X64, MEMO_PROGRAM_ID, MIN_SQRT_PRICE_X64, SqrtPriceMath } from '@raydium-io/raydium-sdk-v2';
import { calculateRemainingAccountsForSwap, calculateSqrtPriceLimitX64, getTokenAta, sortMint } from './swap.utils';
import { TOKEN_2022_PROGRAM_ID, TOKEN_PROGRAM_ID } from '@solana/spl-token';
import { TransactionDto } from 'src/transaction/transaction.dto';
import { buildTickArrayAccounts } from './buildTickerAddress';
import Decimal from 'decimal.js';

  
export class SolanaFactory {
  private static connection: Connection;
  private static anchorProvider: AnchorProvider;
  private static clmmProgram: Program;

  // 私有构造函数，防止外部实例化
  private constructor() {}
  // 静态初始化
  static {
    SolanaFactory.initialize();
  }


  // 初始化方法
  private static initialize() {
    this.connection = new Connection(clientConfig.http_url, 'confirmed');
    this.anchorProvider = new AnchorProvider(this.connection, null, { commitment: 'confirmed' });
    this.clmmProgram = new Program(raydiumAmmV3IDL as any, new PublicKey(CLMM_PROGRAM_ID), this.anchorProvider);
  }

  // 重置方法（用于测试或需要重新初始化的情况）
  static reset() {
    this.initialize();
  }

  // 获取所有实例
  static getInstances() {
    return {
      connection: this.connection,
      anchorProvider: this.anchorProvider,
      clmmProgram: this.clmmProgram,
    };
  }

  // 获取单个实例的方法
  static getConnection() {
    return this.connection;
  }

  // 获取provider
  static getProvider() {
    return this.anchorProvider;
  }

  // 获取clmm program
  static getProgram() {
    return this.clmmProgram;
  }

    //  获取pool state
    static async getPoolState() {
        return await this.clmmProgram.account.poolState.fetch(new PublicKey(POOL_PROGRAM_ID)) as PoolState;
    }

  // 交易构建
  static async generateSwapTransaction(request: TransactionDto, swapType: 'BaseIn' | 'BaseOut') {
    const {swapResponse:{data:swapResponse},wallet} = request;
    const {inputMint,outputMint,inputAmount,outputAmount} = swapResponse;
    const poolState = await this.clmmProgram.account.poolState.fetch(new PublicKey(POOL_PROGRAM_ID)) as PoolState;
    const mint0ATA = await getTokenAta(poolState.tokenMint0, wallet);
    const mint1ATA = await getTokenAta(poolState.tokenMint1, wallet);

    // 判断from to，需要换算mint位置
    let mintInfos;
    console.log('inputMint===>',inputMint,poolState.tokenMint0.toString());
    if(inputMint===poolState.tokenMint0.toString()){
      console.log('0===>1');
      mintInfos = {
        inputTokenAccount: mint0ATA,//ata账户
        outputTokenAccount: mint1ATA,//ata账户
        inputVault: new PublicKey(poolState.tokenVault0),
        outputVault: new PublicKey(poolState.tokenVault1),
        inputVaultMint: new PublicKey(poolState.tokenMint0),
        outputVaultMint: new PublicKey(poolState.tokenMint1)
      }
    }else{
      console.log('1===>0');
      mintInfos = {
        inputTokenAccount: mint1ATA,//ata账户
        outputTokenAccount: mint0ATA,//ata账户
        inputVault: new PublicKey(poolState.tokenVault1),
        outputVault: new PublicKey(poolState.tokenVault0),
        inputVaultMint: new PublicKey(poolState.tokenMint1),
        outputVaultMint: new PublicKey(poolState.tokenMint0)
      }
    }
    console.log('inputAmount===>',mintInfos);
     try {
         console.log('创建交易指令===>BaseIn:',swapType==='BaseIn');
         const transaction = await this.clmmProgram.methods
         .swapV2(
            new BN(inputAmount),
            new BN(0),
            undefined,
            swapType==='BaseIn'
          )
         .accounts({
             payer: new PublicKey(wallet),
             ammConfig: new PublicKey(poolState.ammConfig),
             poolState: new PublicKey(POOL_PROGRAM_ID),
             observationState: new PublicKey(poolState.observationKey),
             tokenProgram: TOKEN_PROGRAM_ID,
             tokenProgram2022: TOKEN_2022_PROGRAM_ID,
             memoProgram: MEMO_PROGRAM_ID,
             ...mintInfos
         })
         .remainingAccounts([{
            pubkey: new PublicKey('9Z2PpBfmJxR7MPdG2cuCy6JdJ8Yj9qMJGvAtBRuMGZ5L'),
            isSigner: false,
            isWritable: false
         },{
            pubkey: new PublicKey('DLgwxP8SGTrRcssS3qd1tusvWD669r1p3USDo9eqsfTz'),
            isSigner: false,
            isWritable: true
         },{
            pubkey: new PublicKey('E7piHoq4ryUAtq2x9rBqFB5X3ez1upF5Q1HY7vUQSLAM'),
            isSigner: false,
            isWritable: true
         }])
         .transaction();
         let blockhash = (await this.connection.getLatestBlockhash("finalized")).blockhash;
         transaction.recentBlockhash = blockhash;
         transaction.feePayer = new PublicKey(wallet);
         const serialize = transaction.serialize({
             requireAllSignatures: false,
             verifySignatures: false,
         })
         const swapV2Instruction = Buffer.from(serialize).toString("base64")
         return swapV2Instruction
     } catch (error) {
         console.error('Error swapV2Instruction:', error);
         return null
     }
  }

}