/**
 * AI Companion NFT Typed Contract Client
 *
 * Interacts with AI NFT Soroban contract (minting, transfers, metadata, burning).
 */

import { TypedContractClient } from "./baseClient";
import { InvokeContractOptions, InvocationResult, AINFTMetadata } from "./types";

export class AiNftContractClient extends TypedContractClient {
  /**
   * Mints a new AI Companion NFT.
   */
  async mint(
    to: string,
    uri: string,
    name: string,
    symbol: string,
    options?: InvokeContractOptions,
  ): Promise<InvocationResult> {
    return this.invoke("mint", [to, uri, name, symbol], options);
  }

  /**
   * Transfers an AI NFT to a new owner.
   */
  async transfer(
    nftId: number | bigint,
    to: string,
    options?: InvokeContractOptions,
  ): Promise<InvocationResult> {
    return this.invoke("transfer", [BigInt(nftId), to], options);
  }

  /**
   * Permanently retires / burns an AI NFT.
   */
  async burn(
    owner: string,
    nftId: number | bigint,
    options?: InvokeContractOptions,
  ): Promise<InvocationResult> {
    return this.invoke("burn", [owner, BigInt(nftId)], options);
  }

  /**
   * Queries owner of a token (read-only query).
   */
  async ownerOf(nftId: number | bigint): Promise<string> {
    return this.query<string>("owner_of", [BigInt(nftId)]);
  }

  /**
   * Queries metadata for a token (read-only query).
   */
  async metadata(nftId: number | bigint): Promise<AINFTMetadata> {
    return this.query<AINFTMetadata>("metadata", [BigInt(nftId)]);
  }

  /**
   * Queries total supply of minted NFTs (read-only query).
   */
  async totalSupply(): Promise<bigint> {
    return this.query<bigint>("total_supply", []);
  }

  /**
   * Checks if NFT contract minting/transfers are currently paused (read-only query).
   */
  async isPaused(): Promise<boolean> {
    return this.query<boolean>("is_paused", []);
  }

  /**
   * Queries contract admin (read-only query).
   */
  async getAdmin(): Promise<string> {
    return this.query<string>("admin", []);
  }
}
