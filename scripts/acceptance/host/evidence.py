"""Retain invocation-owned evidence without merging existing archives."""
import os
import shutil


def copy_evidence(source, destination):
    """Retain the full owned tree, including Windows paths beyond MAX_PATH."""
    def native(path):
        absolute = os.path.abspath(path)
        if os.name != 'nt' or absolute.startswith('\\\\?\\'):
            return absolute
        if absolute.startswith('\\\\'):
            return '\\\\?\\UNC\\' + absolute[2:]
        return '\\\\?\\' + absolute
    # Keep copytree's exclusive destination semantics: an old/partial archive
    # must never be silently merged with a different invocation.
    shutil.copytree(native(source), native(destination))

