"""P17/P19/P20/P21/P22/P46: installed OMP native MCP through real Herdr.

Approved seams S2/S3/S4. The native host, discovery and manager remain real;
only the external MCP server and loopback model are controlled. Literal file
bytes and independent wire/start counts distinguish a tool-only bridge, lost
native sources, forced Suite precedence, and a Chat-success/readiness conflation.
"""
import sys

from acceptance.suites.mcp import main


if __name__ == '__main__':
    sys.exit(main())
