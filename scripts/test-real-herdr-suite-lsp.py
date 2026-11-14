"""P37/P38 at S2/S3/S4: installed OMP via real Herdr, never an SDK loader.

The loopback model requests real native LSP capabilities/status. No fake language
server, installation, PATH mutation, user credentials or user Chat is used.
Default all-language acceptance remains nonzero if any prerequisite is missing.
"""
import sys

from acceptance.suites.lsp import main


if __name__ == '__main__':
    sys.exit(main())
