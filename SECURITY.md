# Security policy

## Scope

TokenScope is a testing tool. Security-relevant reports include: memory-safety or parsing issues
in the tool, ways untrusted scenario/finding/config/corpus files could cause harm (path
traversal, unbounded resource use, code execution), and incorrect results that could lead users to
believe a vulnerable program is safe.

The vulnerable demo escrow (`programs/demo-escrow` without the `corrected` feature) is
**intentionally vulnerable**. Reports about it are not vulnerabilities; never deploy it.

## Reporting

Please report privately; do not open a public issue for an undisclosed vulnerability.

**No dedicated security contact address exists, and none is invented here.** The disclosure route
therefore depends on where the repository is hosted:

* On GitHub: use *Security -> Report a vulnerability* (private vulnerability reporting), which the
  repository owner must enable. If it is not enabled, open a public issue titled "Security contact
  request" that contains **no technical details**, and ask the maintainers for a private channel.
* On other hosts: contact the maintainers through the host's private-message or confidential-issue feature.

A maintainer must replace this section with a real contact (and enable private reporting) before the first
public release.

Include the version (`tokenscope --version`), the command run, and a minimal input file. No response
timeline is promised for this early-stage project.

## Known, accepted dependency advisory

`cargo audit` reports RUSTSEC-2026-0387 (`crypto-bigint` timing variability), reachable only through
Mollusk's Agave dependency tree. It is not relevant to this tool's use (no secrets or key operations) and is
recorded with its justification in `docs/dependency-review.md` and `.cargo/audit.toml`.

## Handling findings from TokenScope itself

A TokenScope finding is an *invariant violation on observed state*. It is not proof of
exploitability. If you use TokenScope to find a bug in someone else's program, follow that
project's disclosure process.

## Supported versions

Only the latest commit of the default branch.
