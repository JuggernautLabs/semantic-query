# Streaming Display Protocol Issues

## Problem Statement
The deepseek agent demo attempts to provide a "live typing" experience by:
1. Printing tokens as they arrive in real-time
2. Detecting when JSON tool calls appear in the stream
3. Retroactively deleting the raw JSON and replacing it with formatted output

## Fundamental Issues

### 1. **Prediction Problem**
The system must predict what content will be parsed as structured data before parsing is complete. This creates a race condition where text gets printed, then needs to be retroactively classified and potentially deleted.

### 2. **Boundary Detection Failure**  
The original implementation counted "lines" but failed when JSON appeared mid-sentence:
- Text: `"Let me search for informati{"name": "search"...}`  
- System incorrectly treats the entire line as "JSON lines to delete"
- Result: Legitimate text (`"Let me search for informati"`) gets erased

### 3. **Model Output Format Variability**
Models don't output consistent formats:
- Sometimes raw JSON: `{"name": "search", "args": {...}}`
- Sometimes markdown-wrapped: ````json\n{"name": "search"}\n````
- Sometimes formatted text that mentions JSON but isn't parseable
- Detection logic cannot reliably distinguish these cases

### 4. **State Synchronization**
Multiple state variables must be kept in sync:
- Character/line positions
- JSON parsing depth  
- String escaping state
- Buffer contents
- Cursor position

Any desync causes incorrect deletions or missed content.

### 5. **Terminal Capability Assumptions**
ANSI escape codes for cursor movement and deletion:
- May not work in all terminal environments
- Can be disabled by terminal settings
- Don't compose well with other output (logging, etc.)

## Attempted Solutions and Their Failures

### Line-Based Deletion (Original)
- **Approach**: Count lines while JSON is being output, delete those lines when parsing completes
- **Failure**: Treats text+JSON as one unit, deletes legitimate text that appeared before JSON started

### Character-Precise Deletion  
- **Approach**: Track exact character positions where JSON starts/ends, use cursor positioning to delete specific ranges
- **Failure**: Complex state management, fails with markdown wrappers, sensitive to terminal capabilities

### Buffered Approach
- **Approach**: Don't print tokens immediately, buffer them and only flush when confident they're not JSON  
- **Partial Success**: Avoids deletion entirely, preserves legitimate text
- **Remaining Issues**: Can't detect markdown code blocks, may delay legitimate text output

## Core Issue: Error Recovery
**All approaches fail because they cannot detect and recover from classification mistakes.**

When the system incorrectly classifies content (text as JSON, or JSON boundaries), it has no mechanism to:
1. Recognize the error occurred
2. Reconstruct what should have been displayed  
3. Correct the display state

This is fundamentally a **retroactive classification problem** that requires perfect prediction, which is impossible with the variability of LLM outputs.

## Recommendation
**Don't implement live deletion.** Instead:

1. **Simple Approach**: Print all tokens as they arrive, display parsed structures as additional formatted output
2. **Buffered Approach**: Use smart buffering with conservative flushing rules
3. **Post-Processing**: Let the stream complete, then present a cleaned-up version

The complexity and fragility of retroactive deletion outweighs the UX benefit of "clean" real-time display.

## Status
Leaving this as a known limitation. The core streaming parser works correctly - this is purely a display/UX challenge that would require significant complexity to solve robustly.