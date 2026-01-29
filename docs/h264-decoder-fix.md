# Fix: H.264 "Unsupported configuration" Error on Windows

## Problem Analysis

The error "Decoder error: Unsupported configuration. Check isConfigSupported prior to calling configure" was NOT caused by missing `isConfigSupported()` checks, but by a **format mismatch** between the decoder configuration and the bitstream format.

### Root Cause

1. **Backend** sends H.264 data in **Annex-B format** (with `00 00 00 01` start codes)
2. **Backend** also sends `sps_pps` in **avcC/AVCDecoderConfigurationRecord format**
3. **Frontend** was NOT setting `config.description` in the VideoDecoder config
4. **Problem**: According to WebCodecs spec:
   - ✅ **With `description`**: Decoder expects **AVCC format** (ISO 14496-15, length-prefixed NAL units)
   - ⚠️ **Without `description`**: Decoder expects **Annex-B format** (start codes), AND keyframes must contain SPS/PPS inline
   
### Why Windows Failed

Windows hardware decoders (D3D11/MediaFoundation) are stricter about format compliance:
- Received: Annex-B bitstream + separate avcC description (not in config)
- Decoder expected: Either pure Annex-B with inline SPS/PPS, OR AVCC with description
- Result: **Configuration mismatch → NotSupportedError**

## Solution: Use AVCC Format (Option B)

We implemented **Option B**: Use AVCC format with `config.description`

### Changes Made

#### 1. Added Annex-B → AVCC Conversion Function

```typescript
function annexBToAvcc(annexB: Uint8Array): Uint8Array {
  // Parses Annex-B start codes (00 00 00 01 or 00 00 01)
  // Converts to length-prefixed format (4-byte big-endian length + NAL data)
}
```

#### 2. Updated VideoDecoderConfig

```typescript
const config: VideoDecoderConfig = {
  codec: codecStr,              // avc1.PPCCLL (from avcC)
  codedWidth: width,            // Required for some implementations
  codedHeight: height,          // Required for some implementations
  description,                  // avcC from backend (CRITICAL!)
  optimizeForLatency: !forceSoftware,
  hardwareAcceleration: forceSoftware ? 'prefer-software' : 'prefer-hardware',
};
```

Key additions:
- ✅ `codedWidth` / `codedHeight` - Required by some decoder implementations
- ✅ `description` - Tells decoder to expect AVCC format

#### 3. Convert Bitstream Before Decoding

```typescript
// Track if decoder is using AVCC format
const usingDescriptionRef = useRef(false);

// In initDecoder:
usingDescriptionRef.current = hasDescription;

// In decodeFrame:
let dataForDecoder = h264Data;
if (usingDescriptionRef.current) {
  dataForDecoder = annexBToAvcc(h264Data);  // Convert!
}

const chunk = new EncodedVideoChunk({
  type: frameData.is_keyframe ? 'key' : 'delta',
  timestamp: frameData.timestamp * 1000,
  data: dataForDecoder,  // Use converted data
});
```

#### 4. Updated Error Recovery

The retry logic also converts stored keyframes to AVCC when retrying after decoder reset.

## Benefits

✅ **Spec-compliant**: Config and bitstream format now match (AVCC + AVCC)
✅ **Windows compatible**: Hardware decoders accept the proper format
✅ **No backend changes**: All conversion happens in frontend
✅ **Better logging**: Shows when conversion happens for debugging

## Testing Checklist

- [ ] Test on Windows (hardware decoder)
- [ ] Test on macOS (verify still works)
- [ ] Check console logs for "Converted Annex-B → AVCC" messages
- [ ] Verify no "Unsupported configuration" errors
- [ ] Confirm video plays smoothly with both keyframes and delta frames

## Alternative (Not Implemented)

**Option A**: Keep Annex-B format
- Would require: Modifying Rust backend to prepend SPS/PPS to every keyframe chunk
- Would require: Removing `sps_pps` from config (no `description`)
- More invasive changes needed

## References

- [WebCodecs VideoDecoder API](https://www.w3.org/TR/webcodecs/#videodecoder-interface)
- [ISO/IEC 14496-15 (AVCC format)](https://www.w3.org/TR/webcodecs-avc-codec-registration/)
- [H.264 Annex-B format](https://www.itu.int/rec/T-REC-H.264)
