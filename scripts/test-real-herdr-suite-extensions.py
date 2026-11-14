"""P7/P9/P46: existing extension packages consumed through real Herdr/OMP.

S2/S3/S4, isolated loopback model only. Real factories import sibling TS and a
package dependency; actual tools read assets and compute literal known results.
No SDK loader, source CLI, user credentials, configuration-dir product redirect,
or replacement manager. Shared real-host lifecycle comes from the skills runner.
"""
import sys

from acceptance.suites.extensions import main


if __name__ == '__main__':
    sys.exit(main())
