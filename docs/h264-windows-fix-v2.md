# H.264 Windows Compatibility Fixes (Part 2)

## 1. Rust Backend Fix (`h264_encoder.rs`)

The `extract_sps_pps_to_avcc` function was incorrect, causing invalid `sps_pps` data to be sent to the frontend. This was a primary cause of decoder initialization failures.

### Issues Fixed
- **Missing NAL Headers**: Previous implementation sliced off the `0x67`/`0x68` NAL header bytes. AVCC specs require these bytes to be present in the configuration record.
- **Incorrect AVCC Header Construction**: The lengthSizeMinusOne/numOfSPS and other fields were being packed incorrectly (merging nibbles into wrong bytes).

### Solution
- Replaced the extraction logic with a spec-compliant version.
- Correctly parses Annex-B start codes (3 or 4 bytes).
- Preserves the NAL header byte in the extracted SPS/PPS.
- Constructs the AVCC configuration record (decoder configuration box) with correct bitwise layout (version, profile, level, length size, etc.).

## 2. Frontend Logic Updates (`H264VideoPlayer.tsx`)

### Issues Fixed
- **Strict Codec String Rejection**: Windows decoders were rejecting strict codec strings like `avc1.42C02A` even if they could decode the content.
- **False Negative `isConfigSupported`**: On Windows, `VideoDecoder.isConfigSupported` often returns `false` for valid configurations (especially mixed profile/level checks), causing our app to incorrectly switch to software decoding or fail entirely.

### Solution
- **Default to Baseline String**: Initial codec string is now always set to `'avc1.42E01f'` (Baseline 3.1) when a description is provided. The actual decoding parameters are derived from the `description` (AVCC blob), so the codec string primarily serves as a "compatibility key" to pass browser checks.
- **Relaxed Support Check**: If `isConfigSupported` returns `false`, we now **Log a Warning** and **Proceed** to call `configure()` anyway. We rely on the `error` callback of the decoder to handle actual failures.
- **Removed Aggressive Fallback**: We no longer automatically switch to `prefer-software` just because the support check failed. We let the hardware decoder try first.

## How to Verify
1.  Running the Rust backend should now produce valid `sps_pps` arrays (non-empty, correct length).
2.  Frontend logs should show:
    ```
    [H264Player] Decoder config attempt: { codec: 'avc1.42E01f', realProfile: 'avc1.42C02A', ... }
    ```
3.  Even if `isConfigSupported` logs a warning, the video should start playing without "Configuration not supported" errors.
