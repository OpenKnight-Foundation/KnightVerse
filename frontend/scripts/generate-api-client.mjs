import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const specPath = resolve(here, "../../openapi.json");
const outPath = resolve(here, "../lib/api/generated.ts");
const methods = ["get", "post", "put", "patch", "delete"];

const spec = JSON.parse(await readFile(specPath, "utf8"));
const schemas = spec.components?.schemas ?? {};

function refName(ref) {
  return ref.split("/").pop();
}

function tsType(schema, depth = 0) {
  if (!schema) return "unknown";
  const base = (() => {
    if (schema.$ref) return refName(schema.$ref);
    if (schema.enum) return schema.enum.map((value) => JSON.stringify(value)).join(" | ");
    if (schema.oneOf) return schema.oneOf.map((item) => tsType(item, depth)).join(" | ");
    if (schema.anyOf) return schema.anyOf.map((item) => tsType(item, depth)).join(" | ");
    if (schema.allOf) return schema.allOf.map((item) => tsType(item, depth)).join(" & ");
    if (schema.type === "object") {
      const properties = schema.properties ?? {};
      const required = new Set(schema.required ?? []);
      const keys = Object.keys(properties);
      if (keys.length === 0) return "Record<string, unknown>";
      const pad = "  ".repeat(depth + 1);
      const lines = keys.map((key) => {
        const optional = required.has(key) ? "" : "?";
        return `${pad}${JSON.stringify(key)}${optional}: ${tsType(properties[key], depth + 1)};`;
      });
      return `{\n${lines.join("\n")}\n${"  ".repeat(depth)}}`;
    }
    if (schema.type === "array") return `Array<${tsType(schema.items, depth)}>`;
    if (schema.type === "string") return "string";
    if (schema.type === "number" || schema.type === "integer") return "number";
    if (schema.type === "boolean") return "boolean";
    if (schema.type === "null") return "null";
    return "unknown";
  })();
  return schema.nullable ? `${base} | null` : base;
}

const operations = [];
for (const [path, item] of Object.entries(spec.paths ?? {})) {
  for (const method of methods) {
    const operation = item[method];
    if (!operation?.operationId) continue;
    operations.push({
      path,
      method: method.toUpperCase(),
      id: operation.operationId,
      parameters: (operation.parameters ?? []).filter((param) => param.in === "path"),
      body: operation.requestBody?.content?.["application/json"]?.schema,
      response: Object.entries(operation.responses ?? {}).find(([status]) => status.startsWith("2"))?.[1]
        ?.content?.["application/json"]?.schema,
    });
  }
}

const methodNames = operations.map((operation) => operation.id);
if (new Set(methodNames).size !== methodNames.length) {
  throw new Error("Duplicate operationId values in openapi.json");
}

const schemaNames = new Set(Object.keys(schemas));
const responseTypeNames = new Map(
  operations.map((operation) => {
    const base = operation.id[0].toUpperCase() + operation.id.slice(1);
    const candidate = `${base}Response`;
    return [operation.id, schemaNames.has(candidate) ? `${base}OperationResponse` : candidate];
  }),
);

const lines = [
  `import { API_BASE, fetchWithRetry, type RetryOptions } from "../api";`,
  ``,
  `export interface ApiCallOptions {`,
  `  headers?: HeadersInit;`,
  `  signal?: AbortSignal;`,
  `  credentials?: RequestCredentials;`,
  `  token?: string | null;`,
  `  retryOptions?: RetryOptions;`,
  `}`,
  ``,
  `export class ApiError extends Error {`,
  `  readonly status: number;`,
  `  constructor(status: number, message: string) {`,
  `    super(message);`,
  `    this.name = "ApiError";`,
  `    this.status = status;`,
  `  }`,
  `}`,
  ``,
];

for (const [name, schema] of Object.entries(schemas)) {
  lines.push(`export type ${name} = ${tsType(schema)};`, ``);
}

lines.push(
  `export const apiPaths = {`,
  ...Object.keys(spec.paths ?? {}).map((path) => `  ${JSON.stringify(path)}: ${JSON.stringify(path)},`),
  `} as const;`,
  ``,
  `export type ApiPath = keyof typeof apiPaths;`,
  ``,
  `export interface ApiOperationMap {`,
);

