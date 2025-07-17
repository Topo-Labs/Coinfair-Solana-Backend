
// pool状态查询
export interface RewardInfo {
	rewardState: number;
	openTime: string;
	endTime: string;
	lastUpdateTime: string;
	emissionsPerSecondX64: string;
	rewardTotalEmissioned: string;
	rewardClaimed: string;
	tokenMint: string;
	tokenVault: string;
	authority: string;
	rewardGrowthGlobalX64: string;
}

export interface PoolState {
	bump: number[];
	ammConfig: string;
	owner: string;
	tokenMint0: string;
	tokenMint1: string;
	tokenVault0: string;
	tokenVault1: string;
	observationKey: string;
	mintDecimals0: number;
	mintDecimals1: number;
	tickSpacing: number;
	liquidity: string;
	sqrtPriceX64: string;
	tickCurrent: number;
	padding3: number;
	padding4: number;
	feeGrowthGlobal0X64: string;
	feeGrowthGlobal1X64: string;
	protocolFeesToken0: string;
	protocolFeesToken1: string;
	swapInAmountToken0: string;
	swapOutAmountToken1: string;
	swapInAmountToken1: string;
	swapOutAmountToken0: string;
	status: number;
	padding: number[];
	rewardInfos: RewardInfo[];
	tickArrayBitmap: string[];
	totalFeesToken0: string;
	totalFeesClaimedToken0: string;
	totalFeesToken1: string;
	totalFeesClaimedToken1: string;
	fundFeesToken0: string;
	fundFeesToken1: string;
	openTime: string;
	recentEpoch: string;
	padding1: string[];
	padding2: string[];
}

// 配置文件查询
export interface AmmConfig {
	bump: number;
	index: number;
	owner: string;
	protocolFeeRate: number;
	tradeFeeRate: number;
	tickSpacing: number;
	fundFeeRate: number;
	paddingU32: number;
	fundOwner: string;
	padding: string[];
}


// 计算返回结果
export interface RoutePlan {
	poolId: string;
	inputMint: string;
	outputMint: string;
	feeMint: string;
	feeRate: string;
	feeAmount: string;
	remainingAccounts: any[];
	lastPoolPriceX64: string;
}

export interface SwapResponse {
	currentPrice: string;
	nextPrice: string;
	swapType: string;
	inputMint: string;
	inputAmount: string;
	outputMint: string;
	outputAmount: string;
	otherAmountThreshold: string;
	slippageBps: string;
	priceImpactPct: string;
	referrerAmount: string;
	routePlan: RoutePlan[];
}