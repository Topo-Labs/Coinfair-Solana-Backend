import { Injectable, Logger } from '@nestjs/common';
import { SolanaService } from '../shared/solana.service';
import { TransactionDto } from './transaction.dto';
import { SolanaFactory } from 'src/utils/solanaFactory';

@Injectable()
export class TransactionService {

  constructor() {}

  async swapBaseIn(request: TransactionDto) {
    const res = await SolanaFactory.generateSwapTransaction(request, 'BaseIn')
    return [
      {
        "transaction": res
      }
    ]
  }

  
  async swapBaseOut(request: TransactionDto) {
    const res = await SolanaFactory.generateSwapTransaction(request, 'BaseOut')
    return [
      {
        "transaction": res
      }
    ]
  }

}
