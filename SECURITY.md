# Security Policy

1. [Reporting security problems](#reporting-security-problems)
2. [Security Bug Bounties](#security-bug-bounties)
3. [Incident Response Process](#incident-response-process)

## Reporting security problems

**DO NOT CREATE A GITHUB ISSUE** to report a security problem.

Instead please use this [Report a Vulnerability](https://github.com/solana-program/stake-pool/security/advisories/new) link.
Provide a helpful title that includes the affected program name, a detailed
description of the vulnerability, and an exploit proof-of-concept. Speculative
submissions without proof-of-concept will be closed with no further
consideration.

Proof-of-concept requirements:

- All PoC code must be inlined directly in the advisory as source code
- Do not attach or link to compiled binaries, executable files, or external repositories
- The PoC should be self-contained and reproducible by building from the provided source
- Include the output you observed when running the PoC locally (e.g., test validator logs, bankrun/mollusk test output)

AI/LLM reports that have not been manually verified, including reports citing
non-existent code, describing theoretical vulnerabilities without demonstrated
impact, or lacking verifiable execution artifacts, will be closed immediately.
Repeated low-quality submissions may result in permanent disqualification.

If you haven't done so already, please **enable two-factor auth** in your GitHub account.

Expect a response as fast as possible in the advisory, typically within 72 hours.

--

If you do not receive a response in the advisory, send an email to
<security@anza.xyz> with the full URL of the advisory you have created. DO NOT
include attachments or provide detail sufficient for exploitation regarding the
security issue in this email. **Only provide such details in the advisory**.

If you do not receive a response from <security@anza.xyz> please follow up with
the team directly. You can do this in one of the `#Dev Tooling` channels of the
[Solana Tech discord server](https://solana.com/discord), by pinging the admins
in the channel and referencing the fact that you submitted a security problem.

## Incident Response Process

In case an incident is discovered or reported, the following process will be
followed to contain, respond and remediate:

### 1. Accept the new report

In response to a newly reported security problem, a member of the
`solana-program/admins` group will accept the report to turn it into a draft
advisory. The `solana-program/security-incident-response` group should be added
to the draft security advisory, and create a private fork of the repository
(grey button towards the bottom of the page) if necessary.

If the advisory is the result of an audit finding, follow the same process as
above but add the auditor's github user(s) and begin the title with "[Audit]".

If the report is out of scope, a member of the `solana-program/admins` group
will comment as such and then close the report.

### 2. Triage

Within the draft security advisory, discuss and determine the severity of the
issue. If necessary, members of the `solana-program/security-incident-response`
group may add other github users to the advisory to assist. If it is determined
that this is not a critical issue then the advisory should be closed and if more
follow-up is required a normal public GitHub issue should be created.

### 3. Prepare and Deploy Fix

Prepare a fix for the issue and push it to the private repository associated
with the draft security advisory. There is no CI available in the private
repository so you must build from source and manually verify fixes. Code review
from the reporter is ideal, as well as from multiple members of the core
development team. Once the fix is accepted, coordinate deployment through the
program upgrade authority.

### 4. Public Disclosure and Bounty Accounting

Once the fix has been deployed, the patches from the security advisory may be
merged into the main source repository. If this issue is eligible for a bounty,
prefix the title of the security advisory with one of the following, depending
on the severity:

- `[Bounty Category: Loss of Funds]`
- `[Bounty Category: Permanent Account Freeze]`
- `[Bounty Category: State Corruption]`
- `[Bounty Category: Program DoS]`

Confirm with the reporter that they agree with the severity assessment, and
discuss as required to reach a conclusion.

We currently do not use the GitHub workflow to publish security advisories. Once
the issue and fix have been disclosed, and a bounty category is assessed if
appropriate, the GitHub security advisory is no longer needed and can be closed.

### Expected Response Timeline

| Milestone         | Loss of Funds / Freeze | State Corruption | Program DoS |
| ----------------- | ---------------------- | ---------------- | ----------- |
| Acknowledgment    | 72 hours               | 72 hours         | 72 hours    |
| Triage complete   | 3 days                 | 5 days           | 7 days      |
| Fix ready         | 14 days                | 30 days          | 60 days     |
| Public disclosure | ≤90 days               | ≤90 days         | ≤90 days    |

These are targets, not guarantees. We will communicate proactively if a
milestone slips.

## Security Bug Bounties

At its sole discretion, the Solana Foundation may offer a bounty for valid
reports of `spl-stake-pool` vulnerabilities. Please see below for more details.
The submitter is not required to provide a mitigation to qualify.

**IMPORTANT | PLEASE NOTE**

Bug Bounty rewards are denominated in SOL tokens. Payments are paid out in
12-month locked SOL.

**Loss of Funds:**
Max: 10,000 SOL tokens. Min: 1,000 SOL tokens

- Theft or unauthorized transfer of user funds
- Bypassing signature or authority checks that protect funds
- Unauthorized minting or burning of tokens
- Bypassing or seizing the program upgrade authority

**Permanent Account Freeze:**
Max: 5,000 SOL tokens. Min: 500 SOL tokens

- Irrecoverable freeze of user funds requiring program redeployment

**State Corruption:**
Max: 2,000 SOL tokens. Min: 200 SOL tokens

- Unauthorized mutation of account metadata or state not directly causing fund loss
- Access control bypass on non-financial operations

**Program DoS:**
Max: 500 SOL tokens. Min: 50 SOL tokens

- Denial of service to program functionality
- Input validation flaws causing panics or aborts

### Scope

Only the `spl-stake-pool` program is included in the bounty scope.

| Program          | Mainnet Program ID                             |
| ---------------- | ---------------------------------------------- |
| `spl-stake-pool` | `SPoo1Ku8WFXoNDMHPsrGSTSG1Y47rzgn41SLUNakuHy`  |

### Known Issues

Issues already publicly known are not eligible for a new bounty. Reporters will
be notified during triage if their submission is already publicly known.

### Out of Scope

The following components are out of scope for the bounty program:

- Client SDKs, JavaScript/TypeScript bindings, and test utilities in this repository
- Any encrypted credentials, auth tokens, etc. checked into the repo
- Bugs in dependencies. Please take them upstream!
- Attacks that require social engineering
- Any undeveloped automated tooling (scanners, etc) results. (OK with developed PoC)
- Any asset whose source code does not exist in this repository (including, but
  not limited to, any and all web properties not explicitly listed on this page)

### Eligibility

- Current or recent (within 6 months) employees and contractors of Anza and the Solana Foundation are not eligible
- Anyone currently engaged on a security audit of the in-scope component is not eligible
- Anyone under a grant or financial arrangement with the Solana Foundation and Anza to develop or audit related tools is not eligible
- Submissions MUST include an exploit proof-of-concept to be considered eligible
- All PoC code must be inlined in the advisory as source code — no binaries, no external downloads
- The participant submitting the bug report shall follow the process outlined within this document
- Valid exploits can be eligible even if they are not successfully executed on a public cluster
- Multiple submissions for the same class of exploit are still eligible for
  compensation, though may be compensated at a lower rate, however these will be
  assessed on a case-by-case basis
- Participants must complete KYC and sign the participation agreement
  [here](https://solana.org/kyc) when the registrations are open. Security
  exploits will still be assessed and open for submission at all times. This
  needs only be done prior to distribution of tokens.

### Safe Harbor

We support responsible security research and will not pursue civil or criminal
action against researchers who:

- Act in good faith within the scope of this program
- Avoid accessing or modifying user data beyond the minimum necessary to demonstrate the vulnerability
- Do not disrupt services or negatively affect other users
- Report vulnerabilities promptly and allow reasonable time for remediation

We follow the [disclose.io](https://disclose.io) framework. This safe harbor
does not apply to researchers who exceed these terms.

### Duplicate Reports

Compensation for duplicative reports will be split among reporters with first to
report taking priority using the following equation:

```
R = total reports
ri = report priority
bi = bounty share

bi = 2 ^ (R - ri) / ((2^R) - 1)
```

#### Bounty Split Examples

| Total reports | Priority | Share  |
| ------------- | -------- | ------ |
| 1             | 1        | 100%   |
| 2             | 1        | 66.67% |
| 2             | 2        | 33.33% |
| 3             | 1        | 57.14% |
| 3             | 2        | 28.57% |
| 3             | 3        | 14.29% |
| 4             | 1        | 53.33% |
| 4             | 2        | 26.67% |
| 4             | 3        | 13.33% |
| 4             | 4        | 6.67%  |
| 5             | 1        | 51.61% |
| 5             | 2        | 25.81% |
| 5             | 3        | 12.90% |
| 5             | 4        | 6.45%  |
| 5             | 5        | 3.23%  |

### Payment of Bug Bounties

- Bounties are currently awarded on a rolling/weekly basis and paid out within 30 days upon receipt of an invoice.
- Bug bounties that are paid out in SOL are paid to stake accounts with a lockup expiring 12 months from the date of delivery of SOL.
- Note: payment notices need to be sent to <ap@solana.org> within 90 days of
  receiving payment advice instructions. Failure to do so may result in
  forfeiture of the bug bounty reward.

### Program Changes

The organization reserves the right to modify or suspend this program at any
time. Reports submitted before any suspension date will be reviewed under the
policy in effect at submission time.
