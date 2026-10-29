# Security Policy

## Supported versions

Security fixes are provided for the latest release of the client and the
latest release of the server. Older releases are not patched.

## Reporting a vulnerability

Do not open a public issue, pull request, or discussion for a vulnerability.

Use GitHub private vulnerability reporting:
<https://github.com/ClemPera/Nooto/security/advisories/new>

If you cannot use GitHub, open a public issue saying only that you have a
security report and asking for a private channel. Do not include details.

A useful report includes:

- the affected component and version, or the commit if building from source;
- what an attacker can do and under which conditions;
- reproduction steps or a minimal proof of concept, if you have one;
- whether you believe it is remotely exploitable.

An incomplete report is better than no report.

## What to expect

Nooto is maintained by volunteers. We aim to acknowledge reports within
5 business days and to publish a fix and advisory within 90 days. If a fix
needs longer, we will tell you why and agree a new date with you before any
public disclosure. We will credit you in the advisory unless you ask us not to.

## Scope

In scope:

- disclosure of note contents, titles, or keys to the server or another device;
- authentication, key derivation, or encryption bypasses;
- account takeover, credential stuffing, or brute-force weaknesses in the
  login and registration flow;
- sync token or session handling flaws, including reuse after logout or
  across devices;
- access to another user's notes, metadata, or account on a shared server;
- remote code execution, memory corruption, or denial of service from crafted
  requests or sync payloads;
- client-side injection, path traversal, or local file access beyond the
  intended scope.

Out of scope:

- the missing independent audit of the encryption design. This is a known
  limitation, documented in the README. Flaws in the design itself are in
  scope;
- attacks that require an already compromised device, a malicious OS, or
  physical access to the hardware;
- vulnerabilities in third-party dependencies that are not reachable through
  Nooto's own code, report those upstream;
- missing hardening or best-practice suggestions without a demonstrated attack
  path;
- unsupported versions and self-hosted server misconfiguration.

If you are unsure whether something is in scope, report it privately and we
will triage it.

## Safe harbor

We consider good-faith security research under this policy to be authorized,
and we will not pursue or support legal action for accidental violations of
it.

Only test against an instance you own: your own self-hosted server, a local
build, or your own data on a test account. Do not run scans, fuzzing, or
exploit attempts against the public server bundled with the app, or against
any instance you do not own or have explicit permission to test. This also
means no accessing, modifying, or deleting other people's notes or accounts.
If you need to test something that the public server would affect, contact us
first and we will arrange a safe target.

This statement covers claims we control and does not bind third parties. You
must still comply with applicable law, and it does not cover research that
breaks the testing rule above.