const operationTypes = [];
for (const operation of operations) {
  const typeName = operation.id[0].toUpperCase() + operation.id.slice(1);
  const fields = [
    ...operation.parameters.map((param) => `${param.name}: ${tsType(param.schema ?? { type: "string" })}`),
    ...(operation.body ? [`body: ${tsType(operation.body)}`] : []),
  ];
  const input = fields.length > 0 ? `ApiCallOptions & { ${fields.join("; ")} }` : "ApiCallOptions";
  lines.push(`  ${JSON.stringify(`${operation.method} ${operation.path}`)}: {`);
  lines.push(`    input: ${input};`);
  lines.push(`    response: ${operation.response ? tsType(operation.response) : "void"};`);
  lines.push(`  };`);
  operationTypes.push(
    `export type ${typeName}Input = ${input};`,
    `export type ${responseTypeNames.get(operation.id)} = ${operation.response ? tsType(operation.response) : "void"};`,
    ``,
  );
}

lines.push(`}`, ``, ...operationTypes);
lines.push(`export type ApiOperationKey = keyof ApiOperationMap;`, ``);
lines.push(`export interface ApiClient {`);
for (const operation of operations) {
  const typeName = operation.id[0].toUpperCase() + operation.id.slice(1);
  const optional = operation.parameters.length === 0 && !operation.body ? "?" : "";
  lines.push(`  ${operation.id}(input${optional}: ${typeName}Input): Promise<${responseTypeNames.get(operation.id)}>;`);
}
lines.push(`}`, ``);

lines.push(
  `async function request<K extends ApiOperationKey>(`,
  `  baseUrl: string,`,
  `  path: string,`,
  `  method: string,`,
  `  input: ApiOperationMap[K]["input"],`,
  `): Promise<ApiOperationMap[K]["response"]> {`,
  `  const options = input as ApiCallOptions & { body?: unknown };`,
  `  const url = path.replace(/\\{([^}]+)\\}/g, (_, key: string) =>`,
  `    encodeURIComponent(String((options as Record<string, unknown>)[key])),`,
  `  );`,
  `  const headers = new Headers(options.headers);`,
  `  if (options.body !== undefined && !headers.has("Content-Type")) {`,
  `    headers.set("Content-Type", "application/json");`,
  `  }`,
  `  if (options.token && !headers.has("Authorization")) {`,
  `    headers.set("Authorization", \`Bearer \${options.token}\`);`,
  `  }`,
  `  const response = await fetchWithRetry(`,
  `    \`\${baseUrl}\${url}\`,`,
  `    {`,
  `      method,`,
  `      headers,`,
  `      body: options.body === undefined ? undefined : JSON.stringify(options.body),`,
  `      credentials: options.credentials,`,
  `      signal: options.signal,`,
  `    },`,
  `    options.retryOptions,`,
  `  );`,
  `  if (!response.ok) {`,
  `    let message = \`Request failed (\${response.status})\`;`,
  `    try {`,
  `      const body: unknown = await response.json();`,
  `      if (body && typeof body === "object" && typeof (body as { message?: unknown }).message === "string") {`,
  `        message = (body as { message: string }).message;`,
  `      }`,
  `    } catch {`,
  `    }`,
  `    throw new ApiError(response.status, message);`,
  `  }`,
  `  if (response.status === 204) return undefined as ApiOperationMap[K]["response"];`,
  `  try {`,
  `    return (await response.json()) as ApiOperationMap[K]["response"];`,
  `  } catch {`,
  `    return undefined as ApiOperationMap[K]["response"];`,
  `  }`,
  `}`,
  ``,
  `export interface ApiClientOptions {`,
  `  baseUrl?: string;`,
  `}`,
  ``,
  `export function createApiClient(options: ApiClientOptions = {}): ApiClient {`,
  `  const baseUrl = (options.baseUrl ?? API_BASE).replace(/\\/$/, "");`,
  `  return {`,
);

for (const operation of operations) {
  const key = `${operation.method} ${operation.path}`;
  const optional = operation.parameters.length === 0 && !operation.body ? " = {}" : "";
  lines.push(
    `    ${operation.id}: (input: ApiOperationMap[${JSON.stringify(key)}]["input"]${optional}) =>`,
    `      request<${JSON.stringify(key)}>(baseUrl, ${JSON.stringify(operation.path)}, ${JSON.stringify(operation.method)}, input),`,
  );
}

lines.push(`  };`, `}`, ``, `export const apiClient = createApiClient();`, ``);

await mkdir(dirname(outPath), { recursive: true });
await writeFile(outPath, lines.join("\n"), "utf8");
process.stdout.write(`Generated ${outPath} from ${specPath} (${operations.length} operations)\n`);
