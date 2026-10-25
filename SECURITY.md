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

* Use the hosting platform's private vulnerability reporting for this repository.
* **A dedicated security contact address has not been established yet.** Add one here before the
  first public release.

Include the version (`tokenscope --version`), the command run, and a minimal input file.
Acknowledge-and-triage timelines are not promised for this early-stage project.

## Handling findings from TokenScope itself

A TokenScope finding is an *invariant violation on observed state*. It is not proof of
exploitability. If you use TokenScope to find a bug in someone else's program, follow that
project's disclosure process.

## Supported versions

Only the latest commit of the default branch.
