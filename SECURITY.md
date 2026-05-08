# Security Policy

Thanks for helping keep XLMate and its on-chain assets safe.

XLMate is still pre-1.0, so our security process is intentionally simple and practical. If you believe you have found a vulnerability in the platform, smart contracts, or supporting infrastructure, please report it privately so we can investigate and fix it responsibly.

## Supported Versions

Because XLMate is still pre-1.0, we currently provide security updates only for the active development branch.

| Version | Supported |
| --- | --- |
| `main` branch | Yes |
| Tagged releases before `1.0.0` | No |
| Unmaintained forks or old snapshots | No |

## Reporting a Vulnerability

Please email security reports to `security@xlmate.ai`.

Do **not** open a public GitHub issue for security bugs, smart contract weaknesses, auth bypasses, or other vulnerabilities that could put users, wallets, or funds at risk.

When you report an issue, please include:

- A clear description of the problem
- The affected component, endpoint, contract, or workflow
- Step-by-step reproduction instructions
- Proof of concept, logs, screenshots, or transactions if available
- Your assessment of impact, especially if funds, authentication, or game integrity are affected
- Any suggested mitigation or patch ideas

What you can expect from us:

- We will acknowledge receipt within 48 hours
- We will triage the report within 7 days
- We may contact you for clarification, test data, or coordinated validation of a fix
- We will keep you informed as the issue moves through investigation, remediation, and disclosure

Please avoid:

- Public disclosure before a fix or mitigation is available
- Accessing funds, private data, or accounts beyond what is necessary to demonstrate the issue
- Running destructive tests against shared infrastructure without prior coordination

## Security Scope

We welcome responsible reports covering the full XLMate stack, including:

- **Smart contracts (Soroban/Rust)**: staking flows, escrow, payouts, game rules, admin controls, and on-chain asset handling
- **Backend API**: authentication, authorization, session handling, refresh token rotation, WebSocket security, and data access controls
- **AI engine**: input validation, sandboxing assumptions, resource limits, model or engine abuse, and denial-of-service vectors
- **Frontend**: XSS, CSRF, unsafe token handling, wallet interaction flaws, and user-facing auth/session issues
- **Infrastructure**: Docker configuration, Redis/PostgreSQL exposure, CI/dependency supply chain risks, and insecure deployment defaults

Reports related to third-party services, local-only misconfiguration with no project impact, or purely theoretical issues without a plausible exploitation path may be considered out of scope.

## Security Measures Already in Place

XLMate already includes several security-focused controls:

- **JWT-based authentication with HS256** for access and reconnect tokens
- **Refresh token rotation** with SHA256 hashing at rest and token family tracking
- **Token theft detection** that invalidates the entire token family on refresh token reuse
- **Emergency circuit breaker smart contract** to pause state-changing on-chain operations during incidents
- **Input validation on chess move parsing** and related game payloads before processing

These controls reduce risk, but they do not replace careful review, testing, and smart contract auditing.

## Vulnerability Classification

We generally classify reports using the following severity levels:

### Critical
Issues that can directly compromise funds, take over accounts, or break core trust assumptions.

Examples:

- Smart contract fund extraction or escrow drain
- Authentication bypass or full account takeover
- Unauthorized minting, payout manipulation, or admin-level contract control

### High
Issues with serious impact that require meaningful but not total compromise.

Examples:

- Refresh token theft with practical account compromise
- Privilege escalation in backend or contract admin flows
- WebSocket or API flaws that let one player act on behalf of another

### Medium
Issues with limited or contained impact.

Examples:

- Information disclosure of non-public user or game data
- Denial-of-service vectors against game services or AI engines
- Vulnerabilities that require unusual preconditions or have partial mitigations in place

### Low
Issues with minor security impact or limited exploitability.

Examples:

- Minor information leaks with little operational value
- Missing hardening that is not currently exploitable
- Best-practice gaps without a realistic attack chain

Severity may be adjusted based on exploitability, affected assets, chain impact, and whether real user funds or credentials are exposed.

## Disclosure Policy

We follow a coordinated disclosure process.

- We ask reporters to allow up to **90 days** for investigation, remediation, and release before public disclosure
- If a fix is ready sooner, we may disclose earlier with the reporter's agreement
- We are happy to credit reporters for confirmed findings if they opt in
- For confirmed vulnerabilities that warrant it, we will evaluate and coordinate **CVE** assignment as part of disclosure
- For smart contract issues affecting deployed on-chain assets, we may prioritize mitigation steps such as pausing contracts, disabling flows, or publishing user guidance before full technical disclosure

## Security Best Practices for Contributors

If you contribute code to XLMate, please keep security in mind:

- Never commit secrets, seed phrases, private keys, or production credentials
- Use parameterized database queries; SeaORM already helps with this by default
- Validate all user input at API boundaries, contract entrypoints, and engine interfaces
- Follow least-privilege principles in smart contracts and admin workflows
- Keep dependencies updated and review changes to blockchain, auth, and networking libraries carefully

If you are unsure whether something is security-sensitive, treat it as such and ask privately.

