export interface ContractMethodInvoker {
  (
    method: string,
    args?: unknown[],
    options?: { simulateOnly?: boolean },
  ): Promise<unknown>;
}

export interface TitleBadge {
  title: string;
  granted_at: bigint;
  active: boolean;
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === "object" && value !== null
    ? (value as Record<string, unknown>)
    : null;
}

function unwrap(value: unknown): unknown {
  const record = asRecord(value);
  return record && "_value" in record ? record._value : value;
}

function toBigInt(value: unknown): bigint {
  if (typeof value === "bigint") return value;
  if (typeof value === "number") return BigInt(Math.trunc(value));
  if (typeof value === "string" && value.trim()) return BigInt(value);
  if (typeof value === "object" && value !== null && "_value" in value) {
    return toBigInt((value as { _value: unknown })._value);
  }
  throw new TypeError("Invalid bigint value");
}

function toBoolean(value: unknown): boolean {
  const unwrapped = unwrap(value);
  if (typeof unwrapped === "boolean") return unwrapped;
  if (typeof unwrapped === "bigint") return unwrapped !== 0n;
  if (typeof unwrapped === "number") return unwrapped !== 0;
  if (typeof unwrapped === "string") return unwrapped.toLowerCase() === "true";
  return false;
}

export function normalizeTitleBadge(value: unknown): TitleBadge | null {
  if (!value) return null;

  const raw = Array.isArray(value)
    ? { title: value[0], granted_at: value[1], active: value[2] }
    : asRecord(value);
  if (!raw) return null;

  const title = unwrap(raw.title ?? raw.title_symbol);
  if (typeof title !== "string" || !title.trim()) return null;
  if (!("active" in raw) || !toBoolean(raw.active)) return null;

  let grantedAt = 0n;
  try {
    grantedAt = toBigInt(raw.granted_at);
  } catch {
    grantedAt = 0n;
  }

  return {
    title: title.trim(),
    granted_at: grantedAt,
    active: true,
  };
}

export class TitleBadgeClient {
  constructor(private readonly invoke: ContractMethodInvoker) {}

  async get(player: string): Promise<TitleBadge | null> {
    const result = await this.invoke("get", [player], { simulateOnly: true });
    return normalizeTitleBadge(result);
  }

  async isVerified(player: string): Promise<boolean> {
    const result = await this.invoke("is_verified", [player], {
      simulateOnly: true,
    });
    const record = asRecord(result);
    return toBoolean(record?._value ?? result);
  }
}
