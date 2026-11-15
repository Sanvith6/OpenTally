# Stage 7A Phase 3B: Atomic File Finalization

The export writer now writes to an exclusive sibling temporary file and does not open or truncate the requested destination during serialization.

After streaming, `flush().await`, `sync_all().await`, metadata collection, and handle closure:

- Existing destinations use Windows `ReplaceFileW` with `dwReplaceFlags = 0`.
- New destinations use Windows `MoveFileExW` with flags `0`, so a concurrent creator cannot be overwritten.
- Non-Windows builds fail closed because the approved finalization contract is Windows-specific.
- Unsupported filesystem, lock, access, and race errors are returned as application I/O errors.
- The temporary-file guard commits only after finalization succeeds and removes the temporary file on streaming, synchronization, finalization, cancellation, or panic cleanup paths.

No accounting, database schema, or frontend behavior was changed.

## Integration coverage

The Windows integration suite covers CSV and JSON creation/replacement, overwrite denial, empty exports, company isolation, snapshot-consistent reads, cancellation/pool hygiene, writer-error cleanup, and preservation of an existing destination when finalization fails. The filesystem cases use the Windows development filesystem; writer and cancellation cases are controlled failure simulations and are labeled separately in the test suite.

The implementation does not claim power-loss durability beyond `sync_all` and the guarantees of the underlying filesystem and storage hardware. Network shares and filesystems that reject the approved Windows primitives fail closed; there is no delete-and-rename fallback.
