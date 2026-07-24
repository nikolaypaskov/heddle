# Security Policy

Heddle is a privacy-oriented, community fork of the open-source
[Warp](https://github.com/warpdotdev/Warp) client. It is **not** affiliated with
Warp / Denver Technologies, and Warp does not handle Heddle security reports.

## Reporting a vulnerability

Please practice responsible disclosure: **do not** open a public issue or pull
request for a suspected vulnerability, as that can expose it before a fix exists.

Report it privately through this repository's GitHub Security Advisories:

- **[Open a private security advisory](https://github.com/nikolaypaskov/heddle/security/advisories/new)**

If you cannot use GitHub Security Advisories, open a normal issue that says only
*"security report — please contact me privately"* (with no details) so a
maintainer can arrange a private channel.

We will acknowledge your report as promptly as a small volunteer project can, and
work with you to understand and resolve the issue. There is no bug-bounty program.

## Scope

Heddle removes Warp's cloud backend, telemetry, and hosted authentication, so
whole classes of network/account issues do not apply. Reports about Warp's
servers, `warp.dev`, or hosted services should go to
**[Warp](https://github.com/warpdotdev/Warp/security)**, not here.
