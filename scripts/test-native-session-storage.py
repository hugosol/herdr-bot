"""P23/P24/P25 S4 acceptance with installed, real Pi/OMP CLIs.

The loopback model is controlled; the agent launch, storage, and native discovery
are real. Never reads user auth/config or submits input to an existing session.
Retains the isolated root and prints JSON evidence; does not certify deployment H1.
"""
from acceptance.native.session_storage import main


if __name__ == '__main__':
    main()
