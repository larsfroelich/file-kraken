# ZIP Support: Part 1 Handoff

This document outlines the foundation laid in Part 1 of the transparent ZIP support feature, as requested, to guide the integration in Part 2.

## Implemented Features

1. **Identity Model & Database Schema**
   * **Model**: `FileKrakenFileType::ArchiveMember` was added to differentiate members from normal files and archives.
   * **Identity**: Members are assigned unique IDs via the `path` field in the format `archive_path::member_index::member_name`.
   * **Database**: Added columns `archive_path`, `archive_size`, and `archive_mtime` to the `files` table in SQLite. Legacy compatibility is maintained through non-destructive `ALTER TABLE ADD COLUMN` queries.
   * **Display Path**: Provided `FileKrakenFile::display_path()` which returns the desired `archive_path::member_name` format for the UI.

2. **Scanning & Indexing**
   * Handled seamlessly in `src/processing/scan.rs`. Discovered ZIP archives are opened, and their contents are streamed through the existing location indexing system as `ArchiveMember` types. Re-scans handle missing/added members using the pre-existing deleted files reconciliation.

3. **Hashing Logic**
   * Extracted a generic `hash_stream` function in `src/utils/hashing.rs` that accepts any `std::io::Read` stream. Both loose files and zipped members can use this stream reader for computing SHA-256 hashes uniformly without loading entire files into memory.

4. **Rewrite Engine**
   * A batch rewrite engine (`rewrite_archive`) is located in `src/processing/zip_support/archive.rs`.
   * It takes an `ArchiveSnapshot` (ensuring we don't inadvertently remove files from an updated archive) and a list of `MemberIdentity` items.
   * Safely streams retained members to a temporary file next to the original archive using `zip` crate's `raw_copy_file` (preserving compression/metadata) and finally renames it atomically to replace the original.
   * Returns explicit success/failure information via the `RewriteError` enum, leaving the original intact on failure.

5. **Safe Intermediate Filter**
   * `filter_out_archive_members` was added to `src/processing/find_duplicates.rs` to strip `ArchiveMember`s from candidate duplicate sets, ensuring Part 1 retains the old behavior entirely until Part 2 integrates the checkbox features.

## Expected Integration for Part 2

Part 2 should coordinate the backend engine with the user-facing app state and complete the workflow.

* **UI Checkbox:** Expose a setting in the app for "Search inside ZIP archives". Use this to toggle whether `filter_out_archive_members` is used or if `ArchiveMember`s are allowed into the final duplicate list.
* **UI Display:** Update the display lists in the tab components to use `file.display_path()` rather than `file.path` so the `::` formatted member paths are shown clearly to the user.
* **Rewrite Coordination:** When deleting duplicates, intercept the `FileKrakenFileType::ArchiveMember` cases. Group these deletions by their parent archive. Retrieve the appropriate snapshot variables, invoke `rewrite_archive`, and then trigger a rescan on the parent archive's path (or location) to refresh the index cleanly.
* **Cache Invalidation:** Ensure that the UI appropriately reflects invalidations of deleted members.
