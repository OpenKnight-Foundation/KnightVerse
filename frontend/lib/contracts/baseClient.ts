/**
 * Base Typed Contract Client
 *
 * Provides core abstraction for typed contract clients.
 * Wraps simulation, signing, submission, and polling through a shared invoker.
 */

import { ContractInvoker, InvokeContractOptions, InvocationResult } from "./types";

export abstract class TypedContractClient {
  constructor(
    public readonly contractId: string,
    protected readonly invoker: ContractInvoker,
  ) {}

  /**
   * Invokes a mutating contract function: simulates, signs with Freighter,
   * submits to Soroban RPC, and polls for confirmation.
   */
  protected async invoke<T = unknown>(
    method: string,
    args: unknown[] = [],
    options?: InvokeContractOptions,
  ): Promise<InvocationResult<T>> {
    const res = await this.invoker(this.contractId, method, args, {
      ...options,
      simulateOnly: false,
    });
    return res as InvocationResult<T>;
  }

  /**
   * Simulates a read-only contract query: does not request Freighter signature
   * or submit to ledger, simply simulates and returns the parsed return value.
   */
  protected async query<T = unknown>(
    method: string,
    args: unknown[] = [],
    options?: Omit<InvokeContractOptions, "simulateOnly">,
  ): Promise<T> {
    const res = await this.invoker(this.contractId, method, args, {
      ...options,
      simulateOnly: true,
    });
    if (res && typeof res === "object" && "returnValue" in res) {
      return (res as InvocationResult<T>).returnValue;
    }
    return res as T;
  }
}
