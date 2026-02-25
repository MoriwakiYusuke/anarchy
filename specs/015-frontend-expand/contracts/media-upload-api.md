# Contract: Media Upload API

**Version**: 1.0.0  
**Date**: 2026-02-25  
**Type**: React Hook + Storage Node RPC

## Overview

メディア（画像・動画）をクライアントサイドでSSS/KZG分割し、分散ストレージノードにアップロードするインターフェース。

## Hook Interface

### useMediaUpload

```typescript
// apps/frontend/src/hooks/useMediaUpload.ts

export interface UseMediaUploadOptions {
  maxFiles?: number           // default: 4
  concurrency?: number        // default: 5 (parallel uploads)
  onProgress?: (progress: UploadProgress) => void
  onComplete?: (results: MediaUploadResult[]) => void
  onError?: (error: string, fileId: string) => void
}

export interface UseMediaUploadResult {
  /** Currently staged files */
  files: MediaFile[]
  
  /** Add file(s) to upload queue */
  addFiles: (files: FileList | File[]) => void
  
  /** Remove a staged file */
  removeFile: (fileId: string) => void
  
  /** Start uploading all staged files */
  upload: () => Promise<MediaUploadResult[]>
  
  /** Cancel ongoing upload */
  cancel: () => void
  
  /** Clear all files and reset state */
  reset: () => void
  
  /** Validate a single file */
  validateFile: (file: File) => FileValidation
  
  /** Overall upload state */
  state: UploadState
}

export interface MediaFile {
  id: string                   // crypto.randomUUID()
  file: File
  type: 'image' | 'video'
  size: number
  previewUrl?: string          // blob: URL
  uploadProgress: number       // 0-100
  status: 'pending' | 'splitting' | 'uploading' | 'complete' | 'error'
  error?: string
  result?: MediaUploadResult
}

export interface MediaUploadResult {
  fileId: string
  merkleRoot: string           // hex encoded
  mediaType: 'image' | 'video'
  sizeBytes: number
  width?: number
  height?: number
  threshold: number            // k
  totalShards: number          // n
}

export interface UploadProgress {
  fileId: string
  phase: 'splitting' | 'uploading'
  current: number
  total: number
  percent: number
}

export interface FileValidation {
  valid: boolean
  error?: string               // i18n key
}

export type UploadState = 'idle' | 'processing' | 'complete' | 'error'

export function useMediaUpload(options?: UseMediaUploadOptions): UseMediaUploadResult
```

## Usage Example

```tsx
// components/MediaUpload.tsx
import { useMediaUpload } from '@/hooks/useMediaUpload'
import { useLocale } from '@/i18n'

function MediaUpload() {
  const { t } = useLocale()
  const { 
    files, 
    addFiles, 
    removeFile, 
    upload, 
    validateFile,
    state 
  } = useMediaUpload({
    maxFiles: 4,
    onProgress: (progress) => console.log(`${progress.fileId}: ${progress.percent}%`),
    onComplete: (results) => console.log('Upload complete:', results)
  })
  
  const handleDrop = (e: DragEvent) => {
    e.preventDefault()
    addFiles(e.dataTransfer.files)
  }
  
  const handleFileSelect = (e: ChangeEvent<HTMLInputElement>) => {
    if (e.target.files) {
      addFiles(e.target.files)
    }
  }
  
  return (
    <div 
      onDrop={handleDrop}
      onDragOver={e => e.preventDefault()}
    >
      <input type="file" multiple accept="image/*,video/*" onChange={handleFileSelect} />
      
      {files.map(file => (
        <MediaPreview 
          key={file.id}
          file={file}
          onRemove={() => removeFile(file.id)}
        />
      ))}
      
      {state === 'processing' && <ProgressBar files={files} />}
      
      <button onClick={upload} disabled={state === 'processing' || files.length === 0}>
        {t('media.upload')}
      </button>
    </div>
  )
}
```

## Processing Flow

