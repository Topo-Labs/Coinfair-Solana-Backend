// src/compute/dto/swap-query.dto.ts
import { IsString, IsNotEmpty } from 'class-validator';

export class SwapQueryDto {
  @IsString()
  @IsNotEmpty()
  inputMint: string;

  @IsString()
  @IsNotEmpty()
  outputMint: string;

  @IsString()
  @IsNotEmpty()
  amount: string;

  @IsString()
  @IsNotEmpty()
  slippageBps: string;

  @IsString()
  @IsNotEmpty()
  txVersion: string;
}