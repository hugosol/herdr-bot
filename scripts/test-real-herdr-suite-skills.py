"""P5/P6/P7/P8/P10/P15: selected skills consumed by installed OMP via Herdr.

Approved seams S2/S3/S4. A controlled loopback model asks the REAL read tool
for skill:// resources and calls a REAL native project MCP tool in the same
turn. It checks literal bytes, the exact advertised skill set, native project
instructions and omission of Suite templates. It does not use SDK assembly,
source CLI, user credentials, a replacement manager or a probe-only API.
"""
import sys

from acceptance.suites.skills import main


if __name__ == '__main__':
    sys.exit(main())