```
1. File Selection
   ├─ Validate format (JPEG/PNG/GIF/WebP, MP4/WebM)
   ├─ Validate size (100MB image, 1GB video)
   └─ Generate preview URL

2. EXIF Stripping (images only)
   └─ Canvas re-encode to remove metadata

3. Hybrid Split (Web Worker)
   ├─ Compress if beneficial
   ├─ AES-256-GCM encrypt
   ├─ Reed-Solomon k-of-n encode
   └─ SSS key split

4. Shard Upload (parallel, 5 concurrent)
   ├─ storage_storeKzgShard for each shard
   └─ Progress callback per shard

5. Complete
   └─ Return merkle_root and metadata
```

## Web Worker Interface

```typescript
// workers/mediaProcessor.worker.ts

interface ProcessMediaMessage {
  type: 'process'
  fileId: string
  data: ArrayBuffer
  threshold: number     // k (default: 3)
  shardCount: number    // n (default: 5)
}

interface ProcessMediaResult {
  type: 'result'
  fileId: string
  merkleRoot: Uint8Array
  shards: SerializedShard[]
  metadata: {
    originalLen: number
    compressed: boolean
    ciphertextLen: number
    shardSize: number
    threshold: number
    totalShards: number
  }
}

interface ProcessMediaProgress {
  type: 'progress'
  fileId: string
  phase: 'splitting'
  percent: number
}

interface SerializedShard {
  index: number
  chunk: Uint8Array         // base64 for JSON transfer
  chunkHash: Uint8Array
  keyShare: Uint8Array
  kzgCommitment?: Uint8Array
}
```

## Storage Node RPC Contract

### storage_storeKzgShard

```typescript
// JSON-RPC request
interface StoreKzgShardParams {
  merkle_root: string      // hex encoded [u8; 32]
  index: number            // shard index
  data: string             // base64 encoded chunk
  kzg_commitment: string   // hex encoded
  chunk_hash: string       // hex encoded Blake2b hash
  key_share?: string       // base64 encoded (optional)
}

// JSON-RPC response
interface StoreKzgShardResult {
  fragment_id: string      // hex encoded
  stored_at: string        // ISO timestamp
}

// Fetch usage
const response = await fetch('http://storage-node:3030/rpc', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    jsonrpc: '2.0',
    method: 'storage_storeKzgShard',
    params: {
      merkle_root: merkleRootHex,
      index: shard.index,
      data: base64Encode(shard.chunk),
      kzg_commitment: hexEncode(shard.kzgCommitment),
      chunk_hash: hexEncode(shard.chunkHash)
    },
    id: 1
  })
})
```

### storage_getFragment

```typescript
// JSON-RPC request
interface GetFragmentParams {
  merkle_root: string
  index: number
}

// JSON-RPC response
interface GetFragmentResult {
  data: string             // base64 encoded
  hash: string             // Blake2b hash
}
```

## Validation Rules

### File Format

| Media Type | Accepted Formats | MIME Types |
|------------|-----------------|------------|
| Image | JPEG, PNG, GIF, WebP | `image/jpeg`, `image/png`, `image/gif`, `image/webp` |
| Video | MP4, WebM | `video/mp4`, `video/webm` |

### File Size

| Media Type | Max Size | Error Key |
|------------|----------|-----------|
| Image | 100 MB | `media.imageTooLarge` |
| Video | 1 GB | `media.videoTooLarge` |

### File Count

| Rule | Max | Error Key |
|------|-----|-----------|
| Per post | 4 files | `media.tooManyFiles` |

## i18n Keys

```typescript
type MediaKeys =
  | 'media.upload'
  | 'media.dragDrop'
  | 'media.selectFiles'
  | 'media.uploading'
  | 'media.splitting'
  | 'media.complete'
  | 'media.error'
  | 'media.remove'
  | 'media.preview'
  | 'media.imageTooLarge'
  | 'media.videoTooLarge'
  | 'media.unsupportedFormat'
  | 'media.tooManyFiles'
  | 'media.uploadFailed'
```

## Error Handling

| Error | i18n Key | Recovery |
|-------|----------|----------|
| File too large | `media.imageTooLarge` / `media.videoTooLarge` | Remove file |
| Unsupported format | `media.unsupportedFormat` | Remove file |
| Too many files | `media.tooManyFiles` | Remove excess files |
| Network error | `media.uploadFailed` | Retry upload |
| Storage node error | `media.uploadFailed` | Retry or try different node |
