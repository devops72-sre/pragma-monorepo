// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

/// @title ErrorsLib
/// @author Pragma Labs
/// @custom:contact security@pragma.build
/// @notice Library exposing errors.
library ErrorsLib {
    // Insufficient fee is paid to the method.
    // Signature: 0x025dbdd4
    error InsufficientFee();
    // Update data is coming from an invalid data source.
    // Signature: 0xe60dce71
    error InvalidUpdateDataSource();
    // Version is invalid.
    // TODO: add signature
    error InvalidVersion();
    // Given message is not a valid Hyperlane Checkpoint Root.
    // TODO: add signature
    error InvalidHyperlaneCheckpointRoot();
    // Signature is invalid.
    error InvalidHyperlaneSignatures(string);
    // Update data is invalid (e.g., deserialization error)
    // Signature: 0xe69ffece
    error InvalidUpdateData();
    // Data feed type is not supported.
    // TODO: add signature
    error InvalidDataFeedType();
    // Data feed is not found.
    // TODO: add signature
    error DataNotFound();
    // Data feed is stale. (e.g., not updated for a long time)
    // TODO: add signature
    error DataStale();
}
